//! Inspection tool for the install's bundles.
//!
//! ```text
//! probe find <text>                 manifest keys containing <text>
//! probe info <key>                  files, type-tree flag and object counts
//! probe names <key> [class]         objects with their names
//! probe dump <key> <path_id> [depth] [items]
//! probe tree <key>                  GameObject hierarchy with components
//! probe scripts <key>               MonoBehaviours with script class and owner
//! probe preset <weapon template id> stock build of a weapon with prefab bundles
//! probe fbx <file> [records|view|slice ...]   a binary FBX file: its models, a drawing, a slab
//! probe overlay <weapon> <fbx> <trigger model> <out.png>   a weapon with such a model over it
//! probe custom [id]                 the weapons that are not the install's, and one as built
//! ```

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use trp_data::unity::{class, Assets, ObjRef};
use trp_data::sketch::Drawn;
use trp_data::Install;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str);
    let install = Install::locate()?;
    let mut assets = Assets::new(install)?;

    match (arg(0), arg(1)) {
        // One of the build's own serialized files (`sharedassets44.assets`):
        // its engine version, whether it keeps its classes' type trees, and
        // how many objects of each class it holds.
        (Some("file"), Some(file)) => {
            let data = std::fs::read(assets.install.root.join("EscapeFromTarkov_Data").join(file))?;
            let meta = trp_data::unity::SerializedFile::parse(&data)?;
            println!("{file}: format {}, engine {}, type trees {}", meta.version, meta.unity_version, meta.has_type_trees);
            let mut classes: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
            for object in &meta.objects {
                *classes.entry(object.class_id).or_default() += 1;
            }
            for (class, count) in classes {
                println!("  class {class}: {count}");
            }
        }
        // The inventory screen's health panel as the app reads it, on a
        // screen 1920 by 1080 (or so wide: `healthpanel 2560`): the left
        // panel's size, then each object with its rectangle in it, whether
        // it shows, its sprite (with its texture's size and format) and
        // its text.
        // The game's window that shows a thing (`infowindow [width]`): each
        // of its objects as the prefab has it, placed in a screen that wide
        // and 1080 high by its own anchors, with its picture and its words.
        (Some("infowindow"), width) => {
            let window = trp_data::screen::info_window(&assets.install)?;
            let screen = [width.and_then(|w| w.parse().ok()).unwrap_or(1920.0), 1080.0];
            println!("{} ({} objects)", window.file, window.nodes.len());
            for (index, node) in window.nodes.iter().enumerate() {
                let [x, y, w, h] = window.placed_in(index, screen).rect;
                let mut line = format!("{:70} {}  ({x:.1}, {y:.1}) {w:.1} x {h:.1}", node.path, if window.shown(index) { "on " } else { "off" });
                if let Some(image) = &node.image {
                    line += &format!("  image {:?} kind {} sprite {:?}", image.color, image.kind, image.sprite);
                }
                if let Some(text) = &node.text {
                    line += &format!("  text {:?} size {} {:?} colour {:?} style {} align {}/{}", text.text, text.size, text.auto_size, text.color, text.style, text.horizontal, text.vertical);
                }
                let scripts: Vec<&str> = node.scripts.iter().map(|(name, _)| name.as_str()).collect();
                if !scripts.is_empty() {
                    line += &format!("  [{}]", scripts.join(", "));
                }
                println!("{line}");
            }
        }
        // Each font asset the GEAR tab's panels and the things' views draw
        // in (`fontscale`): its sampling size and scale, its lines, and the
        // width of a few words at 48 by its glyphs (TextMeshPro's measure)
        // beside the same words by its font file's own advances at an em of
        // 48 (the app's drawing measure), and their ratio.
        (Some("fontscale"), _) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let health = trp_data::screen::HealthPanel::load(&assets.install, &gear.file)?;
            let items = trp_data::screen::ItemViews::load(&assets.install)?;
            let mut seen = std::collections::BTreeSet::new();
            let panels: Vec<&trp_data::screen::ScenePanel> = [health.gear_panel.as_ref(), items.slot.as_ref(), items.grid.as_ref()].into_iter().flatten().collect();
            for panel in panels {
                for node in &panel.nodes {
                    let Some(text) = &node.text else { continue };
                    let Some(file) = panel.file_of(text.font.0) else { continue };
                    if std::env::var("FONTSCALE_TEXTS").is_ok() {
                        println!("text {:70} size {} style {} spacing {} kerning {} margin {:?} font {} material {:?}", node.path, text.size, text.style, text.spacing, text.kerning, text.margin, text.font.1, text.material);
                        continue;
                    }
                    if !seen.insert((file.clone(), text.font.1)) {
                        continue;
                    }
                    let tmp = trp_data::screen::TmpFont::read(&assets.install, &file, text.font.1)?;
                    let fonts = trp_data::screen::build_fonts(&assets.install, &file)?;
                    let name = trp_data::screen::script_name(&assets.install, &file, text.font.1).unwrap_or_default();
                    let ttf = trp_data::screen::font_for(&name, &fonts).or_else(|| trp_data::screen::font_by_face(&tmp.family, &tmp.style, &fonts));
                    println!("{name} ({file} {}): {} {}, sampled at {}, scale {}, ascent {} descent {}", text.font.1, tmp.family, tmp.style, tmp.point_size, tmp.scale, tmp.ascent, tmp.descent);
                    println!("  bold typeface {:?}, normal spacing {}, bold spacing {}", tmp.bold, tmp.normal_spacing, tmp.bold_spacing);
                    println!("  atlases {:?}, glyphs {}, M: {:?}", tmp.atlases, tmp.glyph_table.len(), tmp.glyph_of('M'));
                    let mut pictures = trp_data::screen::ScenePictures::new(&assets.install)?;
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
                    if let Ok((material_file, material)) = trp_data::screen::font_material(&assets.install, &file, text.font.1) {
                        if let Ok(props) = trp_data::screen::material_properties(&assets.install, &material_file, material) {
                            let mut floats: Vec<_> = props.floats.iter().filter(|(k, _)| ["_WeightNormal", "_WeightBold", "_FaceDilate", "_ScaleRatioA", "_ScaleRatioB", "_ScaleRatioC", "_GradientScale", "_OutlineWidth", "_OutlineSoftness", "_UnderlayDilate", "_UnderlaySoftness", "_UnderlayOffsetX", "_UnderlayOffsetY", "_Sharpness", "_TextureWidth", "_PerspectiveFilter"].contains(&k.as_str())).collect();
                            floats.sort_by(|a, b| a.0.cmp(b.0));
                            println!("  material {:?} keywords {:?}: {:?}", props.name, props.keywords, floats);
                            println!("  shader {:?}", trp_data::screen::material_shader(&assets.install, &material_file, material).map_err(|e| e.to_string()));
                        }
                    }
                    let Some((ttf_name, bytes)) = ttf else { println!("  no font file"); continue };
                    let face = ttf_parser::Face::parse(bytes, 0)?;
                    println!("  file {ttf_name}: {} units to the em, ascender {} descender {} line gap {}", face.units_per_em(), face.ascender(), face.descender(), face.line_gap());
                    for words in ["M", "ARMBAND", "ON SLING", "TACTICAL RIG", "AKM", "30"] {
                        let one = tmp.width(words, 48.0, 0.0, false) - tmp.normal_spacing * 48.0 / 100.0 * (words.chars().count().saturating_sub(1)) as f32;
                        let file_width = trp_data::screen::text_width(bytes, words, 48.0, false).unwrap_or(0.0);
                        println!("  {words:14} asset {one:8.2}  file {file_width:8.2}  ratio {:.4}", one / file_width.max(1e-3));
                    }
                }
            }
        }
        // Words drawn as the game draws them (`tmptext <words> <size>
        // <bold 0|1> <spacing> <kerning 0|1> <asset id> <pixels to a unit>
        // <out.png> [ground r,g,b]`): from the asset of that number in
        // resources.assets and its own material, laid on a plain ground, as
        // a PNG, with the box the words are aligned by.
        (Some("tmptext"), Some(words)) => {
            let num = |k: usize| arg(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let (size, bold, spacing, kerning, id, s) = (num(2), num(3) != 0.0, num(4), num(5) != 0.0, num(6) as i64, num(7));
            let out = arg(8).unwrap_or("local/tmptext.png");
            let ground: Vec<f32> = arg(9).unwrap_or("15,15,15").split(',').map(|v| v.parse::<f32>().unwrap_or(0.0) / 255.0).collect();
            let file = "resources.assets";
            let face_of = |id: i64| -> anyhow::Result<trp_data::tmp_text::TmpFace> {
                let font = trp_data::screen::TmpFont::read(&assets.install, file, id)?;
                let mut pictures = trp_data::screen::ScenePictures::new(&assets.install)?;
                let (atlas_file, atlas_id) = font.atlases.first().cloned().ok_or_else(|| anyhow::anyhow!("no atlas"))?;
                let atlas = pictures.texture(&atlas_file, atlas_id)?;
                let fonts = trp_data::screen::build_fonts(&assets.install, file)?;
                let name = trp_data::screen::script_name(&assets.install, file, id).unwrap_or_default();
                let ttf = trp_data::screen::font_for(&name, &fonts).or_else(|| trp_data::screen::font_by_face(&font.family, &font.style, &fonts)).map(|(_, bytes)| bytes.clone());
                Ok(trp_data::tmp_text::TmpFace::new(font, &atlas, ttf.as_deref()))
            };
            let mut face = face_of(id)?;
            if let Some((_, bold_id)) = face.font.bold.clone() {
                face.bold = Some(std::sync::Arc::new(face_of(bold_id)?));
            }
            let (material_file, material) = trp_data::screen::font_material(&assets.install, file, id)?;
            let material = trp_data::tmp_text::TmpMaterial::of(&trp_data::screen::material_properties(&assets.install, &material_file, material)?);
            let color = [0xb6 as f32 / 255.0, 0xc1 as f32 / 255.0, 0xc7 as f32 / 255.0, 1.0];
            let runs = [trp_data::tmp_text::TmpRun { text: words.to_owned(), size: size * s, color, bold }];
            let image = trp_data::tmp_text::draw(&face, &material, &runs, &trp_data::tmp_text::TmpLayout { em: size * s, spacing, kerning, wrap: None, align: 1 });
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
        }
        (Some("healthpanel"), width) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let health = trp_data::screen::HealthPanel::load(&assets.install, &gear.file)?;
            let screen = [width.and_then(|w| w.parse().ok()).unwrap_or(1920.0), 1080.0];
            let left = health.left_panel(screen);
            println!("{}: left panel {left:?}, the equipment panel at {:?}, spacing {}", gear.file, health.gear_in(left), health.spacing);
            let mut pictures = trp_data::screen::ScenePictures::new(&assets.install)?;
            for (index, node) in health.panel.nodes.iter().enumerate() {
                let placed = health.panel.placed(index, left);
                let [x, y, w, h] = placed.rect;
                let flip = match placed.flip { [false, false] => "", [true, false] => " mirrored", [false, true] => " upside down", [true, true] => " turned" };
                let mut line = format!("{:60} {}  ({x:.1}, {y:.1}) {w:.1} x {h:.1}{flip}", node.path, if health.panel.shown(index) { "on " } else { "off" });
                if let Some(image) = &node.image {
                    line += &format!("  image {:?} kind {} fill {}/{}/{}", image.color, image.kind, image.fill_method, image.fill_origin, image.fill_amount);
                    if let Some((file, id)) = image.sprite {
                        let name = if file == 0 { gear.file.clone() } else { health.panel.externals.get(file as usize - 1).cloned().unwrap_or_default() };
                        match pictures.sprite(&name, id) {
                            Ok(cut) => {
                                let texture = pictures.texture(&cut.texture.0, cut.texture.1).map(|t| format!("{}x{} format {}", t.width, t.height, t.format)).unwrap_or_else(|e| format!("{e:#}"));
                                line += &format!("  sprite {:?} {:?} border {:?} ppu {} covers {:?}{} of {texture}", cut.name, cut.rect, cut.border, cut.pixels_per_unit, cut.covers(), if cut.turned { " (turned in its atlas)" } else { "" });
                            }
                            Err(e) => line += &format!("  sprite {name} {id}: {e:#}"),
                        }
                    }
                }
                if let Some(text) = &node.text {
                    line += &format!("  text {:?} size {} {:?} colour {:?} style {} align {}/{}", text.text, text.size, text.auto_size, text.color, text.style, text.horizontal, text.vertical);
                }
                println!("{line}");
            }
            let path = |index: Option<usize>| index.map_or("-".to_owned(), |index| health.panel.nodes[index].path.clone());
            let views = &health.views;
            println!("\nsilhouette {}, legend {}", path(views.silhouette), path(views.legend));
            for part in &views.parts {
                println!("view {}: value {}, bar {}, border {}, shape {}, broken {}, effects {}", path(Some(part.view)), path(part.value), path(part.bar), path(part.border), path(part.shape), path(part.broken), path(part.effects));
            }
            if let Some(gear) = &health.gear_panel {
                println!("\nthe equipment panel's slot views:");
                let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| gear.nodes[index].path.clone());
                for (slot, parts) in &health.slot_views {
                    println!(
                        "{slot}: {} full {}, back {}, fill {}, empty {}, chosen {} {}, words {}, header {}",
                        at(Some(parts.view)), at(parts.full_border), at(parts.back_image), at(parts.background), at(parts.empty_border), at(parts.selected_border), at(parts.selected_corner), at(parts.header_text), at(parts.header)
                    );
                }
            }
            if let Some(strip) = &health.strip {
                println!("\nthe GEAR tab's strip of figures, in the left panel:");
                for (index, node) in strip.nodes.iter().enumerate() {
                    let [x, y, w, h] = strip.placed_in(index, left).rect;
                    let text = node.text.as_ref().map_or(String::new(), |text| format!("  text {:?} size {}", text.text, text.size));
                    println!("{:50} {}  ({x:.1}, {y:.1}) {w:.1} x {h:.1}{text}", node.path, if strip.shown(index) { "on " } else { "off" });
                }
                for (name, figure) in &health.strip_figures {
                    if let Some(f) = figure {
                        let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| strip.nodes[index].path.clone());
                        println!("{name}: {} current {}, max {}, ok glow {}, glow {}, icon {}, warning {:?}, alternative {:?}", at(Some(f.panel)), at(f.current), at(f.max), at(f.ok_glow), at(f.glow), at(f.icon), f.warning, f.alternative);
                    }
                }
            }
            for (name, figure) in &views.parameters {
                match figure {
                    Some(f) => println!(
                        "{name}: {} current {}, max {}, ok glow {}, glow {}, icon {}, warning {:?}, alternative {:?}, digits {} noughts {:?}, rate {} {} {}",
                        path(Some(f.panel)), path(f.current), path(f.max), path(f.ok_glow), path(f.glow), path(f.icon), f.warning, f.alternative, f.least_digits, f.noughts, path(f.up), path(f.down), path(f.rate)
                    ),
                    None => println!("{name}: none"),
                }
            }
        }
        // The animation clips of one of the build's own files, read by the
        // tree of a clip in a bundle of the install's (`sceneclips
        // sharedassets44.assets assets/content/weapons/animations/simple_animations.bundle
        // [part of a name]`): each curve that drives a property of a
        // component, with its values at the clip's start and end, the
        // object's path and the property named where they are among the
        // health panel's objects and the screen's usual properties.
        (Some("sceneclips"), Some(file)) => {
            let donor = arg(2).ok_or_else(|| anyhow::anyhow!("a bundle that has a clip"))?;
            let wanted = arg(3).unwrap_or("").to_lowercase();
            let tree = trp_data::screen::tree_of(&assets.install, donor, trp_data::unity::class::ANIMATION_CLIP)?;
            let mut files = trp_data::screen::ScenePictures::new(&assets.install)?;
            // The names the clips' numbers may be for: every object's path
            // below a view of the health panel, and below the panel.
            let mut paths: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            if let Ok(gear) = trp_data::screen::GearPanel::load(&assets.install) {
                if let Ok(health) = trp_data::screen::HealthPanel::load(&assets.install, &gear.file) {
                    for node in &health.panel.nodes {
                        let mut path = node.path.as_str();
                        loop {
                            paths.insert(trp_data::unity::anim::path_hash(path), path.to_owned());
                            match path.split_once('/') {
                                Some((_, rest)) => path = rest,
                                None => break,
                            }
                        }
                    }
                }
            }
            paths.insert(trp_data::unity::anim::path_hash(""), "(the root)".to_owned());
            let properties: std::collections::HashMap<u32, &str> = ["m_Color.r", "m_Color.g", "m_Color.b", "m_Color.a", "m_IsActive", "m_Enabled", "m_FillAmount", "m_LocalScale.x", "m_LocalScale.y", "m_AnchoredPosition.x", "m_AnchoredPosition.y", "m_SizeDelta.x", "m_SizeDelta.y", "m_Alpha", "m_fontColor.a"]
                .iter()
                .map(|name| (trp_data::unity::anim::path_hash(name), *name))
                .collect();
            for id in files.objects(file, trp_data::unity::class::ANIMATION_CLIP)? {
                let clip = match files.read_by(file, id, trp_data::unity::class::ANIMATION_CLIP, &tree).and_then(|v| trp_data::unity::anim::Clip::of(&v)) {
                    Ok(clip) => clip,
                    Err(e) => {
                        println!("clip {id}: {e:#}");
                        continue;
                    }
                };
                if !clip.name.to_lowercase().contains(&wanted) {
                    continue;
                }
                // Its values at nine times from its start to its end.
                let samples: Vec<Vec<f32>> = (0..9)
                    .map(|n| {
                        let mut values = Vec::new();
                        clip.sample(clip.duration() * n as f32 / 8.0, &mut values);
                        values
                    })
                    .collect();
                println!("clip {id} {:?}: {:.3} s{}", clip.name, clip.duration(), if clip.looping { ", looping" } else { "" });
                for b in &clip.others {
                    let path = paths.get(&b.path).cloned().unwrap_or_else(|| format!("#{:08x}", b.path));
                    let property = properties.get(&b.attribute).map_or_else(|| format!("#{:08x}", b.attribute), |name| name.to_string());
                    let values: Vec<String> = samples.iter().map(|values| format!("{:.3}", values.get(b.curve).copied().unwrap_or(f32::NAN))).collect();
                    println!("  {path} (class {}) {property}: {}", b.class, values.join(" "));
                }
            }
        }
        // The smallest bundles of the install's that keep a class with its
        // tree (`classfind 74`), smallest first, ten of them: where to
        // borrow the tree to read the build's own files by.
        (Some("classfind"), Some(class)) => {
            let class: i32 = class.parse()?;
            let mut keys: Vec<(u64, String)> = assets
                .manifest
                .entries
                .keys()
                .filter_map(|key| Some((std::fs::metadata(assets.install.bundle_path(key)).ok()?.len(), key.clone())))
                .collect();
            keys.sort();
            let mut found = 0;
            for (size, key) in keys {
                let Ok(bundle) = trp_data::unity::Bundle::open(&assets.install.bundle_path(&key)) else { continue };
                let has = bundle.nodes.iter().filter(|node| node.is_serialized_file()).any(|node| {
                    trp_data::unity::SerializedFile::parse(bundle.file(node)).is_ok_and(|meta| meta.types.iter().any(|ty| ty.class_id == class && !ty.tree.is_empty()))
                });
                if has {
                    println!("{size:>10}  {key}");
                    found += 1;
                    if found == 10 {
                        break;
                    }
                }
            }
        }
        // Any object of a scene and all under it, as the app reads a panel
        // (`scenepanel level44 "Left Panel/Gear Panel" 494 650`): each
        // object's rectangle in the top's of that size (its own size where
        // none is given), whether it is on, its scripts, its picture and
        // its text.
        (Some("scenepanel"), Some(file)) => {
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let end = arg(2).unwrap_or("");
            let top = (0..scene.nodes.len()).find(|&index| scene.path(index).ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
            let panel = trp_data::screen::ScenePanel::of(&scene, top);
            let own = panel.nodes[0].layout.size_delta;
            let size = [arg(3).and_then(|w| w.parse().ok()).unwrap_or(own[0]), arg(4).and_then(|h| h.parse().ok()).unwrap_or(own[1])];
            println!("{}: {size:?}", scene.path(top));
            let mut pictures = trp_data::screen::ScenePictures::new(&assets.install)?;
            for (index, node) in panel.nodes.iter().enumerate() {
                let placed = panel.placed(index, size);
                let [x, y, w, h] = placed.rect;
                let classes: Vec<&str> = node.scripts.iter().map(|(class, _)| class.as_str()).collect();
                let mut line = format!("{:55} {} ({x:.1}, {y:.1}) {w:.1} x {h:.1} [{}]", node.path, if node.active { "on " } else { "off" }, classes.join(", "));
                if node.layout.turn.abs() > 1e-3 {
                    line += &format!("  turned {:.1}", node.layout.turn);
                }
                if let Some(text) = node.text.as_ref().filter(|text| text.margin.iter().any(|&m| m != 0.0)) {
                    line += &format!("  margin {:?}", text.margin);
                }
                if let Some(group) = node.group {
                    line += &format!("  row {group:?}");
                }
                if let Some(element) = node.element {
                    line += &format!("  asks {element:?}");
                }
                if let Some(image) = &node.image {
                    line += &format!("  image {:?} kind {}", image.color, image.kind);
                    if let Some((outside, id)) = image.sprite {
                        let name = if outside == 0 { file.to_owned() } else { panel.externals.get(outside as usize - 1).map(|n| n.rsplit('/').next().unwrap_or(n).to_owned()).unwrap_or_default() };
                        match pictures.sprite(&name, id) {
                            Ok(cut) => {
                                line += &format!(" sprite {:?} {:?} border {:?}", cut.name, cut.rect, cut.border);
                                // `PROBE_SPRITES=<folder>`: each sprite
                                // written there as it is, its top up.
                                if let Ok(folder) = std::env::var("PROBE_SPRITES") {
                                    let texture = pictures.texture(&cut.texture.0, cut.texture.1)?;
                                    let levels = trp_data::unity::texture::mips(&texture)?;
                                    let pixels = trp_data::unity::texture::decode_rgba(&texture, *levels.first().ok_or_else(|| anyhow::anyhow!("no levels"))?)?;
                                    let [x, y, w, h] = cut.rect.map(|v| v.round().max(0.0) as usize);
                                    let width = texture.width as usize;
                                    // `PROBE_MARGIN=<texels>`: that much of the
                                    // texture around it too, and the alpha
                                    // just outside its left and right edges
                                    // on its middle row.
                                    let margin: usize = std::env::var("PROBE_MARGIN").ok().and_then(|m| m.parse().ok()).unwrap_or(0);
                                    let row_mid = y + h / 2;
                                    let alpha = |column: Option<usize>| column.filter(|&c| c < width).map_or("-".to_owned(), |c| pixels[(row_mid * width + c) * 4 + 3].to_string());
                                    if margin > 0 {
                                        line += &format!(" (alpha outside: left {}, right {}; inside: {} .. {})", alpha(x.checked_sub(1)), alpha(Some(x + w)), alpha(Some(x)), alpha(Some(x + w - 1)));
                                    }
                                    let (x, y) = (x.saturating_sub(margin), y.saturating_sub(margin));
                                    let (w, h) = ((w + 2 * margin).min(width - x), (h + 2 * margin).min(texture.height as usize - y));
                                    let mut out = Vec::with_capacity(w * h * 4);
                                    for row in (y..y + h).rev() {
                                        out.extend_from_slice(&pixels[(row * width + x) * 4..(row * width + x + w) * 4]);
                                    }
                                    line += &format!(" (texture {} by {}, {} levels, format {}, srgb {})", texture.width, texture.height, levels.len(), texture.format, texture.srgb);
                                    let path = std::path::Path::new(&folder).join(format!("{}.png", cut.name));
                                    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&path)?), w as u32, h as u32);
                                    encoder.set_color(png::ColorType::Rgba);
                                    encoder.set_depth(png::BitDepth::Eight);
                                    encoder.write_header()?.write_image_data(&out)?;
                                }
                            }
                            Err(e) => line += &format!(" sprite {name} {id}: {e:#}"),
                        }
                    }
                }
                if let Some(text) = &node.text {
                    line += &format!("  text {:?} size {} colour {:?} align {}/{} style {} font {:?}", text.text, text.size, text.color, text.horizontal, text.vertical, text.style, text.font);
                }
                println!("{line}");
            }
        }
        // A prefab of the build's own Resources by its path
        // (`prefab prefabs/ugui/layouts/slot_layout 125 125`), as
        // `scenepanel` prints a part of a scene.
        // The inventory's quick-use panel as the app reads it, its views on
        // as its script shows them: each view's binding, what it names and
        // where it lies on a screen 1920 by 1080.
        (Some("battle"), _) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let views = trp_data::screen::BattleViews::load(&assets.install, &gear.file)?;
            let stance = |index: Option<usize>| index.map_or("-".to_owned(), |index| views.stance.nodes[index].path.clone());
            let health = |index: Option<usize>| index.map_or("-".to_owned(), |index| views.health.nodes[index].path.clone());
            for (object, factor) in &views.figures {
                println!("figure {} at {factor}", stance(Some(*object)));
            }
            println!("noise {} sprites {:?}", stance(views.noise), views.noise_sprites);
            println!("notch {} final {} row {} colours {:?}", stance(views.notch_template), stance(views.final_notch), stance(views.notches_place), views.notch_colors);
            for slider in [views.stance_slider, views.speed_slider].into_iter().flatten() {
                println!("slider {} handle {} area {} direction {} from {} to {} at {}", stance(Some(slider.object)), stance(slider.handle), stance(slider.area), slider.direction, slider.min, slider.max, slider.value);
                let size = views.stance.nodes[0].layout.size_delta;
                println!("  scene {:?}, at 0 {:?}, at 1 {:?}, at the scene's value {:?}", slider.handle.map(|handle| views.stance.rect(handle, size)), slider.handle_at(&views.stance, size, 0.0), slider.handle_at(&views.stance, size, 1.0), slider.handle_at(&views.stance, size, trp_data::screen::SliderParts::share(slider.value, slider.min, slider.max)));
            }
            println!("fills {} {} sprites {:?}", stance(views.legs_fill), stance(views.arms_fill), views.fill_sprites);
            println!("ground {}", health(views.background));
            for (part, image) in &views.parts {
                println!("part {part}: {}", health(Some(*image)));
            }
            println!("effects {} grid {:?} icon {} count {}", health(views.effects), views.effects_grid, health(views.effect_template), health(views.stack_count));
            let quick = trp_data::screen::QuickViews::load_battle(&assets.install, &gear.file)?;
            let rect = quick.quick_in([1920.0, 1080.0]);
            println!("quick bar {rect:?}");
            let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| quick.panel.nodes[index].path.clone());
            for slot in &quick.slots {
                println!("  {} {}: {:?} install {:?}, key {}, caption {}, border {}, ground {}, arrow {}", slot.key, at(Some(slot.object)), quick.panel.rect(slot.object, [rect[2], rect[3]]), slot.install.map(|i| quick.panel.rect(i, [rect[2], rect[3]])), at(slot.hot_key), at(slot.caption), at(slot.border), at(slot.background), at(slot.arrow));
            }
            for (index, node) in views.stance.nodes.iter().enumerate().filter(|(_, node)| node.path.contains("Handle")) {
                println!("{} image {:?}", node.path, node.image.map(|image| image.sprite));
                let _ = index;
            }
        }
        (Some("quick"), _) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let mut quick = trp_data::screen::QuickViews::load(&assets.install, &gear.file)?;
            for slot in quick.slots.clone() {
                quick.panel.nodes[slot.object].active = true;
            }
            let rect = quick.quick_in([1920.0, 1080.0]);
            println!("the panel {rect:?}");
            let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| quick.panel.nodes[index].path.clone());
            for slot in &quick.slots {
                println!("{} {}: {:?}  install {}, key {}, caption {}, border {}", slot.key, at(Some(slot.object)), quick.panel.rect(slot.object, [rect[2], rect[3]]), at(slot.install), at(slot.hot_key), at(slot.caption), at(slot.border));
            }
            for name in ["Header", "Header/Text", "Quick Slots"] {
                if let Some(index) = quick.panel.find(name) {
                    println!("{name}: {:?}", quick.panel.rect(index, [rect[2], rect[3]]));
                }
            }
        }
        // The inventory's panel for a container opened, as the app reads it:
        // what its script names, its filters' tabs and its scrolling.
        (Some("stash"), _) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let stash = trp_data::screen::StashViews::load(&assets.install, &gear.file)?;
            let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| stash.panel.nodes[index].path.clone());
            println!("simple {}, header {}, grid name {}, filters {}, money {}, name panel {}, name {}", at(Some(stash.simple)), at(stash.header), at(stash.grid_name), at(stash.filters), at(stash.money), at(stash.name_panel), at(stash.name));
            println!("lobby only {:?}", stash.lobby_only.iter().map(|&index| at(Some(index))).collect::<Vec<_>>());
            println!("sort {}, sorting table {}, search {}, scroll {}, external {}, pin {}, lock {}, priority {}, grids {}", at(stash.sort), at(stash.sorting_table), at(stash.search), at(stash.scroll), at(stash.external), at(stash.pin), at(stash.lock), at(stash.priority), at(stash.grids));
            println!("scrolling {:?}", stash.scrolling);
            for tab in &stash.filter_tabs {
                println!("  filter {}: normal {}, selected {}, target {}, icon {}", at(Some(tab.object)), at(tab.normal), at(tab.selected), at(tab.target), at(tab.icon));
            }
            let size = stash.panel.nodes[0].layout.size_delta;
            for (index, node) in stash.panel.nodes.iter().enumerate().filter(|(_, node)| node.path.contains("Filter Panel/") && node.path.matches('/').count() == 3) {
                println!("  {} {:?}", node.path, stash.panel.rect(index, size));
            }
        }
        // Where a script's pointers lead in its panel (`points level44
        // "InventoryScreen/Items Panel/Stash Panel/Simple Panel"
        // SimpleStashPanel`): each place in its fields that names an
        // object of the panel, a script on one, or one's place.
        (Some("points"), Some(file)) => {
            let wanted = arg(2).ok_or_else(|| anyhow::anyhow!("which object?"))?;
            let class = arg(3).ok_or_else(|| anyhow::anyhow!("which script?"))?;
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(wanted)).ok_or_else(|| anyhow::anyhow!("no {wanted}"))?;
            let panel = trp_data::screen::ScenePanel::of(&scene, top);
            let fields = panel.script(0, class).ok_or_else(|| anyhow::anyhow!("no {class} on it"))?;
            println!("{wanted} {class}: {} bytes of fields", fields.len());
            for at in (0..fields.len().saturating_sub(11)).step_by(4) {
                let object = panel.pointer_at(fields, at).map(|index| ("object or script", index));
                let place = panel.transform_at(fields, at).map(|index| ("place", index));
                if let Some((what, index)) = object.or(place) {
                    println!("  {at:5}: {what} {}", panel.nodes[index].path);
                }
            }
        }
        // One object of a build's file by its number, its bytes as words
        // from a place on (`raw sharedassets0.assets 1234 0 64`), each
        // as a number, a float and its letters.
        // A TextMeshPro font asset's material and its floats and colours
        // (`fontmaterial resources.assets 34143`).
        // Each object whose path ends so, and those under it, with its
        // parts' engine classes (223 a canvas: then its fields, which say
        // whether it sorts on its own and by what order).
        (Some("parts"), Some(file)) => {
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let end = arg(2).unwrap_or("");
            let top = (0..scene.nodes.len()).find(|&index| scene.path(index).ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
            let top_path = scene.path(top);
            for index in (0..scene.nodes.len()).filter(|&index| scene.path(index).starts_with(&top_path)) {
                let parts = scene.parts_of(index);
                let classes: Vec<String> = parts.iter().map(|(_, class)| class.to_string()).collect();
                println!("{} [{}]", scene.path(index).trim_start_matches(&top_path), classes.join(" "));
                for (id, class) in parts.iter().filter(|(_, class)| *class == 223) {
                    let (_, bytes) = trp_data::screen::object_bytes(&assets.install, file, *id)?;
                    let words: Vec<String> = bytes.chunks(4).map(|w| format!("{:?}", w)).collect();
                    println!("    canvas {class} {id}: {}", words.join(" "));
                }
            }
        }
        // The interface's sounds (`Resources/Audio/UISoundsWrapper`): each
        // sound's kind and its clip's name, length and how it is kept.
        (Some("uisounds"), _) => {
            let paths = trp_data::screen::resource_paths(&assets.install)?;
            let (_, file, id) = paths.iter().find(|(path, ..)| path == "audio/uisoundswrapper").ok_or_else(|| anyhow::anyhow!("no audio/uisoundswrapper"))?;
            let (class, bytes) = trp_data::screen::object_bytes(&assets.install, file, *id)?;
            println!("{file} {id}: class {class}, {} bytes", bytes.len());
            let tree = trp_data::screen::tree_of(&assets.install, "assets/content/audio/itemsounds/itemsounds.bundle", 83)?;
            let mut pictures = trp_data::screen::ScenePictures::new(&assets.install)?;
            // After the object, its switch, its script and its name: the
            // list of kinds and clips.
            let mut at = 12 + 4 + 12;
            let name_len = u32::from_le_bytes(bytes[at..at + 4].try_into()?) as usize;
            at += 4 + name_len;
            at = (at + 3) & !3;
            let count = u32::from_le_bytes(bytes[at..at + 4].try_into()?) as usize;
            at += 4;
            for n in 0..count {
                let kind = i32::from_le_bytes(bytes[at + 16 * n..at + 16 * n + 4].try_into()?);
                let clip_file = i32::from_le_bytes(bytes[at + 16 * n + 4..at + 16 * n + 8].try_into()?);
                let clip_id = i64::from_le_bytes(bytes[at + 16 * n + 8..at + 16 * n + 16].try_into()?);
                let said = if clip_file == 0 { pictures.read_by(file, clip_id, 83, &tree).map(|v| format!("{:?} {:?}s format {:?} {:?}", v.get("m_Name").and_then(|n| n.as_str()).unwrap_or(""), v.get("m_Length").and_then(|l| l.as_f32()), v.get("m_CompressionFormat").and_then(|f| f.as_i64()), v.get("m_Resource").and_then(|r| r.get("m_Source")).and_then(|s| s.as_str()))).unwrap_or_else(|e| format!("{e:#}")) } else { format!("in file {clip_file}") };
                println!("  kind {kind}: {said}");
            }
        }
        // The sounds of moving (`sound::movement_sounds`): each surface's
        // banks with their figures and how many clips each list has; with
        // `read`, every clip decoded too and a count of those not read; with
        // a folder after `read`, the clips of the surfaces given (by number,
        // `stepsounds read local\x 0 2`) written there as WAVE files.
        (Some("stepsounds"), read) => {
            let read = read.as_deref() == Some("read");
            let only: Vec<u8> = (3..16).filter_map(|n| arg(n).and_then(|s| s.parse().ok())).collect();
            let sounds = trp_data::sound::movement_sounds(&assets.install, (!only.is_empty()).then_some(only.as_slice()), read)?;
            let say = |bank: &trp_data::sound::Bank| {
                format!(
                    "{:28} volume {:.2} -{:.2}, pitch {:.2} +-{:.2}, rolloff {:5.1}, blend {:?}, clips {:?}",
                    bank.name,
                    bank.base_volume,
                    bank.delta_volume,
                    bank.base_pitch,
                    bank.delta_pitch,
                    bank.rolloff,
                    bank.blend,
                    bank.lists.iter().map(Vec::len).collect::<Vec<_>>()
                )
            };
            for set in &sounds.surfaces {
                println!("{} ({})", trp_data::sound::SURFACES.get(set.surface as usize).unwrap_or(&"?"), set.surface);
                for (name, bank) in trp_data::sound::SURFACE_BANKS.iter().zip(&set.banks) {
                    println!("  {name:20} {}", bank.map_or("none".to_owned(), |bank| say(&sounds.banks[bank])));
                }
            }
            for (name, bank) in trp_data::sound::GEAR_BANKS.iter().zip(&sounds.gear) {
                println!("{name:22} {}", bank.map_or("none".to_owned(), |bank| say(&sounds.banks[bank])));
            }
            let mut kinds: std::collections::BTreeMap<(u16, u32), (usize, f32)> = std::collections::BTreeMap::new();
            for clip in sounds.clips.iter().filter_map(|(_, clip)| clip.as_ref()) {
                let entry = kinds.entry((clip.channels, clip.frequency)).or_default();
                entry.0 += 1;
                entry.1 = entry.1.max(clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32);
            }
            for ((channels, rate), (count, longest)) in &kinds {
                println!("  {count} clips of {channels} channels at {rate} Hz, the longest {longest:.2} s");
            }
            let decoded = sounds.clips.iter().filter(|(_, clip)| clip.is_some()).count();
            let frames: usize = sounds.clips.iter().filter_map(|(_, clip)| clip.as_ref()).map(|clip| clip.samples.len()).sum();
            println!("{} clips named, {decoded} read ({:.1} MB of 16-bit samples)", sounds.clips.len(), frames as f64 * 2.0 / 1e6);
            if let Some(folder) = arg(2).filter(|_| read) {
                let folder = std::path::Path::new(folder);
                std::fs::create_dir_all(folder)?;
                for (name, clip) in &sounds.clips {
                    if let Some(clip) = clip {
                        std::fs::write(folder.join(format!("{name}.wav")), clip.wav())?;
                    }
                }
            }
        }
        // The sounds as read (`sound`): the interface's of the kinds given
        // and the things' of a group, each written as a WAVE file to the
        // folder given (to listen to), with its length.
        // A hands rig's sound player (`BaseSoundPlayer`): each of its
        // events with its rolloff, volume and clips. An item that has
        // hands of its own by its template (its `UsePrefab`), else a weapon.
        (Some("handsounds"), Some(what)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let key = db.items.get(what).and_then(|item| item.props["UsePrefab"]["path"].as_str().filter(|path| !path.is_empty()).map(str::to_owned)).unwrap_or_else(|| what.to_owned());
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, &key)?;
            let mut setups = trp_data::sound::VorbisSetups::new(&assets.install);
            match trp_data::sound::hands_sounds(&assets, &weapon, &mut setups, true)? {
                Some(sounds) => {
                    println!("{key}: {}", sounds.player);
                    for (list, elements) in [("main", &sounds.main), ("additional", &sounds.additional)] {
                        for element in elements {
                            let clips: Vec<String> = element.clips.iter().map(|clip| format!("{} ({:.2} s, x{})", clip.name, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32, clip.channels)).collect();
                            println!("  {list} {}: rolloff {}, volume {}, {}", element.event, element.rolloff, element.volume, clips.join(", "));
                        }
                    }
                    // Its shots (`WeaponSoundPlayer`): each bank's figures
                    // and its three lists by distance.
                    if let Some(shots) = &sounds.shots {
                        println!("  shots: {}", if shots.non_auto { "each whole (Non_auto)" } else { "automatic: a loop and a tail" });
                        for (name, bank) in trp_data::sound::SHOT_BANKS.iter().zip(&shots.banks) {
                            let Some(bank) = bank else {
                                println!("  {name}: none");
                                continue;
                            };
                            println!("  {name} = {}: volume {}, rolloff {}, length {:.4} s, blend {:?}", bank.name, bank.base_volume, bank.rolloff, bank.clip_length, bank.blend);
                            for (n, list) in bank.lists.iter().enumerate() {
                                let clips: Vec<String> = list
                                    .iter()
                                    .map(|&at| match &shots.clips[at] {
                                        (name, Some(clip)) => format!("{name} ({:.4} s, x{}, {} Hz)", clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32, clip.channels, clip.frequency),
                                        (name, None) => format!("{name} (not read)"),
                                    })
                                    .collect();
                                println!("    distance {n}: {}", clips.join(", "));
                            }
                        }
                        let bytes: usize = shots.clips.iter().filter_map(|(_, clip)| clip.as_ref()).map(|clip| clip.samples.len() * 2).sum();
                        println!("  {} clips, {:.1} MB decoded", shots.clips.len(), bytes as f32 / 1e6);
                        // With a folder (`handsounds <weapon> <folder>`), each
                        // clip written there as a WAVE file, with its peak.
                        if let Some(folder) = arg(2) {
                            let folder = std::path::Path::new(folder);
                            std::fs::create_dir_all(folder)?;
                            for clip in shots.clips.iter().filter_map(|(_, clip)| clip.as_ref()) {
                                let peak = clip.samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
                                let rms = (clip.samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / clip.samples.len().max(1) as f64).sqrt();
                                println!("  {}: peak {peak}, rms {rms:.0}", clip.name);
                                std::fs::write(folder.join(format!("{}.wav", clip.name)), clip.wav())?;
                            }
                        }
                    }
                }
                None => println!("{key}: no sound player"),
            }
        }
        // The things' sounds kept as Vorbis: each sample's channels,
        // frequency, frames, packets and the CRC32 of its setup header,
        // and whether that header was found in the game's engine.
        (Some("vorbis"), _) => {
            let clips = trp_data::sound::item_vorbis(&assets.install)?;
            let wanted: Vec<u32> = clips.iter().map(|(_, sample)| sample.setup_crc).collect();
            let headers = trp_data::sound::vorbis_setup_headers(&assets.install, &wanted)?;
            // With a folder, each decoded and written there as a WAVE file.
            let folder = arg(1).map(std::path::PathBuf::from);
            for (name, sample) in &clips {
                let found = headers.get(&sample.setup_crc).map_or("not found".to_owned(), |header| format!("found, {} bytes", header.len()));
                println!("{name}: {} Hz x{}, {} frames, {} packets ({} bytes), setup header {:08x} {found}", sample.frequency, sample.channels, sample.frames, sample.packets.len(), sample.packets.iter().map(Vec::len).sum::<usize>(), sample.setup_crc);
                let (Some(folder), Some(setup)) = (&folder, headers.get(&sample.setup_crc)) else { continue };
                match trp_data::sound::decode_vorbis(sample, setup) {
                    Ok(samples) => {
                        let ch = sample.channels.max(1) as usize;
                        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
                        let level: f64 = samples.iter().map(|s| (*s as f64).abs()).sum::<f64>() / samples.len().max(1) as f64;
                        let jumps: f64 = samples.windows(ch + 1).map(|w| (w[ch] as f64 - w[0] as f64).abs()).sum::<f64>() / samples.len().max(1) as f64;
                        println!("  decoded: {} frames of {}, peak {peak}, roughness {:.2}", samples.len() / ch, sample.frames, jumps / level.max(1.0));
                        std::fs::create_dir_all(folder)?;
                        let clip = trp_data::sound::Clip { name: name.clone(), channels: sample.channels, frequency: sample.frequency, samples };
                        std::fs::write(folder.join(format!("{name}.wav")), clip.wav())?;
                    }
                    Err(e) => println!("  not decoded: {e:#}"),
                }
            }
        }
        (Some("sounds"), Some(folder)) => {
            let folder = std::path::Path::new(folder);
            std::fs::create_dir_all(folder)?;
            let ui = trp_data::sound::ui_sounds(&assets.install, &(0..64).collect::<Vec<i32>>())?;
            let items = trp_data::sound::item_sounds(&assets.install)?;
            let group = arg(2).unwrap_or("weap_ar");
            let mut all: Vec<(String, &trp_data::sound::Clip)> = ui.iter().map(|(kind, clip)| (format!("ui{kind}"), clip)).collect();
            all.extend(items.iter().filter(|(name, _)| name.starts_with(group)).map(|(name, clip)| (name.clone(), clip)));
            for (name, clip) in &all {
                std::fs::write(folder.join(format!("{name}_{}.wav", clip.name)), clip.wav())?;
                let ch = clip.channels as usize;
                let level: f64 = clip.samples.iter().map(|s| (*s as f64).abs()).sum::<f64>() / clip.samples.len().max(1) as f64;
                let jumps: f64 = clip.samples.windows(ch + 1).map(|w| (w[ch] as f64 - w[0] as f64).abs()).sum::<f64>() / clip.samples.len().max(1) as f64;
                println!("{name}: {} {} Hz x{}, {:.3} s, peak {}, roughness {:.2}", clip.name, clip.frequency, clip.channels, clip.samples.len() as f32 / clip.channels as f32 / clip.frequency as f32, clip.samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0), jumps / level.max(1.0));
            }
            println!("the things' sounds: {} clips", items.len());
        }
        // The pointers of a scene object's script into other files, each
        // with the script it points to (`scriptptrs level49 UIContext ItemUiContext`).
        (Some("scriptptrs"), Some(file)) => {
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let end = arg(2).unwrap_or("");
            let class = arg(3).unwrap_or("");
            let top = (0..scene.nodes.len()).find(|&index| scene.path(index).ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
            let panel = trp_data::screen::ScenePanel::of(&scene, top);
            let fields = panel.script(0, class).ok_or_else(|| anyhow::anyhow!("no {class} there"))?;
            for start in (0..fields.len().saturating_sub(11)).step_by(4) {
                let to_file = i32::from_le_bytes(fields[start..start + 4].try_into()?);
                let to = i64::from_le_bytes(fields[start + 4..start + 12].try_into()?);
                if (1..64).contains(&to_file) && to > 0 && to < 1 << 40 {
                    let name = panel.file_of(to_file).unwrap_or_default();
                    let script = trp_data::screen::script_name(&assets.install, &name, to).unwrap_or_default();
                    println!("  at {start}: {name} {to} {script}");
                }
            }
        }
        // Each text of the views of a thing and of the inventory's panels:
        // its font asset's own material, and the material it is drawn
        // with, where that is another.
        (Some("textmats"), _) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let items = trp_data::screen::ItemViews::load(&assets.install)?;
            let health = trp_data::screen::HealthPanel::load(&assets.install, &gear.file)?;
            let quick = trp_data::screen::QuickViews::load(&assets.install, &gear.file)?;
            let panels: Vec<(&str, &trp_data::screen::ScenePanel)> = [("slot view", items.slot.as_ref()), ("grid view", items.grid.as_ref()), ("health", Some(&health.panel)), ("quick", Some(&quick.panel))].into_iter().filter_map(|(name, panel)| Some((name, panel?))).collect();
            for (name, panel) in panels {
                for node in &panel.nodes {
                    let Some(text) = &node.text else { continue };
                    let Some(font_file) = panel.file_of(text.font.0) else { continue };
                    let own = trp_data::screen::font_material(&assets.install, &font_file, text.font.1).ok();
                    let used = text.material.and_then(|(outside, id)| Some((panel.file_of(outside)?, id)));
                    let say = |which: &Option<(String, i64)>| which.as_ref().and_then(|(file, id)| trp_data::screen::material_properties(&assets.install, file, *id).ok()).map_or("-".to_owned(), |props| format!("{} {:?} outline {:?} underlay {:?}", props.name, props.keywords, props.floats.get("_OutlineWidth"), props.floats.get("_UnderlayDilate")));
                    println!("{name}: {} {:?} size {} font {} {}\n    font's {}\n    drawn {}{}", node.path, text.text, text.size, font_file, text.font.1, say(&own), say(&used), if used == own { "  (the same)" } else { "" });
                }
            }
        }
        (Some("fontmaterial"), Some(file)) => {
            let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("which font asset?"))?.parse()?;
            let (material_file, material) = trp_data::screen::font_material(&assets.install, file, id)?;
            let props = trp_data::screen::material_properties(&assets.install, &material_file, material)?;
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
        }
        // The files a build's file points into, by their numbers in its
        // pointers (`externals resources.assets`: 1 is the first).
        (Some("externals"), Some(file)) => {
            let path = assets.install.root.join("EscapeFromTarkov_Data").join(file);
            let data = std::fs::read(&path)?;
            let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
            for (at, name) in meta.externals.iter().enumerate() {
                println!("{} {name}", at + 1);
            }
        }
        // An object's bytes from one place on, LZ4 blocks unpacked, as
        // words (`unpack globalgamemanagers.assets 284 3024 710 1464`: a
        // shader's compiled programs).
        (Some("unpack"), Some(file)) => {
            let number = |at: usize| -> Result<usize> { Ok(arg(at).ok_or_else(|| anyhow::anyhow!("unpack takes an object, an offset and two lengths"))?.parse()?) };
            let (id, from, packed, unpacked) = (number(2)? as i64, number(3)?, number(4)?, number(5)?);
            let (_, bytes) = trp_data::screen::object_bytes(&assets.install, file, id)?;
            let out = lz4_flex::block::decompress(&bytes[from..from + packed], unpacked)?;
            if let Some(path) = arg(6) {
                std::fs::write(path, &out)?;
            }
            for at in (0..out.len()).step_by(4) {
                let Some(word) = out.get(at..at + 4) else { break };
                let word: [u8; 4] = word.try_into()?;
                let letters: String = word.iter().map(|&b| if b.is_ascii_graphic() { b as char } else { '.' }).collect();
                println!("  {at:6}: {:08x} {:11} {:14} {letters}", u32::from_le_bytes(word), i32::from_le_bytes(word), f32::from_le_bytes(word));
            }
        }
        (Some("raw"), Some(file)) => {
            let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("which object?"))?.parse()?;
            let from: usize = arg(3).and_then(|n| n.parse().ok()).unwrap_or(0);
            let count: usize = arg(4).and_then(|n| n.parse().ok()).unwrap_or(64);
            let (class, bytes) = trp_data::screen::object_bytes(&assets.install, file, id)?;
            println!("{file} {id}: class {class}, {} bytes", bytes.len());
            for at in (from..bytes.len().min(from + 4 * count)).step_by(4) {
                let Some(word) = bytes.get(at..at + 4) else { break };
                let word: [u8; 4] = word.try_into()?;
                let letters: String = word.iter().map(|&b| if b.is_ascii_graphic() { b as char } else { '.' }).collect();
                println!("  {at:6}: {:11} {:14} {letters}", i32::from_le_bytes(word), f32::from_le_bytes(word));
            }
        }
        // Every text of a scene sized to its words (`ContentSizeFitter`),
        // its saved width beside the width its font's advances give it
        // (`textwidths level44`): how TMP's preferred width goes.
        (Some("textwidths"), Some(file)) => {
            let mut tmp: std::collections::HashMap<(String, i64), Option<trp_data::screen::TmpFont>> = std::collections::HashMap::new();
            let install = assets.install.clone();
            let tmp_width = |tmp: &mut std::collections::HashMap<(String, i64), Option<trp_data::screen::TmpFont>>, file: &str, id: i64, shown: &str, text: &trp_data::screen::TextLook| -> Option<f32> {
                let mut font = |file: &str, id: i64| tmp.entry((file.to_owned(), id)).or_insert_with(|| trp_data::screen::TmpFont::read(&install, file, id).ok()).clone();
                let regular = font(file, id)?;
                let bold = text.style & 1 != 0;
                Some(match regular.bold.clone().filter(|_| bold).and_then(|(file, id)| font(&file, id)) {
                    Some(alternative) => alternative.width(shown, text.size, text.spacing, false),
                    None => regular.width(shown, text.size, text.spacing, bold),
                } + text.margin[0] + text.margin[2])
            };
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let roots: Vec<usize> = (0..scene.nodes.len()).filter(|&index| scene.nodes[index].layout.is_some() && scene.nodes[index].parent.is_none_or(|parent| scene.nodes[parent].layout.is_none())).collect();
            let mut fonts: std::collections::HashMap<String, Vec<(String, Vec<u8>)>> = std::collections::HashMap::new();
            let mut names: std::collections::HashMap<(String, i64), String> = std::collections::HashMap::new();
            for root in roots {
            let mut panel = trp_data::screen::ScenePanel::of(&scene, root);
            panel.file = file.to_owned();
            for node in &panel.nodes {
                let Some(text) = &node.text else { continue };
                if !node.scripts.iter().any(|(class, _)| class == "ContentSizeFitter") || text.text.is_empty() || text.text.contains('\n') || text.text.contains('<') || text.auto_size.is_some() {
                    continue;
                }
                let Some(font_file) = panel.file_of(text.font.0).or_else(|| (text.font.0 == 0).then(|| file.to_owned())) else { continue };
                if !fonts.contains_key(&font_file) {
                    fonts.insert(font_file.clone(), trp_data::screen::build_fonts(&assets.install, &font_file).unwrap_or_default());
                }
                let asset = names.entry((font_file.clone(), text.font.1)).or_insert_with(|| trp_data::screen::script_name(&assets.install, &font_file, text.font.1).unwrap_or_default()).clone();
                let shown = if text.style & 16 != 0 { text.text.to_uppercase() } else if text.style & 8 != 0 { text.text.to_lowercase() } else { text.text.clone() };
                let Some(width) = tmp_width(&mut tmp, &font_file, text.font.1, &shown, text) else { continue };
                let _ = trp_data::screen::font_for(&asset, &fonts[&font_file]);
                let saved = node.layout.size_delta[0];
                let fit = node.scripts.iter().find(|(class, _)| class == "ContentSizeFitter").and_then(|(_, fields)| fields.get(0..4)).map_or(-1, |word| i32::from_le_bytes(word.try_into().unwrap_or_default()));
                if fit != 2 {
                    continue;
                }
                println!("{:?} {:7.2} saved {:7.2} measured {:7.2} size {:5.1} style {:2} spacing {:4.1} {:?} {asset:?} {}", text.margin, saved - width, saved, width, text.size, text.style, text.spacing, shown, node.path);
            }
            }
        }
        // The inventory screen's bar of tabs as the app reads it: each tab
        // with what its scripts name, its words in the install's English
        // and how wide they are in their font.
        (Some("tabs"), _) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let bar = trp_data::screen::TabBar::load(&assets.install, &gear.file)?;
            let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| bar.panel.nodes[index].path.clone());
            println!("row {}, gap {}", at(bar.row), bar.gap);
            let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
            let words = trp_data::inventory::texts(&assets.install, "en", &keys).unwrap_or_default();
            let mut fonts: std::collections::HashMap<String, Vec<(String, Vec<u8>)>> = std::collections::HashMap::new();
            for tab in &bar.tabs {
                let said = words.get(&tab.key.to_lowercase()).cloned().unwrap_or_else(|| tab.key.clone());
                println!(
                    "{} order {} key {:?} ({said:?}) case {} adjust {} definers {:?} normal {} selected {} hover {} idle {} group {} target {}",
                    at(Some(tab.object)),
                    tab.order,
                    tab.key,
                    tab.case,
                    tab.width_adjustment,
                    tab.size_definers.iter().map(|&d| at(Some(d))).collect::<Vec<_>>(),
                    at(tab.normal),
                    at(tab.selected),
                    at(tab.hover),
                    at(tab.idle),
                    at(tab.canvas_group),
                    at(tab.target)
                );
                for &label in &tab.labels {
                    let Some(text) = &bar.panel.nodes[label].text else { continue };
                    let Some(file) = bar.panel.file_of(text.font.0) else { continue };
                    if !fonts.contains_key(&file) {
                        fonts.insert(file.clone(), trp_data::screen::build_fonts(&assets.install, &file).unwrap_or_default());
                    }
                    let asset = trp_data::screen::script_name(&assets.install, &file, text.font.1).unwrap_or_default();
                    println!("    font asset {asset:?}: {file} {}", text.font.1);
                    if let Ok(face) = trp_data::screen::TmpFont::read(&assets.install, &file, text.font.1) {
                        println!("      face {:?} {:?}, bold typeface {:?}", face.family, face.style, face.bold);
                        if let Some((bold_file, bold_id)) = &face.bold {
                            let bold_name = trp_data::screen::script_name(&assets.install, bold_file, *bold_id).unwrap_or_default();
                            match trp_data::screen::TmpFont::read(&assets.install, bold_file, *bold_id) {
                                Ok(bold) => println!("      bold {bold_name:?}: face {:?} {:?}", bold.family, bold.style),
                                Err(e) => println!("      bold {bold_name:?}: {e:#}"),
                            }
                        }
                    }
                    let font = trp_data::screen::font_for(&asset, &fonts[&file]);
                    let width = font.and_then(|(_, bytes)| trp_data::screen::text_width(bytes, &said, text.size, text.kerning));
                    let shown_said = if text.style & 16 != 0 { said.to_uppercase() } else { said.clone() };
                    let tmp = trp_data::screen::TmpFont::read(&assets.install, &file, text.font.1).ok();
                    let bold = text.style & 1 != 0;
                    let tmp_width = tmp.as_ref().map(|regular| match regular.bold.clone().filter(|_| bold).and_then(|(file, id)| trp_data::screen::TmpFont::read(&assets.install, &file, id).ok()) {
                        Some(alternative) => alternative.width(&shown_said, text.size, text.spacing, false),
                        None => regular.width(&shown_said, text.size, text.spacing, bold),
                    } + text.margin[0] + text.margin[2]);
                    println!("    as the game lays it out: {shown_said:?} {tmp_width:?}");
                    let shown = if text.style & 16 != 0 { text.text.to_uppercase() } else { text.text.clone() };
                    let saved = font.and_then(|(_, bytes)| trp_data::screen::text_width(bytes, &shown, text.size, text.kerning));
                    println!("    label {} font {asset:?} ({:?}) size {} style {} spacing {}: {said:?} {width:?}; the scene's {:?} {saved:?}, laid {:?}", at(Some(label)), font.map(|(name, _)| name), text.size, text.style, text.spacing, text.text, bar.panel.nodes[label].layout.size_delta);
                }
            }
        }
        // A sprite of one of the build's files by its number (`sprite
        // resources.assets 4222`): its cut, and its texture's pixels where
        // it is small (16 by 16 at most), each row from the lowest.
        (Some("sprite"), Some(file)) => {
            let id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("which sprite?"))?.parse()?;
            let mut pictures = trp_data::screen::ScenePictures::new(&assets.install)?;
            let cut = pictures.sprite(file, id)?;
            println!("{cut:?}");
            let texture = pictures.texture(&cut.texture.0, cut.texture.1)?;
            println!("texture {}x{} format {} srgb {}", texture.width, texture.height, texture.format, texture.srgb);
            if texture.width <= 16 && texture.height <= 16 {
                let levels = trp_data::unity::texture::mips(&texture)?;
                let pixels = trp_data::unity::texture::decode_rgba(&texture, *levels.first().ok_or_else(|| anyhow::anyhow!("no levels"))?)?;
                for row in pixels.chunks(texture.width as usize * 4) {
                    println!("  {:?}", row.chunks(4).collect::<Vec<_>>());
                }
            }
        }
        (Some("prefab"), Some(wanted)) => {
            // Or a script's prefab by its file and number
            // (`prefab sharedassets44.assets:9060`).
            let (file, id, scene) = match wanted.split_once(':') {
                Some((file, id)) => {
                    let id: i64 = id.parse()?;
                    (file.to_owned(), id, trp_data::screen::Scene::load_prefab_of_script(&assets.install, file, id)?)
                }
                None => {
                    let found = trp_data::screen::resource_paths(&assets.install)?.into_iter().find(|(path, ..)| path == wanted).ok_or_else(|| anyhow::anyhow!("no resource is at {wanted:?}"))?;
                    let (_, file, id) = found;
                    let scene = trp_data::screen::Scene::load_prefab(&assets.install, &file, id)?;
                    (file, id, scene)
                }
            };
            let panel = trp_data::screen::ScenePanel::of(&scene, 0);
            let own = panel.nodes[0].layout.size_delta;
            let size = [arg(2).and_then(|w| w.parse().ok()).unwrap_or(own[0]), arg(3).and_then(|h| h.parse().ok()).unwrap_or(own[1])];
            println!("{wanted} ({file} {id}): {} objects, {size:?}", panel.nodes.len());
            if let Some(group) = panel.nodes[0].group {
                println!("  its row: {group:?}");
            }
            // The pointers of its own scripts into other files.
            for (class, fields) in &panel.nodes[0].scripts {
                for start in (0..fields.len().saturating_sub(11)).step_by(4) {
                    let mut r = trp_data::unity::reader::Reader::new(&fields[start..]);
                    if let (Ok(to_file), Ok(to)) = (r.i32(), r.i64()) {
                        if (0..64).contains(&to_file) && to > 0 && to < 1 << 40 {
                            println!("  {class} at {start}: file {to_file} ({}) object {to}", panel.file_of(to_file).unwrap_or_default());
                        }
                    }
                }
            }
            let mut pictures = trp_data::screen::ScenePictures::new(&assets.install)?;
            for (index, node) in panel.nodes.iter().enumerate() {
                let [x, y, w, h] = panel.placed(index, size).rect;
                let classes: Vec<&str> = node.scripts.iter().map(|(class, _)| class.as_str()).collect();
                let mut line = format!("{:55} {} ({x:.1}, {y:.1}) {w:.1} x {h:.1} [{}]", node.path, if node.active { "on " } else { "off" }, classes.join(", "));
                if let Some(image) = &node.image {
                    line += &format!("  image {:?} kind {}", image.color, image.kind);
                    if let Some((outside, sprite)) = image.sprite {
                        let name = if outside == 0 { file.clone() } else { panel.externals.get(outside as usize - 1).map(|n| n.rsplit('/').next().unwrap_or(n).to_owned()).unwrap_or_default() };
                        match pictures.sprite(&name, sprite) {
                            Ok(cut) => {
                                let texture = pictures
                                    .texture(&cut.texture.0, cut.texture.1)
                                    .map(|t| {
                                        // Its first level's pixels: the least, most and mean of each channel.
                                        let pixels = trp_data::unity::texture::mips(&t).ok().and_then(|levels| trp_data::unity::texture::decode_rgba(&t, *levels.first()?).ok()).unwrap_or_default();
                                        let stats: Vec<String> = (0..4)
                                            .map(|c| {
                                                let values: Vec<u32> = pixels.iter().skip(c).step_by(4).map(|&v| v as u32).collect();
                                                let mean = values.iter().sum::<u32>() as f32 / values.len().max(1) as f32;
                                                format!("{}..{} ~{mean:.0}", values.iter().min().unwrap_or(&0), values.iter().max().unwrap_or(&0))
                                            })
                                            .collect();
                                        format!("{}x{} format {} srgb {} mips {} rgba {}", t.width, t.height, t.format, t.srgb, t.mip_count, stats.join(" "))
                                    })
                                    .unwrap_or_else(|e| format!("{e:#}"));
                                line += &format!(" sprite {:?} {:?} border {:?} of {texture}", cut.name, cut.rect, cut.border)
                            }
                            Err(e) => line += &format!(" sprite {name} {sprite}: {e:#}"),
                        }
                    }
                }
                if let Some(text) = &node.text {
                    line += &format!("  text {:?} size {} colour {:?} align {}/{} style {} font {:?}", text.text, text.size, text.color, text.horizontal, text.vertical, text.style, text.font);
                }
                println!("{line}");
            }
        }
        // The containers' column as the app reads it, and a container's
        // own layout of its grids (`containers [layout name]`).
        (Some("containers"), layout) => {
            let gear = trp_data::screen::GearPanel::load(&assets.install)?;
            let views = trp_data::screen::ContainerViews::load(&assets.install, &gear.file)?;
            let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| views.panel.nodes[index].path.clone());
            println!("template {}, header {}, slot panel {}", at(Some(views.template)), at(views.header), at(views.slot_panel));
            println!("column {:?}\ngrids' row {:?}", views.column, views.grids_row);
            if let Some(scrolling) = views.scrolling {
                println!("scrolling {scrolling:?}: view {}, content {}", at(scrolling.view), at(scrolling.content));
                if let Some(bar) = scrolling.bar {
                    println!("  its bar {}, sliding {}, handle {}, tinted {}", at(Some(bar.object)), at(bar.sliding), at(bar.handle), at(bar.tinted));
                }
            }
            if let Some(grid) = &views.grid {
                for (index, node) in grid.nodes.iter().enumerate() {
                    println!("  grid view: {} {:?} image {:?}", node.path, grid.rect(index, [64.0, 64.0]), node.image.map(|image| image.kind));
                }
            }
            for slot in ["TacticalVest", "Backpack", "SecuredContainer", "Pockets"] {
                println!("  slots/equipment/{slot}: {:?}", trp_data::screen::resource_sprite(&assets.install, &format!("Slots/Equipment/{slot}")));
            }
            if let Some(name) = layout {
                let (panel, grids) = trp_data::screen::rig_layout(&assets.install, name)?;
                let size = panel.nodes[0].layout.size_delta;
                for (n, view) in grids.iter().enumerate() {
                    println!("  grid {n}: {} {:?}", panel.nodes[*view].path, panel.rect(*view, size));
                }
            }
        }
        // The build's own Resources whose paths have a part (`resources
        // ugui/layouts`): each path with its file and object.
        (Some("resources-paths"), filter) => {
            let wanted = filter.unwrap_or("").to_lowercase();
            for (path, file, id) in trp_data::screen::resource_paths(&assets.install)?.iter().filter(|(path, ..)| path.contains(&wanted)) {
                println!("{path}  {file} {id}");
            }
        }
        // The fonts the build keeps whole (`fonts resources.assets`), and
        // the font assets the health panel's texts are drawn with.
        (Some("fonts"), Some(file)) => {
            for (name, font) in trp_data::screen::build_fonts(&assets.install, file)? {
                println!("{name:30} {} bytes, {:?}", font.len(), String::from_utf8_lossy(&font[..4]));
            }
            if let Ok(gear) = trp_data::screen::GearPanel::load(&assets.install) {
                if let Ok(health) = trp_data::screen::HealthPanel::load(&assets.install, &gear.file) {
                    let mut seen = std::collections::BTreeSet::new();
                    for node in health.panel.nodes.iter().chain(health.strip.iter().flat_map(|strip| strip.nodes.iter())) {
                        let Some(text) = &node.text else { continue };
                        if seen.insert(text.font) {
                            let outside = health.panel.externals.get(text.font.0 as usize - 1).cloned().unwrap_or_default();
                            println!("font asset {:?} in {outside}: {:?} (first by {})", text.font, trp_data::screen::script_name(&assets.install, &outside, text.font.1), node.path);
                        }
                    }
                }
            }
        }
        // The build's scenes, each with the file it is, and the inventory
        // screen's equipment panel as the install lays it out.
        (Some("scenes"), filter) => {
            let wanted = filter.unwrap_or("").to_lowercase();
            for (index, path) in trp_data::screen::scene_paths(&assets.install)?.iter().enumerate().filter(|(_, path)| path.to_lowercase().contains(&wanted)) {
                println!("level{index}  {path}");
            }
            let panel = trp_data::screen::GearPanel::load(&assets.install)?;
            println!("the equipment panel, from {}: {} x {}", panel.file, panel.size[0], panel.size[1]);
            for (name, [left, top, width, height]) in &panel.slots {
                println!("  {name}: left {left} top {top}, {width} x {height}");
            }
            let [width, height, up, right] = panel.quick;
            println!("the quick-use panel: {width} x {height}, {up} over the screen's lower edge, its middle {right} to the right of the screen's");
            for (key, middle, side) in &panel.quick_slots {
                println!("  {key}: its middle {middle} along the row, {side} square");
            }
        }
        // The install's own key bindings (`Control.ini`), those whose name
        // has that part: `keys Weapon`.
        (Some("keys"), filter) => {
            let wanted = filter.unwrap_or("").to_lowercase();
            let words = |source: &trp_data::install::KeySource| {
                let wheel = source.axis.iter().map(|(axis, up)| format!("{axis} {}", if *up { "+" } else { "-" }));
                source.keys.iter().cloned().chain(wheel).collect::<Vec<_>>().join(" + ")
            };
            for binding in assets.install.key_bindings().iter().filter(|binding| binding.name.to_lowercase().contains(&wanted)) {
                let variants: Vec<String> = binding.variants.iter().filter(|source| !source.is_empty()).map(words).collect();
                println!("{:28} {:12} {}", binding.name, binding.press, variants.join("   or   "));
            }
            // The axes the keys push, each pair's two ways.
            for axis in assets.install.axis_bindings().iter().filter(|axis| axis.name.to_lowercase().contains(&wanted)) {
                for [positive, negative] in axis.pairs.iter().filter(|pair| pair.iter().any(|way| !way.is_empty())) {
                    println!("{:28} {:12} {}   against   {}", axis.name, "axis", words(positive), words(negative));
                }
            }
            let display = assets.install.display_settings();
            println!("display: vsync {:?}, mode {:?} (0 full screen, 1 borderless, 2 window), resolution {:?}", display.vsync, display.mode, display.resolution);
            for name in ["DoubleClickTimeout", "MouseSensitivity", "MouseAimingSensitivity", "OpticSensitivity"] {
                println!("{name}: {:?}", assets.install.control_setting(name));
            }
            for name in ["InvertedXAxis", "InvertedYAxis"] {
                println!("{name}: {:?}", assets.install.control_flag(name));
            }
            // The engine's own axes that the bindings name, and what each
            // binding multiplies its axis by.
            for axis in assets.install.engine_axes()?.iter().filter(|axis| axis.name.starts_with("Mouse")) {
                println!(
                    "engine axis {:20} x {} a count, dead zone {}, kind {} (1 the mouse's movement), axis {} (2 the wheel)",
                    axis.name, axis.sensitivity, axis.dead, axis.kind, axis.axis
                );
            }
            for name in ["TurnX", "TurnY", "LookX", "LookY"] {
                println!("{name}: its binding multiplies by {:?}", assets.install.axis_sensitivity(name));
            }
            for name in ["PriorityWindowMode", "ItemQuickUseMode"] {
                println!("{name}: {}", assets.install.game_setting(name).unwrap_or_else(|| "(not set)".into()));
            }
        }
        // The objects of one of the install's scene files whose name has
        // that part, each with what hangs from it so many levels down and
        // how each is laid out on its screen: `screen level49 "Slot" 1`.
        (Some("screen"), Some(file)) => {
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let wanted = arg(2).unwrap_or("").to_lowercase();
            let depth: usize = arg(3).and_then(|d| d.parse().ok()).unwrap_or(1);
            fn show(scene: &trp_data::screen::Scene, index: usize, indent: usize, depth: usize) {
                let node = &scene.nodes[index];
                let layout = node.layout.map_or(String::new(), |l| {
                    format!(
                        "  anchors ({}, {})-({}, {})  offset ({}, {})  size ({}, {})  pivot ({}, {})  scale ({}, {})",
                        l.anchor_min[0], l.anchor_min[1], l.anchor_max[0], l.anchor_max[1], l.offset[0], l.offset[1], l.size_delta[0], l.size_delta[1], l.pivot[0], l.pivot[1], l.scale[0], l.scale[1]
                    )
                });
                let scripts = if node.scripts.is_empty() { String::new() } else { format!("  [{}]", node.scripts.join(", ")) };
                println!("{}{}{}{layout}{scripts}", "  ".repeat(indent), node.name, if node.active { "" } else { " (off)" });
                if depth > 0 {
                    for &child in &node.children {
                        show(scene, child, indent + 1, depth - 1);
                    }
                }
            }
            println!("{file}: {} objects", scene.nodes.len());
            for index in (0..scene.nodes.len()).filter(|&index| scene.nodes[index].name.to_lowercase().contains(&wanted)) {
                println!("{}", scene.path(index));
                show(&scene, index, 1, depth);
            }
        }
        // Each script's own fields on the objects of a scene whose path
        // ends so, word by word as a whole number and as a real number
        // (`fields level44 "Items Panel/LeftSide"`): how a layout script of
        // the engine's is set, read by its known order of fields.
        (Some("fields"), Some(file)) => {
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let end = arg(2).unwrap_or("");
            // The files a pointer's first number counts from 1 in.
            let data = std::fs::read(assets.install.root.join("EscapeFromTarkov_Data").join(file))?;
            for (n, outside) in trp_data::unity::SerializedFile::parse(&data)?.externals.iter().enumerate() {
                println!("file {}: {outside}", n + 1);
            }
            for index in (0..scene.nodes.len()).filter(|&index| scene.path(index).ends_with(end)) {
                println!("{}  [{}]", scene.path(index), scene.nodes[index].scripts.join(", "));
                for behaviour in scene.behaviours.iter().filter(|behaviour| behaviour.node == index) {
                    let fields = scene.fields(behaviour);
                    println!("  the script {}: {} bytes of fields", behaviour.id, fields.len());
                    for (n, word) in fields.chunks(4).take(160).enumerate() {
                        let mut bytes = [0u8; 4];
                        bytes[..word.len()].copy_from_slice(word);
                        println!("    {:3}: {:02x?}  {}  {}", n * 4, bytes, i32::from_le_bytes(bytes), f32::from_le_bytes(bytes));
                    }
                }
            }
        }
        // What the scripts on the objects of a scene whose path ends so
        // point to among the scene's own scripts: each such pointer in their
        // fields, by where it lies, with the object it points to and that
        // object's scripts (`pointers level44 "InventoryScreen/Items Panel/Stash Panel/Simple Panel"`).
        (Some("pointers"), Some(file)) => {
            let scene = trp_data::screen::Scene::load(&assets.install, file, true)?;
            let end = arg(2).unwrap_or("");
            for index in (0..scene.nodes.len()).filter(|&index| scene.path(index).ends_with(end)) {
                println!("{}  [{}]", scene.path(index), scene.nodes[index].scripts.join(", "));
                for behaviour in scene.behaviours.iter().filter(|behaviour| behaviour.node == index) {
                    let fields = scene.fields(behaviour);
                    println!("  the script {}: {} bytes of fields", behaviour.id, fields.len());
                    for start in (0..fields.len().saturating_sub(11)).step_by(4) {
                        let mut r = trp_data::unity::reader::Reader::new(&fields[start..]);
                        let (Ok(file), Ok(path)) = (r.i32(), r.i64()) else { continue };
                        let Some(to) = scene.owner(path).filter(|_| file == 0 && path != 0) else { continue };
                        println!("    at {start}: {}  [{}]", scene.path(to), scene.nodes[to].scripts.join(", "));
                    }
                }
            }
        }
        // Every script of a class on the player prefab: the object it is on,
        // that object's parent and layer, and the colliders on it
        // (`player-scripts BodyPartCollider`).
        (Some("player-scripts"), Some(wanted)) => {
            use trp_data::unity::{class, Value};
            let files = assets.load(trp_data::player::PLAYER_BUNDLE)?;
            let objects: Vec<_> = files.iter().flat_map(|&f| assets.objects_of(f, class::MONO_BEHAVIOUR)).collect();
            for obj in objects {
                if trp_data::unity::extract::script_class(&assets, obj).as_deref() != Some(wanted) {
                    continue;
                }
                let v = assets.read(obj)?;
                let Some(owner) = v.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(obj.file, p)) else { continue };
                let go = assets.read(owner)?;
                let name = go.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned();
                let layer = go.get("m_Layer").and_then(Value::as_i64).unwrap_or(0);
                let mut parent = String::new();
                let mut shapes = Vec::new();
                for c in go.get("m_Component").and_then(Value::as_array).unwrap_or(&[]) {
                    let Some(c) = c.get("component").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(owner.file, p)) else { continue };
                    let class_id = assets.class_id(c);
                    let value = assets.read(c)?;
                    if class_id == Some(class::TRANSFORM) {
                        let father = value.get("m_Father").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(c.file, p));
                        let pos = value.get("m_LocalPosition").and_then(Value::as_vec3);
                        parent = father
                            .and_then(|f| assets.read(f).ok())
                            .and_then(|t| t.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(c.file, p)))
                            .and_then(|g| assets.read(g).ok())
                            .and_then(|g| g.get("m_Name").and_then(Value::as_str).map(str::to_owned))
                            .unwrap_or_default();
                        parent = format!("{parent} at {pos:?}");
                    } else if matches!(class_id, Some(65 | 135 | 136)) {
                        let get = |n: &str| value.get(n).map(|v| v.as_vec3().map(|x| format!("{x:?}")).or(v.as_f32().map(|f| f.to_string())).unwrap_or_default()).unwrap_or_default();
                        shapes.push(format!("class {} centre {} size {} radius {} height {} dir {} trigger {}", class_id.unwrap_or(0), get("m_Center"), get("m_Size"), get("m_Radius"), get("m_Height"), get("m_Direction"), get("m_IsTrigger")));
                    }
                }
                let fields: Vec<String> = ["BodyPartType", "BodyPartColliderType"].iter().filter_map(|f| v.get(f).and_then(Value::as_i64).map(|x| format!("{f} {x}"))).collect();
                println!("{name} (layer {layer}) under {parent} {}: {}", fields.join(" "), shapes.join("; "));
            }
        }
        // The ragdoll a dead character becomes (`trp_data::ragdoll`).
        // The engine's physics layers: their names (`TagManager`) and which
        // collide with which (`PhysicsManager.m_LayerCollisionMatrix`, found
        // as 32 masks after their count that agree with one another), and
        // the physics settings' first figures (`layers`).
        (Some("layers"), _) => {
            let path = assets.install.root.join("EscapeFromTarkov_Data").join("globalgamemanagers");
            let data = std::fs::read(&path)?;
            let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
            let object = |class_id: i32| meta.objects.iter().find(|o| o.class_id == class_id).map(|o| &data[o.offset..o.offset + o.size]);
            let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            // The names: the tags' list, then the layers' 32.
            let mut names: Vec<String> = Vec::new();
            if let Some(tags) = object(78) {
                let mut at = 0usize;
                let list = |at: &mut usize| -> Vec<String> {
                    let count = word(tags, *at) as usize;
                    *at += 4;
                    (0..count)
                        .map(|_| {
                            let len = word(tags, *at) as usize;
                            let text = String::from_utf8_lossy(&tags[*at + 4..*at + 4 + len]).into_owned();
                            *at = (*at + 4 + len + 3) & !3;
                            text
                        })
                        .collect()
                };
                let tag_list = list(&mut at);
                names = list(&mut at);
                println!("{} tags; {} layers", tag_list.len(), names.len());
            }
            let Some(physics) = object(55) else { anyhow::bail!("no physics settings") };
            let floats: Vec<String> = physics.chunks_exact(4).take(24).map(|c| {
                let v = f32::from_le_bytes(c.try_into().unwrap());
                if v.is_finite() && (v == 0.0 || v.abs() > 1e-6 && v.abs() < 1e6) { format!("{v}") } else { format!("i{}", i32::from_le_bytes(c.try_into().unwrap())) }
            }).collect();
            println!("physics settings' first words: {}", floats.join(" "));
            let rows = (0..physics.len().saturating_sub(4 * 33)).step_by(4).find(|&at| {
                word(physics, at) == 32 && (0..32).all(|i| (0..32).all(|j| (word(physics, at + 4 + 4 * i) >> j & 1) == (word(physics, at + 4 + 4 * j) >> i & 1)))
                    && (0..32).any(|i| word(physics, at + 4 + 4 * i) != 0)
            });
            let Some(rows) = rows else { anyhow::bail!("no collision matrix found") };
            println!("the collision matrix at byte {rows}");
            let after: Vec<String> = physics[rows + 4 * 33..].chunks_exact(4).take(40).map(|c| {
                let v = f32::from_le_bytes(c.try_into().unwrap());
                if v.is_finite() && (v == 0.0 || v.abs() > 1e-6 && v.abs() < 1e6) { format!("{v}") } else { format!("i{}", i32::from_le_bytes(c.try_into().unwrap())) }
            }).collect();
            println!("after it: {}", after.join(" "));
            let bytes: Vec<String> = physics[rows + 4 * 33..].iter().take(64).map(|b| format!("{b:02x}")).collect();
            println!("its bytes: {}", bytes.join(" "));
            let name = |i: usize| names.get(i).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("({i})"));
            for i in 0..32 {
                let mask = word(physics, rows + 4 + 4 * i);
                let with: Vec<String> = (0..32).filter(|j| mask >> j & 1 == 1).map(name).collect();
                if !names.get(i).is_some_and(|n| n.is_empty()) {
                    println!("{i:2} {}: collides with {}", name(i), with.join(", "));
                }
            }
        }
        (Some("ragdoll"), _) => {
            let ragdoll = trp_data::ragdoll::RagdollData::load(&mut assets)?;
            for body in &ragdoll.bodies {
                println!("body {}: mass {} drag {} angular drag {}", body.bone, body.mass, body.drag, body.angular_drag);
                for collider in &body.colliders {
                    println!("    {} {:?} at {:?}", collider.object, collider.shape, collider.local);
                }
            }
            for joint in &ragdoll.joints {
                println!(
                    "joint {} to {}: anchor {:?} axis {:?} swing axis {:?} twist {}..{} swing {} {} springs {:?} {:?} collision {} projection {:?} auto anchor {} connected anchor {:?} preprocessing {}",
                    joint.bone, joint.connected, joint.anchor, joint.axis, joint.swing_axis, joint.low_twist.limit, joint.high_twist.limit, joint.swing1.limit, joint.swing2.limit, joint.twist_spring, joint.swing_spring, joint.enable_collision, joint.projection, joint.auto_anchor, joint.connected_anchor, joint.preprocessing
                );
            }
        }
        // One object of the game's own `resources.assets` by its number: its
        // class and, for most, its name, which comes first in it; and its
        // first figures (`object 84`).
        (Some("object"), Some(id)) => {
            let path = assets.install.root.join("EscapeFromTarkov_Data").join("resources.assets");
            let data = std::fs::read(&path)?;
            let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
            let id: i64 = id.parse()?;
            let object = meta.objects.iter().find(|o| o.path_id == id).ok_or_else(|| anyhow::anyhow!("no object {id}"))?;
            let bytes = &data[object.offset..object.offset + object.size];
            let len = u32::from_le_bytes(bytes[0..4].try_into()?) as usize;
            let name = (len < 200).then(|| String::from_utf8_lossy(&bytes[4..4 + len.min(bytes.len() - 4)]).into_owned()).unwrap_or_default();
            println!("object {id}: class {}, {} bytes, named {name:?}", object.class_id, bytes.len());
            let text: String = bytes.iter().take(4096).map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect();
            println!("{text}");
        }
        // Game objects of the game's own `resources.assets` (no type trees)
        // whose name has a word: each component's class and path id; a
        // transform's place and turn; the first figures of anything else,
        // as floats and as integers, to be read by hand.
        (Some("resources"), Some(wanted)) => {
            let path = assets.install.root.join("EscapeFromTarkov_Data").join("resources.assets");
            let data = std::fs::read(&path)?;
            let meta = trp_data::unity::serialized::SerializedFile::parse(&data)?;
            let bytes_of = |path_id: i64| meta.objects.iter().find(|o| o.path_id == path_id).map(|o| (o.class_id, &data[o.offset..o.offset + o.size]));
            // A script's class, from the file its pointer names (a script
            // object begins with its name), as `screen::Scene` finds it.
            let folder = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
            let mut outside: std::collections::HashMap<i32, (Vec<u8>, trp_data::unity::serialized::SerializedFile)> = std::collections::HashMap::new();
            let mut script_of = |file: i32, id: i64| -> Option<String> {
                let listed = meta.externals.get(usize::try_from(file).ok()?.checked_sub(1)?)?;
                if !outside.contains_key(&file) {
                    let name = listed.rsplit('/').next().unwrap_or(listed);
                    let data = std::fs::read(folder.join(name)).ok()?;
                    let parsed = trp_data::unity::serialized::SerializedFile::parse(&data).ok()?;
                    outside.insert(file, (data, parsed));
                }
                let (data, parsed) = &outside[&file];
                let found = parsed.objects.iter().find(|o| o.path_id == id)?;
                let bytes = &data[found.offset..found.offset + found.size];
                let len = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
                Some(String::from_utf8_lossy(bytes.get(4..4 + len)?).into_owned())
            };
            let wanted = wanted.to_lowercase();
            for object in meta.objects.iter().filter(|o| o.class_id == 1) {
                let mut r = trp_data::unity::reader::Reader::new(&data[object.offset..object.offset + object.size]);
                let count = r.u32()? as usize;
                let mut components = Vec::new();
                for _ in 0..count {
                    let (file, id) = (r.i32()?, r.i64()?);
                    components.push((file, id));
                }
                let _layer = r.u32()?;
                let len = r.u32()? as usize;
                let name = String::from_utf8_lossy(r.bytes(len)?).into_owned();
                if !name.to_lowercase().contains(&wanted) {
                    continue;
                }
                println!("{name} (game object {}):", object.path_id);
                for (file, id) in components {
                    let Some((class, bytes)) = bytes_of(id).filter(|_| file == 0) else {
                        println!("  component in file {file}, {id}");
                        continue;
                    };
                    let mut r = trp_data::unity::reader::Reader::new(bytes);
                    r.skip(12)?;
                    match class {
                        4 => {
                            let q = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
                            let p = [r.f32()?, r.f32()?, r.f32()?];
                            let s = [r.f32()?, r.f32()?, r.f32()?];
                            let children = r.u32()?;
                            let mut names = Vec::new();
                            for _ in 0..children {
                                let (_, child) = (r.i32()?, r.i64()?);
                                // The child transform's own game object, and its name.
                                let name = bytes_of(child).and_then(|(_, t)| {
                                    let go = i64::from_le_bytes(t[4..12].try_into().ok()?);
                                    let (_, g) = bytes_of(go)?;
                                    let n = u32::from_le_bytes(g[0..4].try_into().ok()?) as usize;
                                    let at = 4 + 12 * n + 4;
                                    let len = u32::from_le_bytes(g[at..at + 4].try_into().ok()?) as usize;
                                    Some(String::from_utf8_lossy(&g[at + 4..at + 4 + len]).into_owned())
                                });
                                names.push(name.unwrap_or_default());
                            }
                            let (_, father) = (r.i32()?, r.i64()?);
                            println!("  transform: turned {q:?} at {p:?} scale {s:?}, children {names:?}, parent {father}");
                        }
                        _ => {
                            let floats: Vec<String> = bytes[12..if arg(2) == Some("all") { bytes.len() } else { bytes.len().min(12 + 4 * 48) }].chunks_exact(4).map(|c| {
                                let (f, i) = (f32::from_le_bytes([c[0], c[1], c[2], c[3]]), i32::from_le_bytes([c[0], c[1], c[2], c[3]]));
                                if f.abs() > 1e-6 && f.abs() < 1e6 { format!("{f}") } else { format!("i{i}") }
                            }).collect();
                            // A script's class, where it is one.
                            let script = (class == 114 && bytes.len() >= 28)
                                .then(|| script_of(i32::from_le_bytes(bytes[16..20].try_into().unwrap()), i64::from_le_bytes(bytes[20..28].try_into().unwrap())))
                                .flatten()
                                .map_or(String::new(), |name| format!(" {name}"));
                            println!("  class {class}{script} ({} bytes, {id}): {}", bytes.len(), floats.join(" "));
                        }
                    }
                }
            }
        }
        // The game's icon camera as `icon::Studio::read` takes it, beside
        // what was read by hand on 2026-10-14.
        (Some("studio"), _) => {
            let read = trp_data::icon::Studio::read(&assets.install)?;
            println!("read: {read:#?}");
            println!("the same as read by hand: {}", read == trp_data::icon::Studio::as_read());
        }
        // An item's icon settings as its prefab has them (`icon::pose_in`):
        // the object they are on, its renderers, and for a weapon the
        // object its hands hold.
        (Some("icon"), Some(wanted)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let item = db.item(wanted)?;
            let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?.to_owned();
            let files = assets.load(&key)?;
            println!("{} {}: {key}", item.id, item.name);
            match trp_data::icon::pose_in(&assets, &files)? {
                Some((object, pose)) => {
                    let tree = trp_data::unity::extract::read_hierarchy(&assets, object)?;
                    println!("  PreviewPivot on {} ({} nodes, {} with a renderer): {pose:?}", tree.nodes[0].name, tree.nodes.len(), tree.nodes.iter().filter(|n| n.renderer.is_some()).count());
                    for node in tree.nodes.iter().take(6) {
                        println!("    {} parent {:?} at {:?} turned {:?} scale {:?}{}", node.name, node.parent, node.translation, node.rotation, node.scale, if node.renderer.is_some() { " (drawn)" } else { "" });
                    }
                }
                None => println!("  no PreviewPivot"),
            }
            if let Ok(weapon) = trp_data::weapon::assemble(&mut assets, &db, &item.id) {
                println!("  assembled: root {} ({} nodes)", weapon.hierarchy.nodes[0].name, weapon.hierarchy.nodes.len());
            }
        }
        // An item template of the database, by id or by a part of its
        // name: what it weighs, the cells it takes, its slots and grids.
        // An item's properties in the install's database, whole or those
        // named (`props 590c678286f77426c9660122 UsePrefab medUseTime`).
        (Some("props"), Some(wanted)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let item = db.items.get(wanted).ok_or_else(|| anyhow::anyhow!("no item {wanted}"))?;
            let names: Vec<&str> = (2..16).filter_map(arg).collect();
            println!("{} {}", item.id, item.name);
            match (names.is_empty(), item.props.as_object()) {
                (false, Some(props)) => {
                    for name in names {
                        println!("  {name}: {}", props.get(name).map_or("-".to_owned(), |value| value.to_string()));
                    }
                }
                _ => println!("{}", serde_json::to_string_pretty(&item.props)?),
            }
        }
        (Some("item"), Some(wanted)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let mut found: Vec<&trp_data::database::ItemTemplate> =
                db.items.values().filter(|item| item.id == wanted || item.name.to_lowercase().contains(&wanted.to_lowercase())).collect();
            found.sort_by(|a, b| a.name.cmp(&b.name));
            let name_of = |id: &str| db.items.get(id).map_or("?", |item| item.name.as_str()).to_owned();
            let full = found.len() <= 3;
            for item in found {
                let mut chain = Vec::new();
                let mut at = item.parent.as_str();
                while let Some(parent) = db.items.get(at) {
                    chain.push(parent.name.as_str());
                    at = parent.parent.as_str();
                }
                println!("{} {}  <- {}", item.id, item.name, chain.join(" <- "));
                if !full {
                    continue;
                }
                let number = |name: &str| item.props.get(name).and_then(|v| v.as_f64()).map_or("-".to_owned(), |v| v.to_string());
                println!(
                    "  weight {}  cells {} x {}  extra left {} right {} up {} down {}  stack {}",
                    number("Weight"),
                    number("Width"),
                    number("Height"),
                    number("ExtraSizeLeft"),
                    number("ExtraSizeRight"),
                    number("ExtraSizeUp"),
                    number("ExtraSizeDown"),
                    number("StackMaxSize")
                );
                let flags: Vec<&str> = item
                    .props
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(name, value)| name.starts_with("Blocks") && value.as_bool() == Some(true))
                    .map(|(name, _)| name.as_str())
                    .collect();
                if !flags.is_empty() {
                    println!("  {}", flags.join(" "));
                }
                let filters = |holder: &serde_json::Value| {
                    let lists = holder.pointer("/_props/filters").and_then(|f| f.as_array()).cloned().unwrap_or_default();
                    let ids = |list: &serde_json::Value, name: &str| {
                        let ids: Vec<String> =
                            list.get(name).and_then(|f| f.as_array()).into_iter().flatten().filter_map(|id| id.as_str()).map(|id| format!("{id} {}", name_of(id))).collect();
                        ids
                    };
                    let (mut takes, mut refuses) = (Vec::new(), Vec::new());
                    for list in &lists {
                        takes.extend(ids(list, "Filter"));
                        refuses.extend(ids(list, "ExcludedFilter"));
                    }
                    (takes, refuses)
                };
                for slot in item.props.get("Slots").and_then(|s| s.as_array()).into_iter().flatten() {
                    let (takes, _) = filters(slot);
                    let shown: Vec<&String> = takes.iter().take(4).collect();
                    println!(
                        "  slot {}{}: {} it takes{}{}",
                        slot.get("_name").and_then(|n| n.as_str()).unwrap_or("?"),
                        if slot.get("_required").and_then(|r| r.as_bool()) == Some(true) { " (required)" } else { "" },
                        takes.len(),
                        shown.iter().map(|s| format!("; {s}")).collect::<String>(),
                        if takes.len() > shown.len() { "; ..." } else { "" }
                    );
                }
                for grid in item.props.get("Grids").and_then(|g| g.as_array()).into_iter().flatten() {
                    let (takes, refuses) = filters(grid);
                    let cells = |name: &str| grid.pointer(&format!("/_props/{name}")).and_then(|v| v.as_i64()).unwrap_or(0);
                    println!(
                        "  grid {}: {} x {}; takes {}; refuses {}",
                        grid.get("_name").and_then(|n| n.as_str()).unwrap_or("?"),
                        cells("cellsH"),
                        cells("cellsV"),
                        if takes.is_empty() { "anything".to_owned() } else { takes.join(", ") },
                        if refuses.is_empty() { "nothing".to_owned() } else { refuses.join(", ") }
                    );
                }
            }
        }
        // A look into one of the database's files: `probe json
        // templates/profiles.json /Standard/usec/character/Inventory 2`.
        (Some("json"), Some(file)) => {
            let path = assets.install.database_dir().join(file);
            let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
            let at = value.pointer(arg(2).unwrap_or("")).ok_or_else(|| anyhow::anyhow!("nothing at {}", arg(2).unwrap_or("")))?;
            let depth: usize = arg(3).map(|d| d.parse()).transpose()?.unwrap_or(1);
            print_json(at, depth, 0);
        }
        (Some("find"), Some(text)) => {
            let mut keys: Vec<&String> =
                assets.manifest.entries.keys().filter(|k| k.contains(text)).collect();
            keys.sort();
            for key in keys {
                println!("{key}");
            }
        }
        (Some("info"), Some(key)) => {
            println!("dependencies: {:?}", assets.manifest.dependencies(key));
            let own = assets.load(key)?;
            for (index, file) in assets.files().iter().enumerate() {
                let mark = if own.contains(&index) { "*" } else { " " };
                let mut counts: BTreeMap<i32, usize> = BTreeMap::new();
                for o in &file.meta.objects {
                    *counts.entry(o.class_id).or_default() += 1;
                }
                let counts: Vec<String> = counts
                    .iter()
                    .map(|(id, n)| match class::name(*id) {
                        "" => format!("#{id}={n}"),
                        name => format!("{name}={n}"),
                    })
                    .collect();
                println!(
                    "{mark}[{index}] {} <- {}\n    v{} {} typetrees={} externals={:?}\n    {}",
                    file.name,
                    file.bundle_key,
                    file.meta.version,
                    file.meta.unity_version,
                    file.meta.has_type_trees,
                    file.meta.externals,
                    counts.join(" ")
                );
            }
        }
        (Some("names"), Some(key)) => {
            let filter: Option<i32> = arg(2).map(|c| c.parse()).transpose()?;
            for file in assets.load(key)? {
                let objects: Vec<_> = assets.file(file).meta.objects.clone();
                for o in objects {
                    if filter.is_some_and(|c| c != o.class_id) {
                        continue;
                    }
                    let obj = ObjRef { file, path_id: o.path_id };
                    let name = match assets.read(obj) {
                        Ok(v) => v.get("m_Name").and_then(|n| n.as_str()).unwrap_or("").to_owned(),
                        Err(e) => format!("<unreadable: {e}>"),
                    };
                    println!("{:>20} {:<22} {:>9}  {name}", o.path_id, class_label(o.class_id), o.size);
                }
            }
        }
        (Some("dump"), Some(key)) => {
            let path_id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("path id needed"))?.parse()?;
            let depth = arg(3).map_or(Ok(6), str::parse)?;
            let items = arg(4).map_or(Ok(12), str::parse)?;
            for file in assets.load(key)? {
                let obj = ObjRef { file, path_id };
                if assets.info(obj).is_some() {
                    println!("{}", assets.read(obj)?.pretty(depth, items));
                }
            }
        }
        (Some("script"), Some(key)) => {
            // Every script of a bundle of the class named, as stored, with
            // the object each is on.
            let wanted = arg(2).ok_or_else(|| anyhow::anyhow!("a script's class name needed"))?;
            let depth = arg(3).map_or(Ok(6), str::parse)?;
            let items = arg(4).map_or(Ok(12), str::parse)?;
            for file in assets.load(key)? {
                for obj in assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
                    if trp_data::unity::extract::script_class(&assets, obj).as_deref() != Some(wanted) {
                        continue;
                    }
                    let value = assets.read(obj)?;
                    let owner = value
                        .get("m_GameObject")
                        .and_then(|v| v.as_pptr())
                        .and_then(|p| assets.resolve_loaded(obj.file, p))
                        .and_then(|owner| assets.read(owner).ok())
                        .and_then(|owner| owner.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned))
                        .unwrap_or_default();
                    println!("{wanted} {} on {owner}\n{}", obj.path_id, value.pretty(depth, items));
                }
            }
        }
        (Some("preset"), Some(weapon)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let preset = db.default_preset(weapon)?;
            println!("{} ({})", preset.name, preset.id);
            for part in &preset.items {
                let item = db.item(&part.template)?;
                println!(
                    "  {:<16} {:<44} {}",
                    part.slot.as_deref().unwrap_or("-"),
                    item.name,
                    item.prefab_key().unwrap_or("-")
                );
            }
        }
        (Some("weapon"), Some(template)) => {
            use trp_data::unity::extract::{read_material, read_mesh, read_texture};
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            println!("{} with {} nodes", weapon.name, weapon.hierarchy.nodes.len());
            for s in &weapon.skipped {
                println!("  SKIPPED {s}");
            }
            // With a third word: every bone whose path has it, at rest.
            if let Some(filter) = arg(2) {
                for (i, node) in weapon.hierarchy.nodes.iter().enumerate() {
                    let path = weapon.hierarchy.path(i);
                    if path.contains(filter) {
                        println!("  [{i}] {path}  at {:?} rot {:?}", node.translation, node.rotation);
                    }
                }
                return Ok(());
            }
            for (i, node) in weapon.hierarchy.nodes.iter().enumerate() {
                if let Some(grip) = &node.grip {
                    println!(
                        "  [{i}] {}  grip for the {} hand, kind {}, {} finger rotations, at {:?} rot {:?}",
                        weapon.hierarchy.path(i),
                        if grip.left_hand { "left" } else { "right" },
                        grip.kind,
                        grip.fingers.len(),
                        node.translation,
                        node.rotation
                    );
                }
                let Some(r) = &node.renderer else { continue };
                let mesh = read_mesh(&assets, r.mesh)?;
                let tris: usize = mesh.submeshes.iter().map(|s| s.len() / 3).sum();
                println!(
                    "  [{i}] {}  mesh={} verts={} tris={} skinned={}",
                    weapon.hierarchy.path(i),
                    mesh.name,
                    mesh.positions.len(),
                    tris,
                    r.skinned
                );
                for m in r.materials.iter().flatten() {
                    let mat = read_material(&assets, *m)?;
                    let mut textures: Vec<String> = Vec::new();
                    for (slot, tex) in &mat.textures {
                        let t = read_texture(&assets, *tex)?;
                        textures.push(format!("{slot}={} {}x{} fmt{} mips{}", t.name, t.width, t.height, t.format, t.mip_count));
                    }
                    textures.sort();
                    println!("        material {}: {}", mat.name, textures.join("; "));
                }
            }
        }
        // A model of S.T.A.L.K.E.R.'s (`docs/spec/outfits.md`), by its
        // file or by its path under that game's `meshes`: its meshes with
        // their pictures, and its bones at rest, each beside the middle of
        // the vertices that go mostly with it.
        (Some("ogf"), Some(wanted)) => {
            let stalker = trp_data::xray::Stalker::locate();
            let (bytes, file) = match std::path::Path::new(wanted).is_file() {
                true => (std::fs::read(wanted)?, wanted.to_owned()),
                false => stalker.read(&format!("meshes/{wanted}"))?,
            };
            let model = trp_data::xray::read(&bytes)?;
            println!("{file}: {} bones, {} meshes", model.bones.len(), model.parts.len());
            for part in &model.parts {
                let told = match stalker.read(&format!("textures/{}.dds", part.texture)) {
                    Ok((bytes, file)) => match trp_data::xray::read_picture(&bytes) {
                        Ok(p) => format!("{} x {}, {} levels, {:?}, {file}", p.width, p.height, p.levels, p.packing),
                        Err(e) => format!("not read ({e:#}), {file}"),
                    },
                    Err(e) => format!("{e:#}"),
                };
                let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
                for p in &part.positions {
                    for axis in 0..3 {
                        (low[axis], high[axis]) = (low[axis].min(p[axis]), high[axis].max(p[axis]));
                    }
                }
                println!("  mesh: {} vertices, {} triangles, from {:.3} {:.3} {:.3} to {:.3} {:.3} {:.3}; picture {}: {told}", part.positions.len(), part.triangles.len() / 3, low[0], low[1], low[2], high[0], high[1], high[2], part.texture);
            }
            for (index, bone) in model.bones.iter().enumerate() {
                // The bone's own line: to the first bone that hangs from
                // it, or else on from the one it hangs from. Its vertices
                // are measured along that, from its joint, and across.
                let next = model.bones.iter().find(|other| other.parent == Some(index)).map(|child| child.at);
                let line = next.map(|at| [at[0] - bone.at[0], at[1] - bone.at[1], at[2] - bone.at[2]]).or_else(|| bone.parent.map(|parent| {
                    let from = model.bones[parent].at;
                    [bone.at[0] - from[0], bone.at[1] - from[1], bone.at[2] - from[2]]
                }));
                let long = line.map_or(0.0, |v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
                let mut sum = [0.0f32; 3];
                let mut count = 0;
                let (mut back, mut ahead, mut wide) = (f32::MAX, f32::MIN, 0.0f32);
                for part in &model.parts {
                    for (p, links) in part.positions.iter().zip(&part.links) {
                        if links.iter().any(|(linked, share)| *linked as usize == index && *share > 0.5) {
                            sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
                            count += 1;
                            if let Some(line) = line.filter(|_| long > 1e-5) {
                                let off = [p[0] - bone.at[0], p[1] - bone.at[1], p[2] - bone.at[2]];
                                let along = (off[0] * line[0] + off[1] * line[1] + off[2] * line[2]) / long;
                                let across = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2] - along * along).max(0.0).sqrt();
                                (back, ahead, wide) = (back.min(along), ahead.max(along), wide.max(across));
                            }
                        }
                    }
                }
                let measured = if count > 0 && long > 1e-5 { format!(", from {:.1} to {:.1} cm along its {:.1} cm, {:.1} cm across", back * 100.0, ahead * 100.0, long * 100.0, wide * 100.0) } else { String::new() };
                let middle = if count > 0 { format!("{count:4} vertices about {:7.3} {:7.3} {:7.3}{measured}", sum[0] / count as f32, sum[1] / count as f32, sum[2] / count as f32) } else { "no vertices".to_owned() };
                let parent = bone.parent.map_or("-", |parent| model.bones[parent].name.as_str());
                println!("  {index:2} {:<20} < {:<18} at {:7.3} {:7.3} {:7.3}   {middle}", bone.name, parent, bone.at[0], bone.at[1], bone.at[2]);
            }
        }
        // The other game's packed archives (`trp_data::xray_archive`): each
        // with its count of files and how long its table took, and the
        // names that have the word; with `get`, also each such file read,
        // unpacked and checked.
        (Some("xdb"), Some(part)) => {
            let stalker = trp_data::xray::Stalker::locate();
            let began = std::time::Instant::now();
            let archives = stalker.archives();
            println!("{} archives, their tables read in {:.2} s", archives.len(), began.elapsed().as_secs_f32());
            let get = args.get(2).is_some_and(|word| word == "get");
            for archive in archives {
                let names = archive.names_with(part);
                println!("{}: {} files, {} with {part:?}", archive.path.display(), archive.len(), names.len());
                for name in names.iter().take(40) {
                    if get {
                        let began = std::time::Instant::now();
                        match archive.read(name) {
                            Ok(Some(bytes)) => println!("  {name}: {} bytes, checked, in {:.1} ms", bytes.len(), began.elapsed().as_secs_f32() * 1000.0),
                            Ok(None) => println!("  {name}: not there"),
                            Err(e) => println!("  {name}: {e:#}"),
                        }
                    } else {
                        println!("  {name}");
                    }
                }
            }
        }
        // A piece of the character's clothing (a body's, a legs', a head's
        // or the arms' bundle): the bones its mesh hangs on, each where it
        // is at rest in the mesh's space.
        (Some("skin"), Some(key)) => {
            let part = trp_data::arms::load(&mut assets, key)?;
            let mesh = trp_data::unity::extract::read_mesh(&assets, part.mesh)?;
            println!("{key}: {} vertices, {} bones", mesh.positions.len(), part.bone_paths.len());
            for (path, bind) in part.bone_paths.iter().zip(&mesh.bind_poses) {
                match trp_data::outfit::rest_place(bind) {
                    Some(at) => println!("  {:7.3} {:7.3} {:7.3}  {path}", at[0], at[1], at[2]),
                    None => println!("  (no place)  {path}"),
                }
            }
        }
        (Some("weapons"), filter) => {
            // Every weapon of the database that has a stock preset: its
            // class, how many states and clips its animator has, and two
            // marks by which those with the same states, and with the same
            // clips, are told (a count of the install's distinct
            // animators, for a library of them).
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            const CLASSES: [&str; 12] = [
                "AssaultRifle", "AssaultCarbine", "Smg", "MarksmanRifle", "MachineGun", "SniperRifle", "Shotgun", "Pistol", "Revolver",
                "GrenadeLauncher", "SpecialWeapon", "RocketLauncher",
            ];
            let db = trp_data::Database::load(&assets.install)?;
            let mut items: Vec<_> = db.items.values().filter(|item| item.name.starts_with("weapon_") && db.is_a(item, "Weapon")).collect();
            items.sort_by(|a, b| a.name.cmp(&b.name));
            let mark = |words: &[String]| {
                let mut hasher = DefaultHasher::new();
                words.hash(&mut hasher);
                hasher.finish() & 0xFFFF_FFFF
            };
            for item in items {
                if filter.is_some_and(|f| !item.name.contains(f)) {
                    continue;
                }
                let class = CLASSES.iter().find(|class| db.is_a(item, class)).copied().unwrap_or("-");
                let read = (|| -> anyhow::Result<_> {
                    let weapon = trp_data::weapon::assemble_named(&mut assets, &db, &item.id)?;
                    let table = trp_data::states::StateTable::load(&assets, &weapon)?;
                    let mut states: Vec<String> = table.states.iter().map(|state| state.full_name.clone()).collect();
                    states.sort();
                    let mut clips: Vec<String> = table.states.iter().flat_map(|state| state.clips.iter().map(|clip| clip.0.clone())).collect();
                    clips.sort();
                    clips.dedup();
                    let layers: Vec<String> = table.layers.iter().map(|layer| layer.name.clone()).collect();
                    Ok((states.len(), clips.len(), mark(&states), mark(&clips), layers.join("+")))
                })();
                match read {
                    Ok((states, clips, states_mark, clips_mark, layers)) => {
                        println!("{class:<16} {:<52} {} states {states:>3} clips {clips:>3} same-states {states_mark:08x} same-clips {clips_mark:08x} layers {layers}", item.name, item.id)
                    }
                    Err(e) => println!("{class:<16} {:<52} {} not read: {e:#}", item.name, item.id),
                }
            }
        }
        (Some("states"), Some(template)) => {
            // Every animator state of a weapon with its clip and events.
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let table = trp_data::states::StateTable::load(&assets, &weapon)?;
            let filter = arg(2).map(str::to_lowercase);
            // `states <weapon> parameters`: the animator's parameters,
            // each with its kind and what it starts at.
            if filter.as_deref() == Some("parameters") {
                for p in &table.parameters {
                    println!("parameter {:?} {} = {}", p.kind, p.name, p.default);
                }
                return Ok(());
            }
            for layer in &table.layers {
                let on = layer.mask.iter().filter(|m| m.1 > 0.0).count();
                println!(
                    "layer {} (machine {}): weight {}, {}, moves {} of {} masked bones, starts in {}",
                    layer.name,
                    layer.machine,
                    layer.default_weight,
                    if layer.additive { "additive" } else { "override" },
                    on,
                    layer.mask.len(),
                    layer.default_state.as_deref().unwrap_or("-")
                );
            }
            for state in &table.states {
                if filter.as_ref().is_some_and(|f| !state.full_name.to_lowercase().contains(f)) {
                    continue;
                }
                let clip = state
                    .clips
                    .iter()
                    .map(|(name, obj)| {
                        format!("{name} {:.3} s", trp_data::states::clip_length(&assets, *obj).unwrap_or(-1.0))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                // With its short name's hash as a recording keeps it (24 bits).
                let own = state.full_name.rsplit('.').next().unwrap_or(&state.full_name);
                println!(
                    "[{}] {} #{}  speed {}{}  {clip}{}",
                    state.machine,
                    state.full_name,
                    trp_data::unity::anim::path_hash(own) & 0x00ff_ffff,
                    state.speed,
                    if state.looping { " loop" } else { "" },
                    match &state.blend_parameter {
                        Some(p) => format!("  by {p} at {:?}", state.thresholds),
                        None => String::new(),
                    }
                );
                for e in &state.events {
                    let conditions: Vec<String> = e
                        .conditions
                        .iter()
                        .map(|c| {
                            format!("{} kind{} mode{} {}/{}/{}", c.parameter, c.kind, c.mode, c.float_value, c.int_value, c.bool_value)
                        })
                        .collect();
                    println!(
                        "      {:.3} {}{}{}{}",
                        e.time,
                        e.name,
                        if e.string_param.is_empty() { String::new() } else { format!(" \"{}\"", e.string_param) },
                        if e.int_param != 0 { format!(" {}", e.int_param) } else { String::new() },
                        if conditions.is_empty() { String::new() } else { format!("   if {}", conditions.join(" & ")) }
                    );
                }
                for t in &state.transitions {
                    println!("      -> {}", describe_transition(t));
                }
            }
            for (machine, t) in &table.any_state {
                if filter.as_ref().is_some_and(|f| !t.to.as_deref().unwrap_or("").to_lowercase().contains(f)) {
                    continue;
                }
                println!("[{machine}] any state -> {}", describe_transition(t));
            }
        }
        (Some("controller"), Some(template)) => {
            // The weapon's animator controller, as stored: states, the
            // table that names them, and the scripts attached to states.
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let depth = arg(2).map_or(Ok(8), str::parse)?;
            let items = arg(3).map_or(Ok(40), str::parse)?;
            let mut controller = weapon
                .prefab
                .req("_originalAnimatorController")?
                .as_pptr()
                .and_then(|p| assets.resolve_loaded(weapon.prefab_file, p))
                .ok_or_else(|| anyhow::anyhow!("no animator controller"))?;
            let mut v = assets.read(controller)?;
            while let Some(base) =
                v.get("m_Controller").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(controller.file, p))
            {
                println!("override of:");
                controller = base;
                v = assets.read(controller)?;
            }
            println!("controller {} in file {}", assets.name_of(controller).unwrap_or_default(), controller.file);
            println!("{}", v.pretty(depth, items));
            let mut events_shown = false;
            for b in v.get("m_StateMachineBehaviours").and_then(|b| b.as_array()).unwrap_or(&[]) {
                let Some(obj) = b.as_pptr().and_then(|p| assets.resolve_loaded(controller.file, p)) else { continue };
                let script = trp_data::unity::extract::script_class(&assets, obj).unwrap_or_default();
                let behaviour = assets.read(obj)?;
                println!("behaviour {} [{script}]\n{}", obj.path_id, behaviour.pretty(4, 6));
                if events_shown {
                    continue;
                }
                if let Some(data) =
                    behaviour.get("EventsData").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(obj.file, p))
                {
                    println!("events data:\n{}", assets.read(data)?.pretty(depth, items));
                    events_shown = true;
                }
            }
        }
        (Some("engine"), _) => {
            println!("{:?}", trp_data::engine::EngineSettings::load(&assets.install)?);
            let hands = trp_data::player::HandsData::load(&mut assets)?;
            println!(
                "walk: step frequency {}, intensity {:?}, {} entries in the normal preset",
                hands.walk_step_frequency,
                hands.walk_intensity_range,
                hands.walk_normal.len()
            );
            for entry in &hands.walk_normal {
                let span = entry.curve.keys.last().map_or(0.0, |k| k.time);
                println!("  curve over 0..{span} shift {} vals {:?}", entry.time_shift, entry.vals);
            }
            println!("mouse: {:?}", hands.mouse.vals);
            println!("movement: {:?}", hands.movement.vals);
            println!("camera offset {:?}, sway {} / {}", hands.camera_offset, hands.sway_frequency, hands.sway_damping);
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, arg(1).unwrap_or("59d6088586f774275f37482f"))?;
            println!(
                "{}: {:.3} kg, ergonomics {}, {} cells wide",
                weapon.name, weapon.total_weight, weapon.total_ergonomics, weapon.cell_width
            );
        }
        (Some("hardsettings"), filter) => {
            let settings = trp_data::hard_settings::HardSettings::load(&assets.install)?;
            let wanted = |name: &str| filter.map_or(true, |f| name.to_lowercase().contains(&f.to_lowercase()));
            for (name, value) in settings.numbers() {
                if wanted(name) {
                    println!("{name} = {value}");
                }
            }
            for (name, curve) in settings.curves() {
                if wanted(name) {
                    let keys: Vec<String> = curve
                        .keys
                        .iter()
                        .map(|k| format!("({:.3}, {:.3} | {:.2} {:.2})", k.time, k.value, k.in_slope, k.out_slope))
                        .collect();
                    println!("{name}: {}", keys.join(" "));
                }
            }
            // The lists of the game's own numbers (movement states).
            for name in ["NO_AIM_STATES", "UnsuitableStates"] {
                if wanted(name) {
                    println!("{name}: {:?}", settings.bytes(name)?);
                }
            }
        }
        (Some("rawfile"), Some(rel)) => {
            // A serialized file that sits loose in the data folder, not in a bundle.
            use trp_data::unity::SerializedFile;
            let path = assets.install.root.join("EscapeFromTarkov_Data").join(rel);
            let data = std::fs::read(&path)?;
            let meta = SerializedFile::parse(&data)?;
            println!(
                "{} v{} {} typetrees={} objects={} externals={}",
                rel,
                meta.version,
                meta.unity_version,
                meta.has_type_trees,
                meta.objects.len(),
                meta.externals.len()
            );
            let filter = arg(2).unwrap_or("");
            for o in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
                // MonoBehaviour header: game object (12), enabled (4), script (12), then the name.
                let bytes = &data[o.offset..o.offset + o.size];
                if bytes.len() < 32 {
                    continue;
                }
                let len = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
                let Some(name) = bytes.get(32..32 + len).and_then(|b| std::str::from_utf8(b).ok()) else { continue };
                if !filter.is_empty() && name.to_lowercase().contains(&filter.to_lowercase()) {
                    let script_file = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
                    let script_path = i64::from_le_bytes(bytes[20..28].try_into().unwrap());
                    println!("  {:>12} size {:>7} script(file {script_file}, path {script_path})  {name}", o.path_id, o.size);
                }
            }
        }
        (Some("rootmotion"), filter) => {
            use trp_data::rootmotion::{Node, RootMotionTable};
            fn show(node: &Node, depth: usize, max_depth: usize) {
                let pad = "  ".repeat(depth + 1);
                match node {
                    Node::Clip { name, looping, length, velocity, uses_curves, .. } => println!(
                        "{pad}clip {name}  {length:.2}s loop={looping} v=({:.3} {:.3} {:.3}){}",
                        velocity[0],
                        velocity[1],
                        velocity[2],
                        if *uses_curves { " curves" } else { "" }
                    ),
                    Node::Tree { name, blend, param_x, param_y, thresholds, positions, children, steps_x, steps_y, .. } => {
                        let layout = if *blend == trp_data::rootmotion::BlendType::Simple1D {
                            format!("thresholds={thresholds:?}")
                        } else {
                            format!("positions={positions:?}")
                        };
                        println!(
                            "{pad}tree {name}  {blend:?} x={param_x:#010x} y={param_y:#010x} {layout} grid={steps_x}x{steps_y} children={}",
                            children.len()
                        );
                        if depth < max_depth {
                            for child in children {
                                show(child, depth + 1, max_depth);
                            }
                        }
                    }
                }
            }
            let table = RootMotionTable::load(&mut assets)?;
            for p in &table.parameters {
                println!("param {:<18} {:#010x}  {}..{} step {}", p.name, trp_data::unity::anim::path_hash(&p.name), p.min, p.max, p.step);
            }
            let depth: usize = arg(2).map_or(Ok(0), str::parse)?;
            let mut states: Vec<_> = table.states.iter().collect();
            states.sort_by(|a, b| a.1 .0.cmp(&b.1 .0));
            println!("{} states", states.len());
            for (hash, (name, node)) in states {
                if filter.is_some_and(|f| !name.to_lowercase().contains(&f.to_lowercase())) {
                    continue;
                }
                println!("state {name}  {hash:#010x}");
                show(node, 0, depth);
            }
        }
        (Some("clips"), Some(template)) => {
            use trp_data::unity::anim::{path_hash, Clip};
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let paths: std::collections::HashMap<u32, String> = (0..weapon.hierarchy.nodes.len())
                .map(|i| weapon.hierarchy.path(i))
                .map(|p| (path_hash(&p), p))
                .collect();
            let filter = arg(2).unwrap_or("");
            for (name, obj) in weapon.clips(&assets)? {
                if !name.contains(filter) {
                    continue;
                }
                let clip = Clip::read(&assets, obj)?;
                let bound = clip.tracks.iter().filter(|t| paths.contains_key(&t.path)).count();
                println!(
                    "{name:<34} -> {:<44} {:>6.2}s loop={} tracks={} on-rig={} from {}",
                    clip.name,
                    clip.duration(),
                    clip.looping,
                    clip.tracks.len(),
                    bound,
                    assets.file(obj.file).bundle_key.rsplit('/').next().unwrap_or("")
                );
                if arg(3) == Some("check") {
                    // Compares the clip's first frame with the prefab's rest pose.
                    use trp_data::unity::anim::{euler_to_quat, Property};
                    let nodes: std::collections::HashMap<u32, usize> = (0..weapon.hierarchy.nodes.len())
                        .map(|i| (path_hash(&weapon.hierarchy.path(i)), i))
                        .collect();
                    let mut values = Vec::new();
                    clip.sample(0.0, &mut values);
                    let (mut count, mut close, mut worst, mut custom) = (0, 0, (0.0f32, String::new()), 0);
                    for t in clip.tracks.iter().filter(|t| t.property == Property::Euler) {
                        custom += (t.custom_type != 0) as usize;
                        let Some(&node) = nodes.get(&t.path) else { continue };
                        let q = euler_to_quat([values[t.curve], values[t.curve + 1], values[t.curve + 2]]);
                        let rest = weapon.hierarchy.nodes[node].rotation;
                        let dot: f32 = q.iter().zip(&rest).map(|(a, b)| a * b).sum();
                        let angle = 2.0 * dot.abs().min(1.0).acos().to_degrees();
                        count += 1;
                        close += (angle < 1.0) as usize;
                        if angle > worst.0 {
                            worst = (angle, weapon.hierarchy.nodes[node].name.clone());
                        }
                    }
                    println!(
                        "    euler tracks on rig: {count}, within 1 degree of rest: {close}, largest difference {:.2} degrees at {}, non-zero customType: {custom}",
                        worst.0, worst.1
                    );
                }
                // `tracks [word] [time]`: the clip's tracks whose bone path
                // ends in the word, at that time (seconds; the start if
                // none is given).
                if !filter.is_empty() && arg(3) == Some("tracks") {
                    let word = arg(4).unwrap_or("");
                    let time = arg(5).and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0);
                    let mut values = Vec::new();
                    clip.sample(time, &mut values);
                    for t in &clip.tracks {
                        let path = paths.get(&t.path).map_or("<not on rig>", String::as_str);
                        if !path.ends_with(word) {
                            continue;
                        }
                        let n = if t.property == trp_data::unity::anim::Property::Rotation { 4 } else { 3 };
                        println!("    {:?} {path} at {time} s = {:?}", t.property, &values[t.curve..t.curve + n]);
                    }
                }
                // `path [word] [from] [to] [step]`: where the first bone
                // whose path ends in the word is through the clip, in the
                // rig's own frame: every bone above it as the clip has it
                // (as the prefab has it where the clip says nothing), one
                // line a time, with how far it has turned from the line
                // before and its speed. A break in the speed at a steady
                // period is the clip's own: curves kept as samples run in
                // straight lines from one to the next.
                if !filter.is_empty() && arg(3) == Some("path") {
                    use trp_data::unity::anim::{euler_to_quat, Property};
                    let word = arg(4).unwrap_or("");
                    let figure = |index: usize, otherwise: f32| arg(index).and_then(|t| t.parse::<f32>().ok()).unwrap_or(otherwise);
                    let (from, to, step) = (figure(5, 0.0), figure(6, clip.duration()), figure(7, 1.0 / 120.0).max(1e-4));
                    let Some(bone) = (0..weapon.hierarchy.nodes.len()).find(|&i| weapon.hierarchy.path(i).ends_with(word)) else {
                        println!("    no bone's path ends in {word}");
                        continue;
                    };
                    // The bone and everything above it, the bone first.
                    let mut chain = vec![bone];
                    while let Some(parent) = weapon.hierarchy.nodes[*chain.last().unwrap()].parent {
                        chain.push(parent);
                    }
                    let tracks_of = |node: usize| {
                        let hash = path_hash(&weapon.hierarchy.path(node));
                        let find = |wanted: &[Property]| clip.tracks.iter().find(|t| t.path == hash && wanted.contains(&t.property)).copied();
                        (find(&[Property::Position]), find(&[Property::Rotation, Property::Euler]))
                    };
                    let tracks: Vec<_> = chain.iter().map(|&node| tracks_of(node)).collect();
                    let (cubics, samples, rate, _) = clip.kept();
                    for (&node, (place, turn)) in chain.iter().zip(&tracks) {
                        let kept = |curve: Option<usize>| match curve {
                            None => "the prefab's",
                            Some(c) if c < cubics => "cubics",
                            Some(c) if c < cubics + samples => "samples",
                            Some(_) => "one figure",
                        };
                        println!("    {}: place {}, turn {}", weapon.hierarchy.path(node), kept(place.map(|t| t.curve)), kept(turn.map(|t| t.curve)));
                    }
                    println!("    samples are {rate} a second");
                    let mul = |a: [f32; 4], b: [f32; 4]| -> [f32; 4] {
                        [
                            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
                            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
                            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
                            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
                        ]
                    };
                    let turned = |q: [f32; 4], v: [f32; 3]| -> [f32; 3] {
                        let p = mul(mul(q, [v[0], v[1], v[2], 0.0]), [-q[0], -q[1], -q[2], q[3]]);
                        [p[0], p[1], p[2]]
                    };
                    let mut values = Vec::new();
                    let mut last: Option<([f32; 3], [f32; 4])> = None;
                    let mut time = from;
                    while time <= to + 1e-6 {
                        clip.sample(time, &mut values);
                        // From the bone up: each bone's place and turn in
                        // its parent takes what is below it along.
                        let (mut at, mut facing) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0]);
                        for (&node, (place, turn)) in chain.iter().zip(&tracks) {
                            let rest = &weapon.hierarchy.nodes[node];
                            let p = place.map_or(rest.translation, |t| [values[t.curve], values[t.curve + 1], values[t.curve + 2]]);
                            let q = match turn {
                                Some(t) if t.property == Property::Euler => euler_to_quat([values[t.curve], values[t.curve + 1], values[t.curve + 2]]),
                                Some(t) => [values[t.curve], values[t.curve + 1], values[t.curve + 2], values[t.curve + 3]],
                                None => rest.rotation,
                            };
                            let moved = turned(q, at);
                            at = [moved[0] + p[0], moved[1] + p[1], moved[2] + p[2]];
                            facing = mul(q, facing);
                        }
                        let mut line = format!("    {time:.4} sample {:6.2}  at {:8.2} {:8.2} {:8.2} mm", time * rate, at[0] * 1000.0, at[1] * 1000.0, at[2] * 1000.0);
                        if let Some((was, faced)) = last {
                            let dot: f32 = (0..4).map(|k| faced[k] * facing[k]).sum();
                            let angle = 2.0 * dot.abs().min(1.0).acos().to_degrees();
                            line.push_str(&format!(
                                "  speed {:8.1} {:8.1} {:8.1} mm/s  turning {:7.1} deg/s",
                                (at[0] - was[0]) / step * 1000.0,
                                (at[1] - was[1]) / step * 1000.0,
                                (at[2] - was[2]) / step * 1000.0,
                                angle / step
                            ));
                        }
                        println!("{line}");
                        last = Some((at, facing));
                        time += step;
                    }
                }
                // `series [word] [from] [to] [step]`: the same tracks at
                // each of a run of times, one line a time, and how each is
                // kept (as cubics from key to key, as samples at a steady
                // rate with straight lines between, or as one figure).
                if !filter.is_empty() && arg(3) == Some("series") {
                    let word = arg(4).unwrap_or("");
                    let figure = |index: usize, otherwise: f32| arg(index).and_then(|t| t.parse::<f32>().ok()).unwrap_or(otherwise);
                    let (from, to, step) = (figure(5, 0.0), figure(6, clip.duration()), figure(7, 1.0 / 120.0).max(1e-4));
                    let chosen: Vec<_> = clip.tracks.iter().filter(|t| paths.get(&t.path).is_some_and(|p| p.ends_with(word))).collect();
                    let (cubics, samples, rate, figures) = clip.kept();
                    println!("    {cubics} curves as cubics, {samples} as samples at {rate} a second, {figures} that never change");
                    for t in &chosen {
                        let kept = if t.curve < cubics {
                            "cubics"
                        } else if t.curve < cubics + samples {
                            "samples"
                        } else {
                            "one figure"
                        };
                        println!("    {:?} {} ({kept}, curve {})", t.property, paths.get(&t.path).map_or("", String::as_str), t.curve);
                    }
                    let mut values = Vec::new();
                    let mut time = from;
                    while time <= to + 1e-6 {
                        clip.sample(time, &mut values);
                        let mut line = format!("    {time:.4}");
                        for t in &chosen {
                            let n = if t.property == trp_data::unity::anim::Property::Rotation { 4 } else { 3 };
                            for value in &values[t.curve..t.curve + n] {
                                line.push_str(&format!(" {value:.5}"));
                            }
                            line.push_str(" |");
                        }
                        println!("{line}");
                        time += step;
                    }
                }
            }
        }
        (Some("blendcheck"), filter) => {
            // Freeform directional trees of the root motion table: where the
            // stored weights and the computed ones part.
            use trp_data::rootmotion::{freeform_directional, BlendType, Node, RootMotionTable};
            let table = RootMotionTable::load(&mut assets)?;
            fn visit(table: &RootMotionTable, state: &str, node: &Node, limit: usize) {
                let Node::Tree { blend, param_x, param_y, positions, steps_x, steps_y, weights, children, .. } = node else {
                    return;
                };
                if *blend == BlendType::FreeformDirectional2D && positions.len() == children.len() {
                    let range = |hash: u32| table.parameters.iter().find(|p| trp_data::unity::anim::path_hash(&p.name) == hash);
                    let (Some(px), Some(py)) = (range(*param_x), range(*param_y)) else { return };
                    let n = children.len();
                    println!("{state}: {} by {} over {:?}, grid {steps_x}x{steps_y}", px.name, py.name, positions);
                    let mut shown = 0;
                    for x in 0..*steps_x {
                        for y in 0..*steps_y {
                            let at = [px.min + x as f32 * px.step, py.min + y as f32 * py.step];
                            let mine = freeform_directional(positions, at);
                            let stored = &weights[(x * steps_y + y) * n..(x * steps_y + y + 1) * n];
                            let gap = mine.iter().zip(stored).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
                            if gap > 0.01 && shown < limit {
                                shown += 1;
                                println!("   at {at:?}: stored {stored:.3?}  computed {mine:.3?}");
                            }
                        }
                    }
                }
                for child in children {
                    visit(table, state, child, limit);
                }
            }
            for (name, node) in table.states.values() {
                if filter.is_none_or(|f| name.contains(f)) {
                    visit(&table, name, node, 6);
                }
            }
            // Which weight of direction against distance fits the stored
            // grids best: the largest and the mean difference for each.
            fn scan(table: &RootMotionTable, node: &Node, k: f32, worst: &mut f32, sum: &mut f64, count: &mut u64) {
                let Node::Tree { blend, param_x, param_y, positions, steps_x, steps_y, weights, children, .. } = node else {
                    return;
                };
                if *blend == BlendType::FreeformDirectional2D && positions.len() == children.len() {
                    let range = |hash: u32| table.parameters.iter().find(|p| trp_data::unity::anim::path_hash(&p.name) == hash);
                    if let (Some(px), Some(py)) = (range(*param_x), range(*param_y)) {
                        let n = children.len();
                        for x in 0..*steps_x {
                            for y in 0..*steps_y {
                                let at = [px.min + x as f32 * px.step, py.min + y as f32 * py.step];
                                let mine = trp_data::rootmotion::freeform_directional_weighted(positions, at, k);
                                for c in 0..n {
                                    let gap = (mine[c] - weights[(x * steps_y + y) * n + c]).abs();
                                    *worst = worst.max(gap);
                                    *sum += gap as f64;
                                    *count += 1;
                                }
                            }
                        }
                    }
                }
                for child in children {
                    scan(table, child, k, worst, sum, count);
                }
            }
            for k in [0.3183, 0.6366, 0.9, 0.98, 1.0, 1.01, 1.02, 1.1, 1.5708, 2.0] {
                let (mut worst, mut sum, mut count) = (0.0f32, 0.0f64, 0u64);
                for (_, node) in table.states.values() {
                    scan(&table, node, k, &mut worst, &mut sum, &mut count);
                }
                println!("direction weight {k}: largest difference {worst:.4}, mean {:.6}", sum / count.max(1) as f64);
            }
        }
        (Some("bodystates"), filter) => {
            // States of the body animator with their clips and transitions.
            let files = assets.load("assets/content/characters/controllers/player_anim_controller.bundle")?;
            // The game asks for the controller of this name. `-` as the
            // filter shows every state.
            let wanted = arg(2).unwrap_or("Player_anim_controller");
            let filter = filter.filter(|f| *f != "-").map(str::to_lowercase);
            for &file in &files {
                for obj in assets.objects_of(file, class::ANIMATOR_CONTROLLER).collect::<Vec<_>>() {
                    let name = assets.name_of(obj).unwrap_or_default();
                    println!("controller {name}");
                    if name != wanted {
                        continue;
                    }
                    let table = trp_data::states::StateTable::from_controller(&assets, obj, &Default::default())?;
                    if arg(3) == Some("parameters") {
                        for p in &table.parameters {
                            println!("parameter {:?} {} = {}", p.kind, p.name, p.default);
                        }
                        for (machine, list) in table.selectors.iter().enumerate() {
                            for (index, selector) in list.iter().enumerate() {
                                let choices: Vec<String> = selector
                                    .choices
                                    .iter()
                                    .map(|c| {
                                        format!(
                                            "{}{} ({} conditions)",
                                            c.to.as_deref().unwrap_or(""),
                                            c.to_selector.map_or(String::new(), |s| format!("point {s}")),
                                            c.conditions.len()
                                        )
                                    })
                                    .collect();
                                println!(
                                    "[{machine}] {} point {index}: {}",
                                    if selector.entry { "entry" } else { "exit" },
                                    choices.join(" | ")
                                );
                            }
                        }
                    }
                    for layer in &table.layers {
                        println!(
                            "layer {} (machine {}, slot {}): weight {}, {}, {} masked bones, starts in {}",
                            layer.name,
                            layer.machine,
                            layer.sync_slot,
                            layer.default_weight,
                            if layer.additive { "additive" } else { "override" },
                            layer.mask.len(),
                            layer.default_state.as_deref().unwrap_or("-")
                        );
                    }
                    for state in &table.states {
                        if filter.as_ref().is_some_and(|f| !state.full_name.to_lowercase().contains(f)) {
                            continue;
                        }
                        let clips: Vec<&str> = state.clips.iter().map(|c| c.0.as_str()).collect();
                        println!(
                            "[{}] {}  speed {}{}{}{}  {}{}",
                            state.machine,
                            state.full_name,
                            state.speed,
                            state.speed_parameter.as_ref().map_or(String::new(), |p| format!(" x {p}")),
                            state.movement.as_ref().map_or(String::new(), |m| {
                                format!(
                                    "  <movement {}{}{}{}{}{}{}{}{}>",
                                    m.state,
                                    if m.can_interact { " interacts" } else { "" },
                                    if m.sensitivity != 1.0 || m.rotation_speed_clamp != 99.0 {
                                        format!(" turning x{} up to {}", m.sensitivity, m.rotation_speed_clamp)
                                    } else {
                                        String::new()
                                    },
                                    if m.default_state { " default" } else { "" },
                                    if m.disable_root_motion { " no-root-motion" } else { "" },
                                    match m.kind {
                                        1 => " in",
                                        2 => " out",
                                        _ => "",
                                    },
                                    match m.direction {
                                        0 => String::new(),
                                        1 => " right".to_owned(),
                                        5 => " left".to_owned(),
                                        other => format!(" direction {other}"),
                                    },
                                    if m.length > 0.0 { format!(" length {:.3} s", m.length) } else { String::new() },
                                    if m.unique { " own-object" } else { "" }
                                )
                            }),
                            if state.looping { " loop" } else { "" },
                            clips.join(", "),
                            state.blend_parameter.as_ref().map_or(String::new(), |p| format!("  by {p}"))
                        );
                        if let Some(pose) = state.movement.as_ref().and_then(|m| m.first_person_pose.as_ref()) {
                            let keys: Vec<String> =
                                pose.blend.keys.iter().map(|k| format!("{:.3}: {:.3}", k.time, k.value)).collect();
                            println!(
                                "      first-person pose {}: position {:?}, rotation {:?}, camera rotation {:?}, camera position {:?}, blend {}",
                                pose.name,
                                pose.position,
                                pose.rotation,
                                pose.camera_rotation,
                                pose.camera_position,
                                keys.join(", ")
                            );
                        }
                        // The marks the game's logic waits for, each by when
                        // in the state it comes.
                        for e in &state.events {
                            println!("      {:.3} {}{}", e.time, e.name, if e.enabled { "" } else { " (off)" });
                        }
                        for t in &state.transitions {
                            println!("      -> {}", describe_transition(t));
                        }
                        if arg(3) == Some("tree") {
                            println!("      trees by layer: {:?}", state.tree_of_layer);
                            for (t, i, node) in
                                state.trees.iter().enumerate().flat_map(|(t, tree)| tree.iter().enumerate().map(move |(i, n)| (t, i, n)))
                            {
                                let i = format!("{t}.{i}");
                                match &node.clip {
                                    Some((name, _)) => {
                                        println!("      node {i}: clip {name}  length x {:.3}  offset {}", node.duration, node.cycle_offset)
                                    }
                                    None => println!(
                                        "      node {i}: blend type {} by {} / {}  children {:?}  thresholds {:?}  positions {:?}",
                                        node.blend_type,
                                        node.parameter_x.as_deref().unwrap_or("-"),
                                        node.parameter_y.as_deref().unwrap_or("-"),
                                        node.children,
                                        node.thresholds,
                                        node.positions
                                    ),
                                }
                            }
                        }
                    }
                    for (machine, t) in &table.any_state {
                        if filter.as_ref().is_some_and(|f| !t.to.as_deref().unwrap_or("").to_lowercase().contains(f)) {
                            continue;
                        }
                        println!("[{machine}] any state -> {}", describe_transition(t));
                    }
                }
            }
        }
        (Some("bodyclips"), Some(filter)) => {
            // Clips of the body animator: what they move on the player's
            // skeleton and which animator parameters they carry curves for.
            use trp_data::unity::anim::{euler_to_quat, path_hash, Clip, Property};
            let player = assets.load("assets/content/commonprefabs/playersuperior.bundle")?;
            let mut paths: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
            for &file in &player {
                for (_, root) in trp_data::unity::extract::exported_game_objects(&assets, file)? {
                    let hierarchy = trp_data::unity::extract::read_hierarchy(&assets, root)?;
                    for i in 0..hierarchy.nodes.len() {
                        let path = hierarchy.path(i);
                        // The animator sits on `Player`, so its paths start below that.
                        if let Some(below) = path.strip_prefix("Player/") {
                            paths.insert(path_hash(below), below.to_owned());
                        }
                    }
                }
            }
            const CURVES: [&str; 15] = [
                "Turn",
                "Weapon_3rd",
                "First_Person_Curve_Weight",
                "Position_cache",
                "Hand_Left",
                "Hand_Right",
                "Shoulder_Left",
                "Shoulder_Right",
                "Elbow_Left",
                "Elbow_Right",
                "Leftstance_curve",
                "Mouse_look",
                "Aiming_Layer",
                "FirstAction",
                "FootStep",
            ];
            let curve_names: std::collections::HashMap<u32, &str> = CURVES.iter().map(|n| (path_hash(n), *n)).collect();
            let wanted = arg(3).unwrap_or("");
            let files = assets.load("assets/content/characters/controllers/player_anim_controller.bundle")?;
            let all: Vec<usize> = (0..assets.files().len()).collect();
            let _ = files;
            for file in all {
                for obj in assets.objects_of(file, class::ANIMATION_CLIP).collect::<Vec<_>>() {
                    let Some(name) = assets.name_of(obj) else { continue };
                    if !name.to_lowercase().contains(&filter.to_lowercase()) {
                        continue;
                    }
                    let clip = Clip::read(&assets, obj)?;
                    let on_rig = clip.tracks.iter().filter(|t| paths.contains_key(&t.path)).count();
                    println!(
                        "{name:<44} {:>6.2}s loop={} tracks={} on-skeleton={} floats={} from {}",
                        clip.duration(),
                        clip.looping,
                        clip.tracks.len(),
                        on_rig,
                        clip.floats.len(),
                        assets.file(file).bundle_key.rsplit('/').next().unwrap_or("")
                    );
                    if arg(2) != Some("tracks") {
                        continue;
                    }
                    let mut values = Vec::new();
                    for f in &clip.floats {
                        let samples: Vec<String> = (0..=4)
                            .map(|i| {
                                clip.sample(clip.duration() * i as f32 / 4.0, &mut values);
                                format!("{:.3}", values[f.curve])
                            })
                            .collect();
                        let label = curve_names.get(&f.name).map_or_else(|| format!("#{:08x}", f.name), |n| (*n).to_owned());
                        println!("    curve {label} (custom {}) = {}", f.custom_type, samples.join(" "));
                    }
                    clip.sample(0.0, &mut values);
                    for t in &clip.tracks {
                        let Some(path) = paths.get(&t.path) else { continue };
                        if !path.contains(wanted) {
                            continue;
                        }
                        let v = &values[t.curve..];
                        match t.property {
                            Property::Euler => println!("    {path}: euler {:?} = quat {:?}", &v[..3], euler_to_quat([v[0], v[1], v[2]])),
                            Property::Rotation => println!("    {path}: rotation {:?}", &v[..4]),
                            Property::Position => println!("    {path}: position {:?}", &v[..3]),
                            Property::Scale => {}
                        }
                    }
                }
            }
        }
        (Some("texstats"), Some(key)) => {
            use trp_data::unity::{extract::read_texture, texture};
            let filter = arg(2).unwrap_or("");
            for file in assets.load(key)? {
                for obj in assets.objects_of(file, class::TEXTURE_2D).collect::<Vec<_>>() {
                    let tex = read_texture(&assets, obj)?;
                    if !tex.name.contains(filter) {
                        continue;
                    }
                    let mip = texture::mips(&tex)?[0];
                    let Ok(rgba) = texture::decode_rgba(&tex, mip) else {
                        println!("{:<60} fmt{} (not decodable)", tex.name, tex.format);
                        continue;
                    };
                    let mut stats = [(255u8, 0u8, 0u64); 4];
                    for px in rgba.chunks_exact(4) {
                        for c in 0..4 {
                            let s = &mut stats[c];
                            *s = (s.0.min(px[c]), s.1.max(px[c]), s.2 + px[c] as u64);
                        }
                    }
                    // `texstats <bundle> <word> <folder>`: each picture is
                    // also written there to be looked at, at the level
                    // nearest 512 across and with nothing see-through (a
                    // folder of scratch: the pictures are the install's).
                    if let Some(folder) = arg(3) {
                        let levels = texture::mips(&tex)?;
                        let level = levels.iter().copied().find(|level| level.width <= 512).unwrap_or(mip);
                        let mut small = texture::decode_rgba(&tex, level)?;
                        small.chunks_exact_mut(4).for_each(|px| px[3] = 255);
                        std::fs::create_dir_all(folder)?;
                        let out = std::path::Path::new(folder).join(format!("{}.png", tex.name));
                        let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&out)?), level.width, level.height);
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder.write_header()?.write_image_data(&small)?;
                        println!("    written to {}", out.display());
                    }
                    let n = (rgba.len() / 4) as u64;
                    let chan = |c: usize| format!("{}..{} avg {}", stats[c].0, stats[c].1, stats[c].2 / n);
                    println!(
                        "{:<60} fmt{} srgb={} R[{}] G[{}] B[{}] A[{}]",
                        tex.name, tex.format, tex.srgb, chan(0), chan(1), chan(2), chan(3)
                    );
                }
            }
        }
        // An item's model as it lies loose (`ObjectsFactory.CreateLootPrefab`:
        // a weapon with its parts, any other item its prefab), and every
        // collider on it: its object, shape, layer, whether a trigger or
        // switched off, and its material.
        (Some("colliders"), Some(template)) => {
            let db = trp_data::Database::load(&assets.install)?;
            let item = db.item(template)?;
            let hierarchy = if db.is_a(item, "Weapon") {
                trp_data::weapon::assemble(&mut assets, &db, template)?.hierarchy
            } else {
                let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("{template} has no model"))?.to_owned();
                trp_data::icon::model_of(&mut assets, &key)?.0
            };
            println!("{} ({}): {} objects", item.name, template, hierarchy.nodes.len());
            for (index, node) in hierarchy.nodes.iter().enumerate() {
                for c in &node.colliders {
                    println!("  {index:>3} {:<28} layer {:>2}{}{} {:?} {:?}", node.name, c.layer, if c.trigger { " trigger" } else { "" }, if c.enabled { "" } else { " off" }, c.shape, c.material);
                }
            }
        }
        // The game's views of a thing (`itemviews [width] [height]`, units:
        // a grid's view of one cell unless told): each object of the slot's
        // and the grid's layout placed in that size, and each text's
        // alignment and margins (left, top, right, bottom).
        (Some("itemviews"), _) => {
            let views = trp_data::screen::ItemViews::load(&assets.install)?;
            let size = [arg(1).and_then(|w| w.parse().ok()).unwrap_or(63.0), arg(2).and_then(|h| h.parse().ok()).unwrap_or(63.0)];
            for (name, panel) in [("slot", &views.slot), ("grid", &views.grid)] {
                let Some(panel) = panel else { continue };
                println!("== {name}");
                for (index, node) in panel.nodes.iter().enumerate() {
                    let [x, y, w, h] = panel.placed(index, size).rect;
                    let text = node.text.as_ref().map(|t| format!(" text {:?} size {} align {}/{} margin {:?}", t.text, t.size, t.horizontal, t.vertical, t.margin)).unwrap_or_default();
                    println!("  {:50} {} ({x:.1}, {y:.1}) {w:.1} x {h:.1}{text}", node.path, if node.active { "on " } else { "off" });
                }
            }
        }
        // The aiming's sounds (`aimsounds`): each kind of weapon's bank,
        // its volume, rolloff and clips.
        (Some("aimsounds"), _) => {
            for (kind, bank) in trp_data::sound::aiming_sounds(&assets.install)? {
                let clips: Vec<String> = bank.clips.iter().map(|clip| format!("{} ({:.2} s, x{})", clip.name, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32, clip.channels)).collect();
                println!("{kind}: {}, volume {}, rolloff {}: {}", bank.name, bank.volume, bank.rolloff, clips.join(", "));
            }
        }
        // An object a bundle's object points to, by the field's name
        // (`follow <bundle> <path id> <field> [depth] [items]`), dumped.
        (Some("follow"), Some(key)) => {
            let path_id: i64 = arg(2).ok_or_else(|| anyhow::anyhow!("path id needed"))?.parse()?;
            let field = arg(3).ok_or_else(|| anyhow::anyhow!("field needed"))?;
            let depth = arg(4).map_or(Ok(6), str::parse)?;
            let items = arg(5).map_or(Ok(12), str::parse)?;
            for file in assets.load(key)? {
                let obj = ObjRef { file, path_id };
                if assets.info(obj).is_none() {
                    continue;
                }
                let value = assets.read(obj)?;
                let target = value.get(field).and_then(|v| v.as_pptr()).and_then(|p| assets.resolve_loaded(file, p)).ok_or_else(|| anyhow::anyhow!("{field} leads nowhere loaded"))?;
                println!("{}", assets.read(target)?.pretty(depth, items));
            }
        }
        // Every object of a bundle: its path id, its class and its name, if
        // it has one (`objects <bundle>`); to find what to `dump`.
        (Some("objects"), Some(key)) => {
            for file in assets.load(key)? {
                let objects: Vec<(i64, i32)> = assets.file(file).meta.objects.iter().map(|o| (o.path_id, o.class_id)).collect();
                for (path_id, class_id) in objects {
                    let name = assets.read(ObjRef { file, path_id }).ok().and_then(|v| v.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned)).unwrap_or_default();
                    println!("{path_id:>22}  class {class_id:>4}  {name}");
                }
            }
        }
        // The game's audio mixer (`mixer`, the bundle
        // `assets/content/audio/mastermixer.bundle`): its groups as a tree,
        // each with its volume in each snapshot (dB, from the snapshot's
        // values at the group's `volumeIndex`), whether a script may set it
        // (an exposed parameter), and the kinds of its effects.
        (Some("mixer"), _) => {
            let mixer = trp_data::sound::master_mixer(&assets.install)?;
            println!("start snapshot: {}", mixer.start_snapshot);
            fn walk(mixer: &trp_data::sound::Mixer, at: usize, depth: usize) {
                let group = &mixer.groups[at];
                let effects: Vec<String> = group
                    .effects
                    .iter()
                    .map(|e| if e.parameters.is_empty() { e.kind.to_string() } else { format!("{}[{}]", e.kind, e.parameters.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(" ")) })
                    .collect();
                println!("{}{}: {:+.1} dB{} (path {:+.1} dB); effects {}", "  ".repeat(depth), group.name, group.volume_db, if group.exposed { " exposed" } else { "" }, mixer.path_db(at), effects.join(" "));
                for (child, g) in mixer.groups.iter().enumerate() {
                    if g.parent == Some(at) {
                        walk(mixer, child, depth + 1);
                    }
                }
            }
            for (at, group) in mixer.groups.iter().enumerate() {
                if group.parent.is_none() {
                    walk(&mixer, at, 0);
                }
            }
            println!("the master's compressor: {:?}", mixer.master_compressor());
        }
        (Some("tree"), Some(key)) => {
            for file in assets.load(key)? {
                print_tree(&assets, file)?;
            }
        }
        // Each Animator of a bundle: the object it is on and the controller
        // it names, through any override to the controller under it.
        (Some("animator"), Some(key)) => {
            for file in assets.load(key)? {
                for obj in assets.objects_of(file, class::ANIMATOR).collect::<Vec<_>>() {
                    let v = assets.read(obj)?;
                    let owner = v.get("m_GameObject").and_then(|g| g.as_pptr()).and_then(|p| assets.resolve_loaded(file, p)).and_then(|go| assets.name_of(go)).unwrap_or_default();
                    let mut controller = v.get("m_Controller").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(file, p));
                    let mut chain = Vec::new();
                    while let Some(at) = controller {
                        chain.push(format!("{} (file {}, {})", assets.name_of(at).unwrap_or_default(), at.file, at.path_id));
                        controller = assets.read(at).ok().and_then(|c| c.get("m_Controller").and_then(|p| p.as_pptr()).and_then(|p| assets.resolve_loaded(at.file, p)));
                    }
                    println!("animator on {owner}: {}", if chain.is_empty() { "no controller".to_owned() } else { chain.join(" over ") });
                }
            }
        }
        (Some("scripts"), Some(key)) => {
            for file in assets.load(key)? {
                for obj in assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
                    let owner = assets
                        .read(obj)
                        .ok()
                        .and_then(|v| v.get("m_GameObject").and_then(|g| g.as_pptr()))
                        .and_then(|p| assets.resolve_loaded(file, p))
                        .and_then(|go| assets.read(go).ok())
                        .and_then(|go| go.get("m_Name").and_then(|n| n.as_str()).map(str::to_owned))
                        .unwrap_or_default();
                    println!("{:>20} {:<34} {owner}", obj.path_id, script_name(&assets, obj));
                }
            }
        }
        // One of the install's weapons at rest from its side and from
        // above, and over it, half seen through, a model of one's own
        // (a binary FBX file whose barrel runs along -z with y up), the
        // two laid with their triggers together: `overlay <weapon> <fbx>
        // <the fbx's trigger model> <out.png> [pixels a centimetre]
        // [ahead behind above below]`. The crosses are the weapon's
        // markers for the palms (left blue, right green) at rest.
        (Some("overlay"), Some(template)) => {
            use trp_data::unity::extract::read_mesh;
            let (Some(file), Some(trigger_model), Some(out)) = (arg(2), arg(3), arg(4)) else { bail!("overlay <weapon> <fbx> <trigger model> <out.png>") };
            let scale = arg(5).and_then(|s| s.parse().ok()).unwrap_or(12.0);
            let db = trp_data::Database::load(&assets.install)?;
            let weapon = trp_data::weapon::assemble_named(&mut assets, &db, template)?;
            let tree = &weapon.hierarchy;
            let root = (0..tree.nodes.len()).find(|&i| tree.path(i) == "Weapon_root/Weapon_root_anim/weapon").ok_or_else(|| anyhow::anyhow!("no weapon bone"))?;
            // A point of a node, on the weapon's own bone at rest.
            let on_weapon = |node: usize, mut p: [f32; 3]| -> Option<[f32; 3]> {
                let mut at = node;
                while at != root {
                    let n = &tree.nodes[at];
                    let q = n.rotation;
                    let v: [f32; 3] = std::array::from_fn(|k| p[k] * n.scale[k]);
                    // The turn of a vector by a quaternion.
                    let t = [2.0 * (q[1] * v[2] - q[2] * v[1]), 2.0 * (q[2] * v[0] - q[0] * v[2]), 2.0 * (q[0] * v[1] - q[1] * v[0])];
                    let turned = [v[0] + q[3] * t[0] + q[1] * t[2] - q[2] * t[1], v[1] + q[3] * t[1] + q[2] * t[0] - q[0] * t[2], v[2] + q[3] * t[2] + q[0] * t[1] - q[1] * t[0]];
                    p = std::array::from_fn(|k| turned[k] + n.translation[k]);
                    at = n.parent?;
                }
                Some(p)
            };
            let mut theirs: Vec<[[f32; 3]; 3]> = Vec::new();
            let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
            for (index, node) in tree.nodes.iter().enumerate() {
                let Some(renderer) = &node.renderer else { continue };
                let path = tree.path(index).to_lowercase();
                if node.name.contains("LOD1") || node.name.contains("LOD2") || path.contains("base human") {
                    continue;
                }
                let Ok(mesh) = read_mesh(&assets, renderer.mesh) else { continue };
                let Some(placed) = mesh.positions.iter().map(|p| on_weapon(index, *p)).collect::<Option<Vec<_>>>() else { continue };
                if node.name.to_lowercase().contains("trigger") {
                    for p in &placed {
                        (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
                    }
                }
                for triangle in mesh.submeshes.iter().flat_map(|indices| indices.chunks_exact(3)) {
                    if triangle.iter().all(|&i| (i as usize) < placed.len()) {
                        theirs.push([0, 1, 2].map(|k| placed[triangle[k] as usize]));
                    }
                }
            }
            if low[0] > high[0] {
                bail!("the weapon has no trigger mesh to lay the two together by");
            }
            let trigger: [f32; 3] = std::array::from_fn(|k| (low[k] + high[k]) * 0.5);
            // On the weapon's bone the barrel runs along -y and up is z.
            let ours_of = |p: [f32; 3]| [-(p[1] - trigger[1]) * 100.0, (p[2] - trigger[2]) * 100.0, (p[0] - trigger[0]) * 100.0];
            let mut triangles: Vec<Drawn> = theirs.iter().map(|corners| (corners.map(ours_of), [150, 155, 165], 1.0)).collect();
            let scene = trp_data::fbx::read(&std::fs::read(file)?)?;
            let unit = scene.unit as f32;
            let blade = scene.models.iter().find(|model| model.name == trigger_model).and_then(|model| model.mesh.as_ref()).ok_or_else(|| anyhow::anyhow!("the file has no model {trigger_model} with a mesh"))?;
            let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &blade.positions {
                (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
            }
            let middle: [f32; 3] = std::array::from_fn(|k| (low[k] + high[k]) * 0.5);
            println!("the weapon's trigger is at {trigger:.4?} on its bone; the model's at {middle:.2?}");
            let skip: Vec<&str> = args.iter().filter_map(|a| a.strip_prefix("--without=")).collect();
            for model in scene.models.iter().filter(|model| !skip.contains(&model.name.as_str())) {
                let Some(mesh) = &model.mesh else { continue };
                for corners in mesh.positions.chunks_exact(3) {
                    let placed = [0, 1, 2].map(|k| [-(corners[k][2] - middle[2]) * unit, (corners[k][1] - middle[1]) * unit, (corners[k][0] - middle[0]) * unit]);
                    triangles.push((placed, [235, 120, 30], 0.5));
                }
            }
            let named = |name: &str| (0..tree.nodes.len()).find(|&i| tree.nodes[i].name == name).and_then(|i| on_weapon(i, [0.0; 3])).map(ours_of);
            let mut marks = Vec::new();
            marks.extend(named("weapon_L_IK_marker").map(|at| (at, [30, 60, 220])));
            marks.extend(named("weapon_R_IK_marker").map(|at| (at, [20, 150, 40])));
            return trp_data::sketch::views(&triangles, &marks, &[], out, scale, window_of(&args, 6));
        }
        // A weapon that is not the install's (`trp_data::custom`), by its
        // item template: what its file made of it. `custom` alone lists
        // them, with whatever was left out and why.
        (Some("custom"), which) => {
            let db = trp_data::Database::load(&assets.install)?;
            for note in &db.custom_notes {
                println!("left out: {note}");
            }
            for (id, definition) in &db.custom {
                println!("{id} {} on the rig of {}", definition.name, db.item(&definition.donor)?.name);
            }
            let Some(id) = which else { return Ok(()) };
            let weapon = trp_data::weapon::assemble(&mut assets, &db, id)?;
            let custom = weapon.custom.as_ref().ok_or_else(|| anyhow::anyhow!("{id} is one of the install's own"))?;
            println!(
                "{}: {:.3} kg loaded, ergonomics {}, {} cells wide, a magazine of {:?}, held closer {}",
                weapon.name, weapon.total_weight, weapon.total_ergonomics, weapon.cell_width, weapon.magazine_capacity, weapon.held_closer
            );
            for part in &custom.parts {
                let triangles: usize = part.mesh.submeshes.iter().map(|s| s.len() / 3).sum();
                println!("  {} on {}: {} vertices, {triangles} triangles", part.mesh.name, weapon.hierarchy.path(weapon.hierarchy.nodes[part.node].parent.unwrap_or(0)), part.mesh.positions.len());
            }
            let started = std::time::Instant::now();
            // As read, before they are compressed to be drawn with: pixels.
            let read: Vec<_> = custom.materials.iter().map(trp_data::custom::pictures).collect();
            println!("its pictures were read in {:.2} s", started.elapsed().as_secs_f32());
            for (index, (material, pictures)) in custom.materials.iter().zip(read).enumerate() {
                let pictures = pictures?;
                println!("  material {}", material.name);
                // What the parts drawn with it see of its colour: the mean
                // of the picture where their vertices lie on it, and how
                // far over it they reach.
                if let Some(colour) = &pictures.colour {
                    let (mut sums, mut count, mut low, mut high) = ([0u64; 3], 0u64, [f32::MAX; 2], [f32::MIN; 2]);
                    for part in custom.parts.iter().filter(|part| part.materials.contains(&index)) {
                        for uv in &part.mesh.uv0 {
                            (0..2).for_each(|k| (low[k], high[k]) = (low[k].min(uv[k]), high[k].max(uv[k])));
                            let x = ((uv[0].rem_euclid(1.0)) * colour.width as f32) as usize % colour.width as usize;
                            let y = ((uv[1].rem_euclid(1.0)) * colour.height as f32) as usize % colour.height as usize;
                            let at = (y * colour.width as usize + x) * colour.channels;
                            (0..3).for_each(|k| sums[k] += colour.data[at + k] as u64);
                            count += 1;
                        }
                    }
                    let mean = sums.map(|sum| sum / count.max(1));
                    println!("    at its parts' vertices the colour is {mean:?} on average; they lie from {low:.3?} to {high:.3?} on the picture");
                }
                for (name, picture) in [("colour", &pictures.colour), ("normal", &pictures.normal), ("surface (occlusion, roughness, metal)", &pictures.surface)] {
                    let Some(picture) = picture else {
                        println!("    no {name}");
                        continue;
                    };
                    // The mean of each channel over the largest size.
                    let first = picture.width as usize * picture.height as usize * picture.channels;
                    let mut sums = vec![0u64; picture.channels];
                    for pixel in picture.data[..first].chunks_exact(picture.channels) {
                        pixel.iter().zip(&mut sums).for_each(|(value, sum)| *sum += *value as u64);
                    }
                    let means: Vec<String> = sums.iter().map(|sum| format!("{:.0}", *sum as f64 / (picture.width * picture.height) as f64)).collect();
                    println!("    {name}: {} x {}, {} sizes, {} bytes, means {}", picture.width, picture.height, picture.mips, picture.data.len(), means.join(" "));
                }
            }
        }
        // A binary FBX file, which is no part of the install: its models
        // with where each is and how large, and each mesh's connected
        // pieces. `fbx <file> records` prints the file's records instead.
        (Some("fbx"), Some(file)) => {
            let data = std::fs::read(file)?;
            if arg(2) == Some("records") {
                fn show(record: &trp_data::fbx::Record, indent: usize, depth: usize) {
                    let values: Vec<String> = record
                        .props
                        .iter()
                        .map(|prop| match prop {
                            trp_data::fbx::Prop::Int(v) => v.to_string(),
                            trp_data::fbx::Prop::Float(v) => v.to_string(),
                            trp_data::fbx::Prop::Text(v) => format!("{:?}", v.replace('\0', "|").replace('\u{1}', "")),
                            trp_data::fbx::Prop::Raw(v) => format!("<{} bytes>", v.len()),
                            trp_data::fbx::Prop::Ints(v) => format!("<{} ints>", v.len()),
                            trp_data::fbx::Prop::Floats(v) => format!("<{} floats>", v.len()),
                        })
                        .collect();
                    println!("{}{} {}", "  ".repeat(indent), record.name, values.join(" "));
                    if depth > 0 {
                        record.children.iter().for_each(|child| show(child, indent + 1, depth - 1));
                    }
                }
                let depth = arg(3).and_then(|d| d.parse().ok()).unwrap_or(2);
                show(&trp_data::fbx::parse(&data)?, 0, depth);
                return Ok(());
            }
            let scene = trp_data::fbx::read(&data)?;
            // `fbx <file> slice <model> <axis> <from> <to> [piece]`: what
            // of a model lies in a slab across one axis (x, y or z), as
            // the least and most of its corners there: for measuring a
            // feature of it.
            if arg(2) == Some("slice") {
                let (name, axis) = (arg(3).unwrap_or(""), arg(4).and_then(|a| "xyz".find(a)).unwrap_or(0));
                let bound = |i: usize, default: f32| arg(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(default);
                let (from, to) = (bound(5, f32::MIN), bound(6, f32::MAX));
                let piece: Option<u32> = arg(7).and_then(|p| p.parse().ok());
                for model in scene.models.iter().filter(|model| model.name == name) {
                    let Some(mesh) = &model.mesh else { continue };
                    let (mut low, mut high, mut count) = ([f32::MAX; 3], [f32::MIN; 3], 0);
                    for (corner, p) in mesh.positions.iter().enumerate() {
                        if p[axis] < from || p[axis] > to || piece.is_some_and(|piece| mesh.piece[corner / 3] != piece) {
                            continue;
                        }
                        count += 1;
                        (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
                    }
                    println!("{name}: {count} corners there, from {low:.2?} to {high:.2?}");
                    // With `points` as the last word: each of them, once.
                    if args.last().map(String::as_str) == Some("points") {
                        let mut points: Vec<[i32; 3]> = mesh
                            .positions
                            .iter()
                            .enumerate()
                            .filter(|(corner, p)| p[axis] >= from && p[axis] <= to && piece.is_none_or(|piece| mesh.piece[corner / 3] == piece))
                            .map(|(_, p)| p.map(|v| (v * 100.0).round() as i32))
                            .collect();
                        points.sort();
                        points.dedup();
                        for p in points {
                            println!("  {:7.2} {:7.2} {:7.2}", p[0] as f32 / 100.0, p[1] as f32 / 100.0, p[2] as f32 / 100.0);
                        }
                    }
                }
                return Ok(());
            }
            // `fbx <file> view <out.png> [pixels a centimetre]`: the model
            // from its left side and from above, each model in a colour of
            // its own, over a grid of the file's own centimetres with the
            // origin where the file has it. A file whose barrel runs along
            // -z with y up, as this project's first one does.
            if arg(2) == Some("view") {
                let out = arg(3).unwrap_or("local/shots/fbx_view.png");
                let scale = arg(4).and_then(|s| s.parse().ok()).unwrap_or(12.0);
                let unit = scene.unit as f32;
                let mut triangles: Vec<Drawn> = Vec::new();
                for (index, model) in scene.models.iter().enumerate() {
                    let Some(mesh) = &model.mesh else { continue };
                    println!("{:?} is {}", PALETTE[index % PALETTE.len()], model.name);
                    for corners in mesh.positions.chunks_exact(3) {
                        let placed = [0, 1, 2].map(|k| [-corners[k][2] * unit, corners[k][1] * unit, corners[k][0] * unit]);
                        triangles.push((placed, PALETTE[index % PALETTE.len()], 1.0));
                    }
                }
                return trp_data::sketch::views(&triangles, &[], &[], out, scale, window_of(&args, 5));
            }
            println!("{} cm a unit; up {:?}, front {:?}, third {:?}", scene.unit, scene.up, scene.front, scene.coord);
            let fewest: usize = arg(2).and_then(|n| n.parse().ok()).unwrap_or(200);
            for model in &scene.models {
                let parent = model.parent.map_or("-", |p| scene.models[p].name.as_str());
                let at = [model.world[0][3], model.world[1][3], model.world[2][3]];
                println!("{}  (under {parent})  at {:.3?}  materials {:?}", model.name, at, model.materials);
                let Some(mesh) = &model.mesh else { continue };
                let bounds = |wanted: &dyn Fn(usize) -> bool| {
                    let (mut low, mut high, mut count) = ([f32::MAX; 3], [f32::MIN; 3], 0usize);
                    for triangle in (0..mesh.piece.len()).filter(|&t| wanted(t)) {
                        count += 1;
                        for p in &mesh.positions[triangle * 3..triangle * 3 + 3] {
                            (0..3).for_each(|k| (low[k], high[k]) = (low[k].min(p[k]), high[k].max(p[k])));
                        }
                    }
                    (low, high, count)
                };
                let (low, high, count) = bounds(&|_| true);
                println!("  {count} triangles in {} pieces, from {low:.2?} to {high:.2?}", mesh.pieces);
                for piece in 0..mesh.pieces {
                    let (low, high, count) = bounds(&|t| mesh.piece[t] == piece);
                    if count < fewest {
                        continue;
                    }
                    let first = mesh.piece.iter().position(|&p| p == piece).unwrap_or(0);
                    let material = model.materials.get(mesh.materials[first] as usize).map_or("?", String::as_str);
                    println!("    piece {piece:4}: {count:6} triangles  {material:12} from {low:.2?} to {high:.2?}");
                }
            }
        }
        _ => bail!("usage: probe find|info|names|dump|tree ..."),
    }
    Ok(())
}

fn describe_transition(t: &trp_data::states::Transition) -> String {
    let conditions: Vec<String> = t
        .conditions
        .iter()
        .map(|c| match c.mode {
            1 => c.parameter.clone(),
            2 => format!("!{}", c.parameter),
            3 => format!("{} > {}", c.parameter, c.threshold),
            4 => format!("{} < {}", c.parameter, c.threshold),
            6 => format!("{} == {}", c.parameter, c.threshold),
            7 => format!("{} != {}", c.parameter, c.threshold),
            other => format!("{} ?{other} {}", c.parameter, c.threshold),
        })
        .collect();
    let interruption = match t.interruption {
        0 => String::new(),
        other => format!("  interruptible({other}{})", if t.ordered_interruption { " ordered" } else { "" }),
    };
    format!(
        "{}{}  blend {}{}  from {}{}{}{interruption}",
        t.to.as_deref().unwrap_or(if t.to_selector.is_some() { "" } else { "(exit)" }),
        t.to_selector.map_or(String::new(), |s| format!("(point {s})")),
        t.duration,
        if t.fixed_duration { " s" } else { " of the state" },
        t.offset,
        t.exit_time.map_or(String::new(), |e| format!("  after {e}")),
        if conditions.is_empty() { String::new() } else { format!("  if {}", conditions.join(" & ")) }
    )
}

/// Prints a JSON value down to `depth` levels: below that, an object or a
/// list is only counted.
fn print_json(value: &serde_json::Value, depth: usize, indent: usize) {
    let pad = "  ".repeat(indent);
    let short = |value: &serde_json::Value| match value {
        serde_json::Value::Object(map) => format!("{{{} entries}}", map.len()),
        serde_json::Value::Array(list) => format!("[{} items]", list.len()),
        other => {
            let text = other.to_string();
            if text.len() > 160 { format!("{}...", &text[..text.char_indices().nth(160).map_or(text.len(), |(i, _)| i)]) } else { text }
        }
    };
    match value {
        serde_json::Value::Object(map) if depth > 0 => {
            for (name, value) in map {
                match value {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) if depth > 1 => {
                        println!("{pad}{name}:");
                        print_json(value, depth - 1, indent + 1);
                    }
                    _ => println!("{pad}{name}: {}", short(value)),
                }
            }
        }
        serde_json::Value::Array(list) if depth > 0 => {
            for (index, value) in list.iter().enumerate() {
                match value {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) if depth > 1 => {
                        println!("{pad}[{index}]");
                        print_json(value, depth - 1, indent + 1);
                    }
                    _ => println!("{pad}[{index}] {}", short(value)),
                }
            }
        }
        other => println!("{pad}{}", short(other)),
    }
}

