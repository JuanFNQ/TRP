//! The inventory's panels as the app reads them: health, scene panels, battle, quick, stash, tabs, item views.

use anyhow::Result;
use trp_data::unity::Assets;

pub fn healthpanel(assets: &mut Assets, width: Option<&str>) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let health = trp_data::interface::screen::HealthPanel::load(&assets.install, &gear.file)?;
    let screen = [width.and_then(|w| w.parse().ok()).unwrap_or(1920.0), 1080.0];
    let left = health.left_panel(screen);
    println!("{}: left panel {left:?}, the equipment panel at {:?}, spacing {}", gear.file, health.gear_in(left), health.spacing);
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
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
    Ok(())
}

// Any object of a scene and all under it, as the app reads a panel
// (`scenepanel level44 "Left Panel/Gear Panel" 494 650`): each
// object's rectangle in the top's of that size (its own size where
// none is given), whether it is on, its scripts, its picture and
// its text.
pub fn scenepanel(assets: &mut Assets, args: &[String], file: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let scene = trp_data::interface::screen::Scene::load(&assets.install, file, true)?;
    let end = arg(2).unwrap_or("");
    let top = (0..scene.nodes.len()).find(|&index| scene.path(index).ends_with(end)).ok_or_else(|| anyhow::anyhow!("no object's path ends with {end:?}"))?;
    let panel = trp_data::interface::screen::ScenePanel::of(&scene, top);
    let own = panel.nodes[0].layout.size_delta;
    let size = [arg(3).and_then(|w| w.parse().ok()).unwrap_or(own[0]), arg(4).and_then(|h| h.parse().ok()).unwrap_or(own[1])];
    println!("{}: {size:?}", scene.path(top));
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
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
            if image.kind == 3 {
                line += &format!(" fill method {} origin {} amount {} clockwise {}", image.fill_method, image.fill_origin, image.fill_amount, image.fill_clockwise);
            }
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
            line += &format!("  text {:?} size {} colour {:?} align {}/{} style {} font {:?} spacing {} kerning {} margin {:?} wraps {} overflow {}", text.text, text.size, text.color, text.horizontal, text.vertical, text.style, text.font, text.spacing, text.kerning, text.margin, text.wraps, text.overflow);
        }
        // `PROBE_LAYOUT=1`: its own layout as kept.
        if std::env::var_os("PROBE_LAYOUT").is_some() {
            let l = &node.layout;
            line += &format!("\n      anchors {:?}-{:?} at {:?} size {:?} pivot {:?} scale {:?}", l.anchor_min, l.anchor_max, l.offset, l.size_delta, l.pivot, l.scale);
        }
        println!("{line}");
    }
    Ok(())
}

