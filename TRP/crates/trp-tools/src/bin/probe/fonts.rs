//! TextMeshPro's fonts and texts: scales, widths, captions and materials.

use anyhow::Result;
use trp_data::unity::Assets;

// Each font asset the GEAR tab's panels and the things' views draw
// in (`fontscale`): its sampling size and scale, its lines, and the
// width of a few words at 48 by its glyphs (TextMeshPro's measure)
// beside the same words by its font file's own advances at an em of
// 48 (the app's drawing measure), and their ratio.
pub fn fontscale(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let health = trp_data::interface::screen::HealthPanel::load(&assets.install, &gear.file)?;
    let items = trp_data::interface::screen::ItemViews::load(&assets.install)?;
    let mut seen = std::collections::BTreeSet::new();
    let panels: Vec<&trp_data::interface::screen::ScenePanel> = [health.gear_panel.as_ref(), items.slot.as_ref(), items.grid.as_ref()].into_iter().flatten().collect();
    for panel in panels {
        for node in &panel.nodes {
            let Some(text) = &node.text else { continue };
            let Some(file) = panel.file_of(text.font.0) else { continue };
            if std::env::var("FONTSCALE_TEXTS").is_ok() {
                println!("text {:70} size {} style {} spacing {} kerning {} margin {:?} font {} material {:?} align {}/{} wraps {} overflow {}", node.path, text.size, text.style, text.spacing, text.kerning, text.margin, text.font.1, text.material, text.horizontal, text.vertical, text.wraps, text.overflow);
                continue;
            }
            if !seen.insert((file.clone(), text.font.1)) {
                continue;
            }
            let tmp = trp_data::interface::screen::TmpFont::read(&assets.install, &file, text.font.1)?;
            let fonts = trp_data::interface::screen::build_fonts(&assets.install, &file)?;
            let name = trp_data::interface::screen::script_name(&assets.install, &file, text.font.1).unwrap_or_default();
            let ttf = trp_data::interface::screen::font_for(&name, &fonts).or_else(|| trp_data::interface::screen::font_by_face(&tmp.family, &tmp.style, &fonts));
            println!("{name} ({file} {}): {} {}, sampled at {}, scale {}, ascent {} descent {}", text.font.1, tmp.family, tmp.style, tmp.point_size, tmp.scale, tmp.ascent, tmp.descent);
            println!("  bold typeface {:?}, normal spacing {}, bold spacing {}", tmp.bold, tmp.normal_spacing, tmp.bold_spacing);
            println!("  atlases {:?}, glyphs {}, M: {:?}", tmp.atlases, tmp.glyph_table.len(), tmp.glyph_of('M'));
            let missing: String = (' '..='~').filter(|letter| tmp.glyph_of(*letter).is_none()).collect();
            println!("  printable ASCII letters the asset has no glyph for: {missing:?}");
            let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
            for (atlas_file, atlas_id) in &tmp.atlases {
                match pictures.texture(atlas_file, *atlas_id) {
                    Ok(t) => {
                        println!("  atlas {} {}x{} format {} mips {} bytes {}", t.name, t.width, t.height, t.format, t.mip_count, t.data.len());
                        if let Some((g, _)) = tmp.glyph_of('M') {
                            let row = (g.rect[1] + g.rect[3] / 2) as usize;
                            let line: Vec<String> = (g.rect[0] - 12..g.rect[0] + 12).map(|x| format!("{}", t.data[row * t.width as usize + x as usize])).collect();
                            println!("  M's middle row from 12 left of its rectangle: {}", line.join(" "));
                        }
                    }
                    Err(e) => println!("  atlas {atlas_file} {atlas_id}: {e:#}"),
                }
            }
            if let Ok((material_file, material)) = trp_data::interface::screen::font_material(&assets.install, &file, text.font.1) {
                if let Ok(props) = trp_data::interface::screen::material_properties(&assets.install, &material_file, material) {
                    let mut floats: Vec<_> = props.floats.iter().filter(|(k, _)| ["_WeightNormal", "_WeightBold", "_FaceDilate", "_ScaleRatioA", "_ScaleRatioB", "_ScaleRatioC", "_GradientScale", "_OutlineWidth", "_OutlineSoftness", "_UnderlayDilate", "_UnderlaySoftness", "_UnderlayOffsetX", "_UnderlayOffsetY", "_Sharpness", "_TextureWidth", "_PerspectiveFilter"].contains(&k.as_str())).collect();
                    floats.sort_by(|a, b| a.0.cmp(b.0));
                    println!("  material {:?} keywords {:?}: {:?}", props.name, props.keywords, floats);
                    println!("  shader {:?}", trp_data::interface::screen::material_shader(&assets.install, &material_file, material).map_err(|e| e.to_string()));
                }
            }
            let Some((ttf_name, bytes)) = ttf else { println!("  no font file"); continue };
            let face = ttf_parser::Face::parse(bytes, 0)?;
            println!("  file {ttf_name}: {} units to the em, ascender {} descender {} line gap {}", face.units_per_em(), face.ascender(), face.descender(), face.line_gap());
            for words in ["M", "ARMBAND", "ON SLING", "TACTICAL RIG", "AKM", "30"] {
                let one = tmp.width(words, 48.0, 0.0, false) - tmp.normal_spacing * 48.0 / 100.0 * (words.chars().count().saturating_sub(1)) as f32;
                let file_width = trp_data::interface::screen::text_width(bytes, words, 48.0, false).unwrap_or(0.0);
                println!("  {words:14} asset {one:8.2}  file {file_width:8.2}  ratio {:.4}", one / file_width.max(1e-3));
            }
        }
    }
    Ok(())
}