fn class_label(id: i32) -> String {
    match class::name(id) {
        "" => format!("#{id}"),
        name => name.to_owned(),
    }
}

fn print_tree(assets: &Assets, file: usize) -> Result<()> {
    for transform in assets.objects_of(file, class::TRANSFORM) {
        let t = assets.read(transform)?;
        let is_root = t.req("m_Father")?.as_pptr().is_some_and(|p| p.is_null());
        if is_root {
            print_node(assets, transform, 0)?;
        }
    }
    Ok(())
}

fn print_node(assets: &Assets, transform: ObjRef, depth: usize) -> Result<()> {
    let t = assets.read(transform)?;
    let go_ptr = t.req("m_GameObject")?.as_pptr().unwrap();
    let mut name = String::from("?");
    let mut components = Vec::new();
    if let Some(go_ref) = assets.resolve_loaded(transform.file, go_ptr) {
        let go = assets.read(go_ref)?;
        name = go.get("m_Name").and_then(|n| n.as_str()).unwrap_or("").to_owned();
        for c in go.req("m_Component")?.as_array().unwrap_or(&[]) {
            let Some(ptr) = c.get("component").and_then(|p| p.as_pptr()) else { continue };
            let Some(obj) = assets.resolve_loaded(go_ref.file, ptr) else { continue };
            match assets.class_id(obj) {
                Some(class::TRANSFORM) | None => {}
                Some(class::MONO_BEHAVIOUR) => components.push(script_name(assets, obj)),
                Some(id) => components.push(class_label(id)),
            }
        }
    }
    let pos = t.req("m_LocalPosition")?.as_vec3().unwrap_or_default();
    let rot = t.req("m_LocalRotation")?.as_vec4().unwrap_or_default();
    let scale = t.req("m_LocalScale")?.as_vec3().unwrap_or_default();
    let mut extra = String::new();
    if rot[3].abs() < 0.9999 {
        extra += &format!(" rot({:.3} {:.3} {:.3} {:.3})", rot[0], rot[1], rot[2], rot[3]);
    }
    if scale.iter().any(|s| (s - 1.0).abs() > 1e-4) {
        extra += &format!(" scale({:.3} {:.3} {:.3})", scale[0], scale[1], scale[2]);
    }
    println!(
        "{}{name}  ({:.3} {:.3} {:.3}){extra}  {}",
        "  ".repeat(depth),
        pos[0],
        pos[1],
        pos[2],
        components.join(", ")
    );
    for child in t.req("m_Children")?.as_array().unwrap_or(&[]) {
        if let Some(obj) = child.as_pptr().and_then(|p| assets.resolve_loaded(transform.file, p)) {
            print_node(assets, obj, depth + 1)?;
        }
    }
    Ok(())
}

/// Class name of the script behind a MonoBehaviour. Only the header fields
/// are needed, so an unreadable body does not matter.
fn script_name(assets: &Assets, behaviour: ObjRef) -> String {
    let script = assets
        .read(behaviour)
        .ok()
        .and_then(|v| v.get("m_Script").and_then(|s| s.as_pptr()))
        .and_then(|p| assets.resolve_loaded(behaviour.file, p))
        .and_then(|obj| assets.read(obj).ok())
        .and_then(|v| v.get("m_ClassName").and_then(|n| n.as_str()).map(str::to_owned));
    format!("[{}]", script.as_deref().unwrap_or("script"))
}

/// A window for `sketch::views` from four words of the command line, if
/// they are there: centimetres ahead, behind, above and below.
fn window_of(args: &[String], first: usize) -> Option<[f32; 4]> {
    let number = |i: usize| args.get(first + i)?.parse::<f32>().ok();
    Some([number(0)?, number(1)?, number(2)?, number(3)?])
}

/// Colours to tell parts apart by.
const PALETTE: [[u8; 3]; 8] = [[120, 130, 145], [200, 120, 60], [70, 150, 90], [170, 80, 160], [60, 130, 200], [190, 170, 60], [200, 70, 70], [90, 90, 90]];