// A prefab of the build's own Resources by its path
// (`prefab prefabs/ugui/layouts/slot_layout 125 125`), as
// `scenepanel` prints a part of a scene.
// The inventory's quick-use panel as the app reads it, its views on
// as its script shows them: each view's binding, what it names and
// where it lies on a screen 1920 by 1080.
pub fn battle(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let views = trp_data::interface::screen::BattleViews::load(&assets.install, &gear.file)?;
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
        println!("  scene {:?}, at 0 {:?}, at 1 {:?}, at the scene's value {:?}", slider.handle.map(|handle| views.stance.rect(handle, size)), slider.handle_at(&views.stance, size, 0.0), slider.handle_at(&views.stance, size, 1.0), slider.handle_at(&views.stance, size, trp_data::interface::screen::SliderParts::share(slider.value, slider.min, slider.max)));
    }
    println!("fills {} {} sprites {:?}", stance(views.legs_fill), stance(views.arms_fill), views.fill_sprites);
    println!("ground {}", health(views.background));
    for (part, image) in &views.parts {
        println!("part {part}: {}", health(Some(*image)));
    }
    println!("effects {} grid {:?} icon {} count {}", health(views.effects), views.effects_grid, health(views.effect_template), health(views.stack_count));
    match &views.action {
        Some(action) => {
            let at = |index: usize| action.panel.nodes[index].path.clone();
            println!("action panel: pointer {}, line {} / {}, list {}, name {} / {}, error {} / {}", at(action.cursor), at(action.line_ground), at(action.line_text), at(action.list_ground), at(action.name_ground), at(action.name_text), at(action.error_ground), at(action.error_text));
            println!("  sprites: hover {:?}, unavailable {:?}, sense {:?}; aiming shift {:?}", action.hover_sprite, action.unavailable_sprite, action.sense_sprite, action.aiming_shift);
            for node in [action.line_text, action.name_text, action.error_text] {
                println!("  {}: {:?}", at(node), action.panel.nodes[node].text);
            }
            for node in [action.line_ground, action.list_ground, action.name_ground, action.error_ground] {
                let parent = action.panel.nodes[node].parent.unwrap_or(0);
                println!("  {}: image {:?}; its parent {} group {:?} element {:?}", at(node), action.panel.nodes[node].image.map(|image| image.color), at(parent), action.panel.nodes[parent].group, action.panel.nodes[parent].element);
            }
        }
        None => println!("action panel: none ({})", views.action_missing.as_deref().unwrap_or("-")),
    }
    match &views.mount {
        Some(mount) => {
            let sides: Vec<String> = mount.directions.iter().map(|(object, side)| format!("{} {side}", stance(Some(*object)))).collect();
            println!("mount icon {} unavailable {} directions [{}] highlight {} s", stance(Some(mount.image)), stance(Some(mount.unavailable)), sides.join(", "), mount.highlight);
        }
        None => println!("mount icon: none"),
    }
    let quick = trp_data::interface::screen::QuickViews::load_battle(&assets.install, &gear.file)?;
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
    Ok(())
}

pub fn quick(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let mut quick = trp_data::interface::screen::QuickViews::load(&assets.install, &gear.file)?;
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
    // Each caption's text as each bar has it (the inventory's, the
    // battle screen's): its rectangle, size, wrapping and overflow.
    let battle = trp_data::interface::screen::QuickViews::load_battle(&assets.install, &gear.file)?;
    for (name, views) in [("inventory", &quick), ("battle", &battle)] {
        let room = views.quick_in([1920.0, 1080.0]);
        for slot in views.slots.iter().take(1) {
            let Some(caption) = slot.caption else { continue };
            let Some(text) = &views.panel.nodes[caption].text else { continue };
            println!("{name} caption {}: {:?} size {} auto {:?} style {} wraps {} overflow {} margin {:?} horizontal {} vertical {}", views.panel.nodes[caption].path, views.panel.rect(caption, [room[2], room[3]]), text.size, text.auto_size, text.style, text.wraps, text.overflow, text.margin, text.horizontal, text.vertical);
        }
    }
    Ok(())
}

// The inventory's panel for a container opened, as the app reads it:
// what its script names, its filters' tabs and its scrolling.
pub fn stash(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let stash = trp_data::interface::screen::StashViews::load(&assets.install, &gear.file)?;
    let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| stash.panel.nodes[index].path.clone());
    println!("simple {}, header {}, grid name {}, filters {}, money {}, name panel {}, name {}", at(Some(stash.simple)), at(stash.header), at(stash.grid_name), at(stash.filters), at(stash.money), at(stash.name_panel), at(stash.name));
    println!("lobby only {:?}", stash.lobby_only.iter().map(|&index| at(Some(index))).collect::<Vec<_>>());
    println!("sort {}, sorting table {}, search {}, scroll {}, external {}, pin {}, lock {}, priority {}, grids {}", at(stash.sort), at(stash.sorting_table), at(stash.search), at(stash.scroll), at(stash.external), at(stash.pin), at(stash.lock), at(stash.priority), at(stash.grids));
    println!("scrolling {:?}", stash.scrolling);
    if let Some((tints, tinted)) = stash.search_tints {
        println!("search button's tints {tints:?} on {}", at(tinted));
    }
    if let Some(complex) = stash.complex {
        println!("complex {}, header {}, grid name {}, content {}, scroll {}, name panel {}, name {}", at(Some(complex.panel)), at(complex.header), at(complex.grid_name), at(complex.content), at(complex.scroll), at(complex.name_panel), at(complex.name));
        println!("  its scrolling {:?}", complex.scrolling);
    }
    for tab in &stash.filter_tabs {
        println!("  filter {}: normal {}, selected {}, target {}, icon {}", at(Some(tab.object)), at(tab.normal), at(tab.selected), at(tab.target), at(tab.icon));
    }
    let size = stash.panel.nodes[0].layout.size_delta;
    for (index, node) in stash.panel.nodes.iter().enumerate().filter(|(_, node)| node.path.contains("Filter Panel/") && node.path.matches('/').count() == 3) {
        println!("  {} {:?}", node.path, stash.panel.rect(index, size));
    }
    Ok(())
}