// A grid item view's caption box (`captionfit <cells across>
// <words>`): the box's width in units and the words' widths by the
// caption's font asset (TextMeshPro's measure: no kerning), and
// the same with each letter's pen and advance, kerning in.
pub fn captionfit(assets: &mut Assets, args: &[String], cells: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let items = trp_data::interface::screen::ItemViews::load(&assets.install)?;
    let panel = if arg(3) == Some("slot") { items.slot.as_ref() } else { items.grid.as_ref() }.ok_or_else(|| anyhow::anyhow!("no view"))?;
    let index = panel.find("Info Panel/Caption").or_else(|| panel.find("Info Panel/Name")).ok_or_else(|| anyhow::anyhow!("no caption"))?;
    let text = panel.nodes[index].text.as_ref().ok_or_else(|| anyhow::anyhow!("no text"))?;
    let file = panel.file_of(text.font.0).ok_or_else(|| anyhow::anyhow!("no font file"))?;
    let tmp = trp_data::interface::screen::TmpFont::read(&assets.install, &file, text.font.1)?;
    let across: f32 = cells.parse().unwrap_or(1.0);
    let units = [across * 63.0, 63.0];
    let rect = panel.placed(index, units).rect;
    println!("caption box {rect:?}, margins {:?}, size {}, spacing {}, kerning {}", text.margin, text.size, text.spacing, text.kerning);
    let words = arg(2).unwrap_or("M-LOK 2.5\"");
    let said = format!(" {words} ");
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    let (atlas_file, atlas_id) = tmp.atlases.first().cloned().ok_or_else(|| anyhow::anyhow!("no atlas"))?;
    let atlas = pictures.texture(&atlas_file, atlas_id)?;
    let fonts = trp_data::interface::screen::build_fonts(&assets.install, &file)?;
    let name = trp_data::interface::screen::script_name(&assets.install, &file, text.font.1).unwrap_or_default();
    let ttf = trp_data::interface::screen::font_for(&name, &fonts).or_else(|| trp_data::interface::screen::font_by_face(&tmp.family, &tmp.style, &fonts)).map(|(_, bytes)| bytes.clone());
    let face = trp_data::interface::tmp_text::TmpFace::new(tmp.clone(), &atlas, ttf.as_deref());
    let positive = face.kerning.values().filter(|value| **value > 0).count();
    println!("kerning pairs {}, of them positive {positive}", face.kerning.len());
    let layout = trp_data::interface::tmp_text::TmpLayout { em: text.size, spacing: text.spacing, kerning: text.kerning, wrap: None, align: 1, lead: 0.0 };
    let mut kept = String::new();
    for letter in said.chars() {
        kept.push(letter);
        let run = trp_data::interface::tmp_text::TmpRun { text: kept.trim_end().to_owned(), size: text.size, color: [1.0; 4], bold: text.style & 1 != 0 };
        let kerned = trp_data::interface::tmp_text::measure(&face, &[run], &layout)[0];
        println!("  {kept:?}: {:.3}  kerned {kerned:.3}", tmp.width(kept.trim_end(), text.size, text.spacing, text.style & 1 != 0));
    }
    Ok(())
}