// The edit build screen's preview as its scene has it: the camera's
// figures, the preview's height and its lights.
pub fn editbuild(assets: &mut Assets) -> Result<()> {
    let screen = trp_data::interface::edit_build::EditBuildScene::load(&assets.install)?;
    println!("camera: field of view {} degrees, planes {} to {} m; the preview at a height of {} m; {} objects", screen.fov, screen.near, screen.far, screen.height, screen.panel.nodes.len());
    for light in &screen.lights {
        println!("  {:16} kind {} on {} colour {:?} intensity {} range {} spot {} at {:?} turned {:?} layers {:#x}", light.name, light.kind, light.on, light.color, light.intensity, light.range, light.spot_angle, light.position, light.rotation, light.culling_mask);
    }
    Ok(())
}

pub fn tabs(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let bar = trp_data::interface::screen::TabBar::load(&assets.install, &gear.file)?;
    let at = |index: Option<usize>| index.map_or("-".to_owned(), |index| bar.panel.nodes[index].path.clone());
    println!("row {}, gap {}", at(bar.row), bar.gap);
    let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
    let words = trp_data::character::inventory::texts(&assets.install, "en", &keys).unwrap_or_default();
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
                fonts.insert(file.clone(), trp_data::interface::screen::build_fonts(&assets.install, &file).unwrap_or_default());
            }
            let asset = trp_data::interface::screen::script_name(&assets.install, &file, text.font.1).unwrap_or_default();
            println!("    font asset {asset:?}: {file} {}", text.font.1);
            if let Ok(face) = trp_data::interface::screen::TmpFont::read(&assets.install, &file, text.font.1) {
                println!("      face {:?} {:?}, bold typeface {:?}", face.family, face.style, face.bold);
                if let Some((bold_file, bold_id)) = &face.bold {
                    let bold_name = trp_data::interface::screen::script_name(&assets.install, bold_file, *bold_id).unwrap_or_default();
                    match trp_data::interface::screen::TmpFont::read(&assets.install, bold_file, *bold_id) {
                        Ok(bold) => println!("      bold {bold_name:?}: face {:?} {:?}", bold.family, bold.style),
                        Err(e) => println!("      bold {bold_name:?}: {e:#}"),
                    }
                }
            }
            let font = trp_data::interface::screen::font_for(&asset, &fonts[&file]);
            let width = font.and_then(|(_, bytes)| trp_data::interface::screen::text_width(bytes, &said, text.size, text.kerning));
            let shown_said = if text.style & 16 != 0 { said.to_uppercase() } else { said.clone() };
            let tmp = trp_data::interface::screen::TmpFont::read(&assets.install, &file, text.font.1).ok();
            let bold = text.style & 1 != 0;
            let tmp_width = tmp.as_ref().map(|regular| match regular.bold.clone().filter(|_| bold).and_then(|(file, id)| trp_data::interface::screen::TmpFont::read(&assets.install, &file, id).ok()) {
                Some(alternative) => alternative.width(&shown_said, text.size, text.spacing, false),
                None => regular.width(&shown_said, text.size, text.spacing, bold),
            } + text.margin[0] + text.margin[2]);
            println!("    as the game lays it out: {shown_said:?} {tmp_width:?}");
            let shown = if text.style & 16 != 0 { text.text.to_uppercase() } else { text.text.clone() };
            let saved = font.and_then(|(_, bytes)| trp_data::interface::screen::text_width(bytes, &shown, text.size, text.kerning));
            println!("    label {} font {asset:?} ({:?}) size {} style {} spacing {}: {said:?} {width:?}; the scene's {:?} {saved:?}, laid {:?}", at(Some(label)), font.map(|(name, _)| name), text.size, text.style, text.spacing, text.text, bar.panel.nodes[label].layout.size_delta);
        }
    }
    Ok(())
}