// Words drawn as the game draws them (`tmptext <words> <size>
// <bold 0|1> <spacing> <kerning 0|1> <asset id> <pixels to a unit>
// <out.png> [ground r,g,b]`): from the asset of that number in
// resources.assets and its own material, laid on a plain ground, as
// a PNG, with the box the words are aligned by.
pub fn tmptext(assets: &mut Assets, args: &[String], words: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let num = |k: usize| arg(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
    let (size, bold, spacing, kerning, id, s) = (num(2), num(3) != 0.0, num(4), num(5) != 0.0, num(6) as i64, num(7));
    let out = arg(8).unwrap_or("local/tmptext.png");
    let ground: Vec<f32> = arg(9).unwrap_or("15,15,15").split(',').map(|v| v.parse::<f32>().unwrap_or(0.0) / 255.0).collect();
    let file = "resources.assets";
    let face_of = |id: i64| -> anyhow::Result<trp_data::interface::tmp_text::TmpFace> {
        let font = trp_data::interface::screen::TmpFont::read(&assets.install, file, id)?;
        let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
        let (atlas_file, atlas_id) = font.atlases.first().cloned().ok_or_else(|| anyhow::anyhow!("no atlas"))?;
        let atlas = pictures.texture(&atlas_file, atlas_id)?;
        let fonts = trp_data::interface::screen::build_fonts(&assets.install, file)?;
        let name = trp_data::interface::screen::script_name(&assets.install, file, id).unwrap_or_default();
        let ttf = trp_data::interface::screen::font_for(&name, &fonts).or_else(|| trp_data::interface::screen::font_by_face(&font.family, &font.style, &fonts)).map(|(_, bytes)| bytes.clone());
        Ok(trp_data::interface::tmp_text::TmpFace::new(font, &atlas, ttf.as_deref()))
    };
    let mut face = face_of(id)?;
    if let Some((_, bold_id)) = face.font.bold.clone() {
        face.bold = Some(std::sync::Arc::new(face_of(bold_id)?));
    }
    let (material_file, material) = trp_data::interface::screen::font_material(&assets.install, file, id)?;
    let material = trp_data::interface::tmp_text::TmpMaterial::of(&trp_data::interface::screen::material_properties(&assets.install, &material_file, material)?);
    let color = [0xb6 as f32 / 255.0, 0xc1 as f32 / 255.0, 0xc7 as f32 / 255.0, 1.0];
    let runs = [trp_data::interface::tmp_text::TmpRun { text: words.to_owned(), size: size * s, color, bold }];
    let image = trp_data::interface::tmp_text::draw(&face, &material, &runs, &trp_data::interface::tmp_text::TmpLayout { em: size * s, spacing, kerning, wrap: None, align: 1, lead: 0.0 });
    println!("{}x{} px, the words' box at {:?}; material {material:?}", image.width, image.height, image.layout);
    // Laid on the ground (the screen's numbers, blended as they are).
    let (w, h) = (image.width + 8, image.height + 8);
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let (ix, iy) = (x as isize - 4, y as isize - 4);
            let px = if ix >= 0 && iy >= 0 && (ix as usize) < image.width && (iy as usize) < image.height { &image.rgba[(iy as usize * image.width + ix as usize) * 4..][..4] } else { &[0, 0, 0, 0][..] };
            let a = px[3] as f32 / 255.0;
            for c in 0..3 {
                rgb[(y * w + x) * 3 + c] = ((px[c] as f32 / 255.0 * a + ground[c] * (1.0 - a)) * 255.0 + 0.5) as u8;
            }
        }
    }
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(out)?), w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgb)?;
    println!("written {out}");
    Ok(())
}

// The game's window of a container's grids as the app reads it:
// its objects the script works and its colours.
// How wide and high TextMeshPro makes words in a font asset at a
// size (`textwidth resources.assets 34142 11 SEARCH`).
pub fn textwidth(assets: &mut Assets, args: &[String]) -> Result<()> {
    let (file, id, size, said) = (args.get(1).cloned().unwrap_or_default(), args.get(2).and_then(|v| v.parse().ok()).unwrap_or(0), args.get(3).and_then(|v| v.parse().ok()).unwrap_or(11.0), args.get(4..).unwrap_or_default().join(" "));
    let font = trp_data::interface::screen::TmpFont::read(&assets.install, &file, id)?;
    println!("{said:?} at {size}: {} wide, {} high (point size {}, scale {}, ascent {}, descent {}, spacing {})", font.width(&said, size, 0.0, false), font.height(size), font.point_size, font.scale, font.ascent, font.descent, font.normal_spacing);
    Ok(())
}

// Each text of the views of a thing and of the inventory's panels:
// its font asset's own material, and the material it is drawn
// with, where that is another.
pub fn textmats(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let items = trp_data::interface::screen::ItemViews::load(&assets.install)?;
    let health = trp_data::interface::screen::HealthPanel::load(&assets.install, &gear.file)?;
    let quick = trp_data::interface::screen::QuickViews::load(&assets.install, &gear.file)?;
    let panels: Vec<(&str, &trp_data::interface::screen::ScenePanel)> = [("slot view", items.slot.as_ref()), ("grid view", items.grid.as_ref()), ("health", Some(&health.panel)), ("quick", Some(&quick.panel))].into_iter().filter_map(|(name, panel)| Some((name, panel?))).collect();
    for (name, panel) in panels {
        for node in &panel.nodes {
            let Some(text) = &node.text else { continue };
            let Some(font_file) = panel.file_of(text.font.0) else { continue };
            let own = trp_data::interface::screen::font_material(&assets.install, &font_file, text.font.1).ok();
            let used = text.material.and_then(|(outside, id)| Some((panel.file_of(outside)?, id)));
            let say = |which: &Option<(String, i64)>| which.as_ref().and_then(|(file, id)| trp_data::interface::screen::material_properties(&assets.install, file, *id).ok()).map_or("-".to_owned(), |props| format!("{} {:?} outline {:?} underlay {:?}", props.name, props.keywords, props.floats.get("_OutlineWidth"), props.floats.get("_UnderlayDilate")));
            println!("{name}: {} {:?} size {} font {} {}\n    font's {}\n    drawn {}{}", node.path, text.text, text.size, font_file, text.font.1, say(&own), say(&used), if used == own { "  (the same)" } else { "" });
        }
    }
    Ok(())
}

pub fn fontmaterial(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("which font asset?"))?.parse()?;
    let (material_file, material) = trp_data::interface::screen::font_material(&assets.install, file, id)?;
    let props = trp_data::interface::screen::material_properties(&assets.install, &material_file, material)?;
    println!("{} ({material_file} {material})", props.name);
    let mut floats: Vec<_> = props.floats.iter().collect();
    floats.sort_by(|a, b| a.0.cmp(b.0));
    for (key, value) in floats {
        println!("  {key} = {value}");
    }
    let mut colors: Vec<_> = props.colors.iter().collect();
    colors.sort_by(|a, b| a.0.cmp(b.0));
    for (key, value) in colors {
        println!("  {key} = {value:?}");
    }
    Ok(())
}