// The game's views of a thing (`itemviews [width] [height]`, units:
// a grid's view of one cell unless told): each object of the slot's
// and the grid's layout placed in that size, and each text's
// alignment and margins (left, top, right, bottom).
// Which of an item view's loaders its script points to where
// (`itemviews loaders`): each pointer in the root's scripts that
// leads to an object named for a loader, by its place in the
// fields (`ItemView._drainLoader` is declared before `_iconLoader`).
pub fn itemviews_loaders(assets: &mut Assets) -> Result<()> {
    let views = trp_data::interface::screen::ItemViews::load(&assets.install)?;
    for (name, panel) in [("slot", &views.slot), ("grid", &views.grid)] {
        let Some(panel) = panel else { continue };
        for (class, fields) in &panel.nodes[0].scripts {
            for at in (0..fields.len().saturating_sub(11)).step_by(4) {
                if let Some(index) = panel.pointer_at(fields, at).filter(|&index| panel.nodes[index].name.contains("Loader") || at < 100) {
                    let sprite = panel.nodes[index].image.and_then(|image| image.sprite).map(|(file, id)| (panel.file_of(file).unwrap_or_else(|| panel.file.clone()), id));
                    println!("{name} {class} at {at}: {:?} sprite {sprite:?}", panel.nodes[index].path);
                }
            }
        }
    }
    Ok(())
}

pub fn itemviews(assets: &mut Assets, args: &[String]) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let views = trp_data::interface::screen::ItemViews::load(&assets.install)?;
    let size = [arg(1).and_then(|w| w.parse().ok()).unwrap_or(63.0), arg(2).and_then(|h| h.parse().ok()).unwrap_or(63.0)];
    for (name, panel) in [("slot", &views.slot), ("grid", &views.grid)] {
        let Some(panel) = panel else { continue };
        println!("== {name}");
        for (index, node) in panel.nodes.iter().enumerate() {
            let [x, y, w, h] = panel.placed(index, size).rect;
            let text = node.text.as_ref().map(|t| format!(" text {:?} size {} align {}/{} margin {:?}", t.text, t.size, t.horizontal, t.vertical, t.margin)).unwrap_or_default();
            println!("  {:50} {} ({x:.1}, {y:.1}) {w:.1} x {h:.1}{text}", node.path, if node.active { "on " } else { "off" });
            // The tag's and the caption's own placing, and the
            // tag's picture (`tags` as the third word).
            let wanted = match arg(3) {
                Some("tags") => node.path.contains("Tag") || node.path.contains("Caption"),
                Some("wish") => node.path.contains("Wishlist") || node.path.contains("BottomLeft") || node.path.contains("ArmorClass") || node.path.contains("ModIcon"),
                // The bottom right row (`buff`): the repair buff's
                // icon and the value beside it.
                Some("buff") => node.path.contains("BottomRight") || node.path.ends_with("BottomLayoutGroup"),
                _ => false,
            };
            if wanted {
                let l = &node.layout;
                let image = node.image.as_ref().map(|image| format!(" image kind {} color {:?} sprite {:?}", image.kind, image.color, image.sprite)).unwrap_or_default();
                println!("      anchors {:?}-{:?} at {:?} size {:?} pivot {:?} element {:?} group {:?}{image}", l.anchor_min, l.anchor_max, l.offset, l.size_delta, l.pivot, node.element, node.group);
            }
        }
    }
    Ok(())
}