// Every text of a scene sized to its words (`ContentSizeFitter`),
// its saved width beside the width its font's advances give it
// (`textwidths level44`): how TMP's preferred width goes.
pub fn textwidths(assets: &mut Assets, file: &str) -> Result<()> {
    let mut tmp: std::collections::HashMap<(String, i64), Option<trp_data::interface::screen::TmpFont>> = std::collections::HashMap::new();
    let install = assets.install.clone();
    let tmp_width = |tmp: &mut std::collections::HashMap<(String, i64), Option<trp_data::interface::screen::TmpFont>>, file: &str, id: i64, shown: &str, text: &trp_data::interface::screen::TextLook| -> Option<f32> {
        let mut font = |file: &str, id: i64| tmp.entry((file.to_owned(), id)).or_insert_with(|| trp_data::interface::screen::TmpFont::read(&install, file, id).ok()).clone();
        let regular = font(file, id)?;
        let bold = text.style & 1 != 0;
        Some(match regular.bold.clone().filter(|_| bold).and_then(|(file, id)| font(&file, id)) {
            Some(alternative) => alternative.width(shown, text.size, text.spacing, false),
            None => regular.width(shown, text.size, text.spacing, bold),
        } + text.margin[0] + text.margin[2])
    };
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let roots: Vec<usize> = (0..scene.nodes.len()).filter(|&index| scene.nodes[index].layout.is_some() && scene.nodes[index].parent.is_none_or(|parent| scene.nodes[parent].layout.is_none())).collect();
    let mut fonts: std::collections::HashMap<String, Vec<(String, Vec<u8>)>> = std::collections::HashMap::new();
    let mut names: std::collections::HashMap<(String, i64), String> = std::collections::HashMap::new();
    for root in roots {
    let mut panel = trp_data::interface::screen::ScenePanel::of(&scene, root);
    panel.file = file.to_owned();
    for node in &panel.nodes {
        let Some(text) = &node.text else { continue };
        if !node.scripts.iter().any(|(class, _)| class == "ContentSizeFitter") || text.text.is_empty() || text.text.contains('\n') || text.text.contains('<') || text.auto_size.is_some() {
            continue;
        }
        let Some(font_file) = panel.file_of(text.font.0).or_else(|| (text.font.0 == 0).then(|| file.to_owned())) else { continue };
        if !fonts.contains_key(&font_file) {
            fonts.insert(font_file.clone(), trp_data::interface::screen::build_fonts(&assets.install, &font_file).unwrap_or_default());
        }
        let asset = names.entry((font_file.clone(), text.font.1)).or_insert_with(|| trp_data::interface::screen::script_name(&assets.install, &font_file, text.font.1).unwrap_or_default()).clone();
        let shown = if text.style & 16 != 0 { text.text.to_uppercase() } else if text.style & 8 != 0 { text.text.to_lowercase() } else { text.text.clone() };
        let Some(width) = tmp_width(&mut tmp, &font_file, text.font.1, &shown, text) else { continue };
        let _ = trp_data::interface::screen::font_for(&asset, &fonts[&font_file]);
        let saved = node.layout.size_delta[0];
        let fit = node.scripts.iter().find(|(class, _)| class == "ContentSizeFitter").and_then(|(_, fields)| fields.get(0..4)).map_or(-1, |word| i32::from_le_bytes(word.try_into().unwrap_or_default()));
        if fit != 2 {
            continue;
        }
        println!("{:?} {:7.2} saved {:7.2} measured {:7.2} size {:5.1} style {:2} spacing {:4.1} {:?} {asset:?} {}", text.margin, saved - width, saved, width, text.size, text.style, text.spacing, shown, node.path);
    }
    }
    Ok(())
}

// The fonts the build keeps whole (`fonts resources.assets`), and
// the font assets the health panel's texts are drawn with.
pub fn fonts(assets: &mut Assets, file: &str) -> Result<()> {
    for (name, font) in trp_data::interface::screen::build_fonts(&assets.install, file)? {
        println!("{name:30} {} bytes, {:?}", font.len(), String::from_utf8_lossy(&font[..4]));
        // What a font finder sees of it: its names (the family 1,
        // the subfamily 2, the full name 4, the typographic family
        // 16 and subfamily 17), its weight and its style.
        if let Ok(face) = ttf_parser::Face::parse(&font, 0) {
            let names: Vec<String> = face.names().into_iter().filter(|n| [1, 2, 4, 16, 17].contains(&n.name_id)).filter_map(|n| Some(format!("{}={:?}", n.name_id, n.to_string()?))).collect();
            println!("    names {}; weight {}, {:?}, italic {}, bold flag {}", names.join(", "), face.weight().to_number(), face.style(), face.is_italic(), face.is_bold());
        }
    }
    if let Ok(gear) = trp_data::interface::screen::GearPanel::load(&assets.install) {
        if let Ok(health) = trp_data::interface::screen::HealthPanel::load(&assets.install, &gear.file) {
            let mut seen = std::collections::BTreeSet::new();
            for node in health.panel.nodes.iter().chain(health.strip.iter().flat_map(|strip| strip.nodes.iter())) {
                let Some(text) = &node.text else { continue };
                if seen.insert(text.font) {
                    let outside = health.panel.externals.get(text.font.0 as usize - 1).cloned().unwrap_or_default();
                    println!("font asset {:?} in {outside}: {:?} (first by {})", text.font, trp_data::interface::screen::script_name(&assets.install, &outside, text.font.1), node.path);
                }
            }
        }
    }
    Ok(())
}
