//! The game's windows and small screens: info, tag, context menu, notice, repair, split, skills, mastering.

use anyhow::Result;
use trp_data::unity::Assets;

use crate::printing::print_prop_lods;

pub fn infowindow(assets: &mut Assets, width: Option<&str>) -> Result<()> {
    let window = trp_data::interface::screen::info_window(&assets.install)?;
    let screen = [width.and_then(|w| w.parse().ok()).unwrap_or(1920.0), 1080.0];
    println!("{} ({} objects)", window.file, window.nodes.len());
    for (index, node) in window.nodes.iter().enumerate() {
        let [x, y, w, h] = window.placed_in(index, screen).rect;
        let mut line = format!("{:70} {}  ({x:.1}, {y:.1}) {w:.1} x {h:.1}", node.path, if window.shown(index) { "on " } else { "off" });
        if let Some(image) = &node.image {
            line += &format!("  image {:?} kind {} sprite {:?}", image.color, image.kind, image.sprite);
        }
        if let Some(text) = &node.text {
            line += &format!("  text {:?} size {} {:?} colour {:?} style {} align {}/{} wraps {} overflow {}", text.text, text.size, text.auto_size, text.color, text.style, text.horizontal, text.vertical, text.wraps, text.overflow);
        }
        let scripts: Vec<&str> = node.scripts.iter().map(|(name, _)| name.as_str()).collect();
        if !scripts.is_empty() {
            line += &format!("  [{}]", scripts.join(", "));
        }
        println!("{line}");
    }
    // A slot of its parts' panel (`ModSlotView`), at 64 square.
    if let Some(slot) = trp_data::interface::screen::info_rows(&assets.install, &window)?.mod_slot {
        println!("a part's slot: {} ({} objects)", slot.file, slot.nodes.len());
        for (index, node) in slot.nodes.iter().enumerate() {
            let [x, y, w, h] = slot.rect(index, [64.0, 64.0]);
            let mut line = format!("  {:60} {} ({x:.1}, {y:.1}) {w:.1} x {h:.1}", node.path, if node.active { "on " } else { "off" });
            if let Some(text) = &node.text {
                line += &format!("  text {:?} size {} colour {:?} style {} align {}/{} wraps {} overflow {} margin {:?}", text.text, text.size, text.color, text.style, text.horizontal, text.vertical, text.wraps, text.overflow, text.margin);
            }
            if let Some(image) = &node.image {
                line += &format!("  image {:?} kind {}", image.color, image.kind);
                // Its sprite by name, its cut and its border.
                if let Some((outside, id)) = image.sprite {
                    let name = if outside == 0 { slot.file.clone() } else { slot.externals.get(outside as usize - 1).cloned().unwrap_or_default() };
                    match trp_data::interface::screen::ScenePictures::new(&assets.install).and_then(|mut pictures| pictures.sprite(&name, id)) {
                        Ok(cut) => line += &format!(" sprite {:?} {:?} border {:?}", cut.name, cut.rect, cut.border),
                        Err(e) => line += &format!(" sprite {name} {id}: {e:#}"),
                    }
                }
            }
            let scripts: Vec<&str> = node.scripts.iter().map(|(name, _)| name.as_str()).collect();
            line += &format!("  [{}]", scripts.join(", "));
            println!("{line}");
        }
    }
    Ok(())
}

pub fn gridwindow(assets: &mut Assets) -> Result<()> {
    let look = trp_data::interface::screen::grid_window(&assets.install)?;
    let at = |index: usize| look.panel.nodes[index].path.clone();
    println!("{}: {} objects; caption panel {}, icon {}, caption {}, close {}, priority {:?}", look.panel.file, look.panel.nodes.len(), at(look.caption_panel), at(look.icon), at(look.caption), at(look.close), look.priority.map(at));
    println!("icon colours {:?}; header colours {:?}", look.icon_colors, look.header_colors);
    for (index, node) in look.panel.nodes.iter().enumerate() {
        let classes: Vec<&str> = node.scripts.iter().map(|(class, _)| class.as_str()).collect();
        println!("  {index:>3} {}{} {:?} {:?}", node.path, if node.active { "" } else { " (off)" }, node.layout.size_delta, classes);
    }
    Ok(())
}

pub fn skillspage(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let page = trp_data::interface::screen::SkillsScreen::load(&assets.install, &gear.file)?;
    println!("{}: {} objects; a skill's view {} objects, a thumb {}", gear.file, page.panel.nodes.len(), page.detailed.nodes.len(), page.thumb.nodes.len());
    println!("list grid {:?}", page.list_grid);
    println!("thumb grid {:?}", page.thumb_grid);
    println!("fills {:?}, buff grounds {:?}", page.fill_sprites, page.buff_grounds);
    println!("line {:?} objects, tints {:?}, highlight {:?}, most seen {:?}", page.line.as_ref().map(|line| line.nodes.len()), page.line_tints, page.dropdown_highlight, page.dropdown_most_seen);
    println!("list scroll {:?}", page.list_scroll.map(|s| (s.sensitivity, s.bar.is_some())));
    println!("view scroll {:?}", page.view_scroll.map(|s| (s.sensitivity, s.bar.is_some(), s.content.map(|c| page.panel.nodes[c].path.clone()))));
    for (index, key, case) in &page.localized {
        println!("  localized {} {key:?} case {case}", page.panel.nodes[*index].path);
    }
    // What each script's pointers lead to, the last `count` of its
    // fields.
    let pointed = |panel: &trp_data::interface::screen::ScenePanel, node: usize, class: &str, count: usize| {
        let Some(fields) = panel.script(node, class) else { return println!("  {class}: none") };
        let start = fields.len().saturating_sub(count * 12);
        let to: Vec<String> = (0..count).map(|n| panel.pointer_at(fields, start + n * 12).map_or("-".to_owned(), |at| panel.nodes[at].path.clone())).collect();
        println!("  {class} at {}: {to:?}", panel.nodes[node].path);
    };
    pointed(&page.detailed, 0, "SkillPanel", 15);
    pointed(&page.detailed, 0, "SkillIcon", 7);
    pointed(&page.thumb, 0, "SkillIcon", 7);
    for (panel, name) in [(&page.detailed, "detailed"), (&page.thumb, "thumb")] {
        for (index, node) in panel.nodes.iter().enumerate() {
            if let Some(fields) = panel.script(index, "HoverTooltipArea") {
                let len = fields.get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().expect("four")) as usize);
                println!("  {name} {} HoverTooltipArea message {:?}", node.path, fields.get(4..4 + len).map(String::from_utf8_lossy));
            }
        }
    }
    // The views' tabs' (`Tab`: seven pointers first).
    for tab in ["BottomPanel/SkillsPanel/Options/Tabs/List Tab", "BottomPanel/SkillsPanel/Options/Tabs/Grid Tab", "Caption Bar/Tabs/Skills"] {
        let Some(node) = page.panel.find(tab) else { continue };
        let class = if page.panel.script(node, "Tab").is_some() { "Tab" } else { "TabFont" };
        let Some(fields) = page.panel.script(node, class) else { continue };
        let to: Vec<String> = (0..7).map(|n| page.panel.pointer_at(fields, n * 12).map_or("-".to_owned(), |at| page.panel.nodes[at].path.clone())).collect();
        println!("  {class} at {tab}: {to:?} ({} bytes)", fields.len());
    }
    let at = |path: &str| page.panel.find(path).map(|index| page.panel.placed_in(index, [1920.0, 1080.0]).rect);
    for path in ["", "Caption Bar", "TopPanel", "BottomPanel", "BottomPanel/SkillsPanel/Lower Part/Scrollview", "BottomPanel/SkillsPanel/Options/Sort"] {
        println!("  placed in 1920 x 1080: {path:?} {:?}", at(path));
    }
    let tooltip = trp_data::interface::screen::skill_tooltip(&assets.install)?;
    println!("tooltip {} objects, fills {:?}, buff line {:?} objects, grounds {:?}", tooltip.panel.nodes.len(), tooltip.fill_sprites, tooltip.buff_line.as_ref().map(|line| line.nodes.len()), tooltip.buff_grounds);
    let skills = trp_data::interface::screen::skill_icons(&assets.install)?;
    let buffs = trp_data::interface::screen::buff_icons(&assets.install)?;
    println!("{} skill icons: {:?}", skills.len(), skills.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>());
    println!("{} buff icons: {:?}", buffs.len(), buffs.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>());
    Ok(())
}

// The MASTERING page as the app reads it (`MasteringScreen`), the
// mastering tooltip, and the install's families.
pub fn masteringpage(assets: &mut Assets) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let page = trp_data::interface::screen::SkillsScreen::load(&assets.install, &gear.file)?;
    let mastering = trp_data::interface::screen::MasteringScreen::of(&page)?;
    println!("a weapon's view {} objects, a thumb {}", mastering.detailed.nodes.len(), mastering.thumb.nodes.len());
    println!("list grid {:?}", mastering.list_grid);
    println!("thumb grid {:?}", mastering.thumb_grid);
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    match &mastering.fill_sprite {
        Some((file, id)) => println!("fill {file} {id}: {:?}", pictures.sprite(file, *id).map(|cut| (cut.name, cut.rect))),
        None => println!("fill: none"),
    }
    println!("line {:?} objects, tints {:?}, highlight {:?}, most seen {:?}", mastering.line.as_ref().map(|line| line.nodes.len()), mastering.line_tints, mastering.dropdown_highlight, mastering.dropdown_most_seen);
    println!("list scroll {:?}", mastering.list_scroll.map(|s| (s.sensitivity, s.bar.is_some())));
    println!("view scroll {:?}", mastering.view_scroll.map(|s| (s.sensitivity, s.bar.is_some(), s.hides)));
    let search = page.panel.find(trp_data::interface::screen::MASTERING_SEARCH);
    println!("search field's scripts {:?}", search.map(|field| page.panel.nodes[field].scripts.iter().map(|(class, fields)| (class.as_str(), fields.len())).collect::<Vec<_>>()));
    match mastering.search {
        Some(field) => println!("search field: text {:?}; selection {:?}; caret {} wide, {} a second, own colour {:?}; selects all {}; limit {}", page.panel.nodes[field.text].path, field.selection, field.caret_width, field.blink_rate, field.caret_color, field.select_all, field.limit),
        None => println!("search field: not read"),
    }
    for (index, key, case) in page.localized.iter().filter(|(index, ..)| page.panel.nodes[*index].path.contains("MasteringPanel")) {
        println!("  localized {} {key:?} case {case}", page.panel.nodes[*index].path);
    }
    let tooltip = trp_data::interface::screen::mastering_tooltip(&assets.install)?;
    println!("tooltip {} objects in {}", tooltip.nodes.len(), tooltip.file);
    let db = trp_data::Database::load(&assets.install)?;
    let families = db.config.get("Mastering").and_then(|m| m.as_array()).map_or(0, |m| m.len());
    println!("{families} families");
    Ok(())
}

pub fn containers(assets: &mut Assets, layout: Option<&str>) -> Result<()> {
    let gear = trp_data::interface::screen::GearPanel::load(&assets.install)?;
    let views = trp_data::interface::screen::ContainerViews::load(&assets.install, &gear.file)?;
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
        println!("  slots/equipment/{slot}: {:?}", trp_data::interface::screen::resource_sprite(&assets.install, &format!("Slots/Equipment/{slot}")));
    }
    // A special slot's view's parts, as saved: on or off, and each
    // one's box from its template's corner (`SlotView`'s borders,
    // its fill and its back picture).
    if let Some(template) = (0..views.panel.nodes.len()).find(|&index| views.panel.nodes[index].path.ends_with("Special Slot Template")) {
        let [tx, ty, ..] = views.panel.rect(template, [64.0, 64.0]);
        for (index, node) in views.panel.nodes.iter().enumerate().filter(|(_, node)| node.path.contains("Special Slot Template")) {
            let [x, y, w, h] = views.panel.rect(index, [64.0, 64.0]);
            println!("  special slot: {} on {} at {:.2},{:.2} {:.2}x{:.2} image {:?}", node.path, node.active, x - tx, y - ty, w, h, node.image.map(|image| image.kind));
        }
    }
    if let Some(name) = layout {
        let (panel, grids) = trp_data::interface::screen::rig_layout(&assets.install, name)?;
        let size = panel.nodes[0].layout.size_delta;
        println!("  {name}: its own size {size:?}");
        for (n, view) in grids.iter().enumerate() {
            println!("  grid {n}: {} {:?}", panel.nodes[*view].path, panel.rect(*view, size));
        }
    }
    Ok(())
}

// Every script of a class on the player prefab: the object it is on,
// that object's parent and layer, and the colliders on it
// (`player-scripts BodyPartCollider`).
// `popper`: the hideout range's popper as `range_targets` reads it:
// its objects with their places and what they draw, and its sounds.
pub fn popper(assets: &mut Assets) -> Result<()> {
    let model = trp_data::level::range_targets::popper(&assets.install, true)?;
    for (index, node) in model.props.nodes.iter().enumerate() {
        println!("{index:2} {} under {:?} at {:?} turned {:?}, draws {}{}", node.name, node.parent, node.translation, node.rotation, node.drawn.len(), node.lod.map_or(String::new(), |lod| format!(", group {} levels {:#b}", lod.group, lod.levels)));
    }
    println!("plate {}, roller {}; {} meshes, {} materials, {} pictures; skipped {:?}", model.plate, model.roller, model.props.meshes.len(), model.props.materials.len(), model.props.pictures.len(), model.props.skipped);
    print_prop_lods(&model.props);
    for (name, clip) in trp_data::level::range_targets::popper_clips(&assets.install)? {
        println!("{name}: {} channels at {} Hz, {:.2} s", clip.channels, clip.frequency, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32);
    }
    Ok(())
}

// The game's icon camera as `icon::Studio::read` takes it, beside
// what was read by hand.
pub fn studio(assets: &mut Assets) -> Result<()> {
    let read = trp_data::interface::icon::Studio::read(&assets.install)?;
    println!("read: {read:#?}");
    println!("the same as read by hand: {}", read == trp_data::interface::icon::Studio::as_read());
    Ok(())
}

// An item's icon settings as its prefab has them (`icon::pose_in`):
// the object they are on, its renderers, and for a weapon the
// object its hands hold.
// The right click's menu as the install keeps it
// (`screen::context_menu`): each object of the button templates, its
// place and size, its layout element and its picture's flags.
// The game's window that edits a tag (`screen::tag_window`): each
// object, its place, size, layout element, picture and words.
pub fn tagwindow_placed(assets: &mut Assets) -> Result<()> {
    let panel = trp_data::interface::screen::tag_window(&assets.install)?;
    let [_, _, w, h] = panel.placed_in(0, [1920.0, 1080.0]).rect;
    println!("the window {w} x {h}");
    for (index, node) in panel.nodes.iter().enumerate() {
        let [x, y, rw, rh] = panel.rect(index, [w, h]);
        println!("[{index}] {} {:60} ({x:.1}, {y:.1}) {rw:.1} x {rh:.1}", if node.active { "on " } else { "off" }, node.path);
    }
    Ok(())
}

pub fn tagwindow(assets: &mut Assets) -> Result<()> {
    let panel = trp_data::interface::screen::tag_window(&assets.install)?;
    for (index, node) in panel.nodes.iter().enumerate() {
        let l = &node.layout;
        let image = node.image.as_ref().map(|image| format!(" image kind {} color {:?} sprite {:?}", image.kind, image.color, image.sprite)).unwrap_or_default();
        let text = node.text.as_ref().map(|text| format!(" text {:?} size {} color {:?} align {}/{} margin {:?} spacing {}", text.text, text.size, text.color, text.horizontal, text.vertical, text.margin, text.spacing)).unwrap_or_default();
        println!(
            "[{index}] {} {} anchors {:?}-{:?} at {:?} size {:?} pivot {:?} element {:?} group {:?} scripts {:?}{image}{text}",
            if node.active { "on " } else { "off" }, node.path, l.anchor_min, l.anchor_max, l.offset, l.size_delta, l.pivot, node.element, node.group, node.scripts.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>()
        );
    }
    // Its field as TextMeshPro keeps it (`screen::input_field`).
    match panel.find("Inner/Contents/TagInput").map(|input| trp_data::interface::screen::input_field(&panel, input, "ValidationInputField")) {
        Some(Ok(field)) => println!("field: text {:?}; selection {:?}; caret {} wide, {} a second, own colour {:?}; selects all {}; limit {}", panel.nodes[field.text].path, field.selection, field.caret_width, field.blink_rate, field.caret_color, field.select_all, field.limit),
        Some(Err(e)) => println!("field: not read: {e:#}"),
        None => println!("field: none"),
    }
    Ok(())
}

pub fn contextmenu(assets: &mut Assets) -> Result<()> {
    let panel = trp_data::interface::screen::context_menu(&assets.install)?;
    for (index, node) in panel.nodes.iter().enumerate() {
        if !node.path.contains("Template") {
            continue;
        }
        let l = &node.layout;
        let image = node.image.as_ref().map(|image| format!(" image kind {} keep {} sprite {:?}", image.kind, image.preserve_aspect, image.sprite)).unwrap_or_default();
        println!(
            "[{index}] {} anchors {:?}-{:?} at {:?} size {:?} pivot {:?} element {:?} group {:?}{image}",
            node.path, l.anchor_min, l.anchor_max, l.offset, l.size_delta, l.pivot, node.element, node.group
        );
    }
    Ok(())
}

// A build's sights as `trp_data::sight` reads them (`sights <template
// or build key>`): each part that is a sight, its item's figures,
// its prefab's modes; the sights that fold; every aim bone.
// The inspect window's preview as the game makes it
// (`trp_data::edit_build::InspectPreview`).
// The notifier's notice and the malfunction glow as the app reads
// them (`trp_data::screen::notice`, `malfunction_glow`).
pub fn notice(assets: &mut Assets) -> Result<()> {
    let notice = trp_data::interface::screen::notice(&assets.install)?;
    for (index, node) in notice.panel.nodes.iter().enumerate() {
        println!("{index} {:?}: {:?} text {:?} group {:?} element {:?}", node.path, node.layout, node.text.as_ref().map(|t| (t.font, t.size, t.style, t.horizontal, t.vertical, t.margin, t.color)), node.group, node.element);
    }
    println!("box {}, text {}, icon {}, padding {:?}, below {}, alert {:?}", notice.content, notice.text, notice.icon, notice.padding, notice.below, notice.alert);
    Ok(())
}

// The game scene's panel of a use or an examine as the app reads it
// (`trp_data::screen::using_panel`): what its script names, and
// each object with its layout, text and picture.
pub fn usingpanel(assets: &mut Assets) -> Result<()> {
    let using = trp_data::interface::screen::using_panel(&assets.install)?;
    println!("{}: caption {:?}, name {:?}, fill {:?}, default colour {:?}, examine colour {:?}", using.panel.file, using.caption, using.name, using.fill, using.default_color, using.examine_color);
    for (index, node) in using.panel.nodes.iter().enumerate() {
        println!("{index} {:?} {}: {:?} text {:?} image {:?} group {:?} element {:?}", node.path, node.active, node.layout, node.text.as_ref().map(|t| (&t.text, t.font, t.size, t.style, t.horizontal, t.vertical, t.margin, t.color, t.wraps)), node.image.map(|i| (i.color, i.kind, i.sprite)), node.group, node.element);
    }
    Ok(())
}

// The game's repair window's prefab (`trp_data::screen::repair_window`):
// every object, its place on 1920 by 1080, its picture, text and
// scripts.
pub fn repairwindow(assets: &mut Assets) -> Result<()> {
    let panel = trp_data::interface::screen::repair_window(&assets.install)?;
    println!("{} objects, from {}", panel.nodes.len(), panel.file);
    println!("figures {:?}", trp_data::interface::screen::repair_figures(&panel));
    for index in 0..panel.nodes.len() {
        if let Some((key, case)) = trp_data::interface::screen::localized(&panel, index) {
            println!("localized {index} {:?}: {key:?} case {case}", panel.nodes[index].path);
        }
    }
    for (index, node) in panel.nodes.iter().enumerate() {
        let size = std::env::var("PROBE_SIZE").ok().and_then(|s| s.split_once('x').and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?]))).unwrap_or([700.0, 800.0]);
        let placed = panel.placed(index, size).rect;
        let scripts: Vec<&str> = node.scripts.iter().map(|(name, _)| name.as_str()).collect();
        let image = node.image.map(|image| (image.sprite, image.color, image.kind));
        let text = node.text.as_ref().map(|t| (t.text.clone(), t.font, t.size, t.style, t.horizontal, t.vertical, t.color));
        println!("{index} {:?}{}: at {:?} size {:?} anchors {:?}-{:?} pivot {:?}; image {:?}; text {:?}; group {:?}; element {:?}; scripts {:?}", node.path, if node.active { "" } else { " (off)" }, placed, node.layout.size_delta, node.layout.anchor_min, node.layout.anchor_max, node.layout.pivot, image, text, node.group, node.element, scripts);
    }
    Ok(())
}

// The game's window that asks how many (`trp_data::screen::
// split_dialog`): every object, its place on 1920 by 1080, its
// picture, text, row and scripts, as `repairwindow` prints them.
// The window laid out as its script leaves it, on a screen of
// 1920 by 1080, its middle at the screen's: `splitdialog laid <step
// or int> <notches> <of them not available first> <share>`; the
// notches' and the handle's places.
pub fn splitdialog_laid(assets: &mut Assets, args: &[String]) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let look = trp_data::interface::screen::split_look(&assets.install)?;
    let int = arg(2) == Some("int");
    let count: usize = arg(3).and_then(|n| n.parse().ok()).unwrap_or(20);
    let dim: usize = arg(4).and_then(|n| n.parse().ok()).unwrap_or(0);
    let share: f32 = arg(5).and_then(|n| n.parse().ok()).unwrap_or(0.5);
    let notches: Vec<bool> = (0..count).map(|n| n >= dim).collect();
    let panel = look.laid_out(int, &notches, share, [960.0, 540.0]);
    for index in (0..panel.nodes.len()).filter(|&index| panel.nodes[index].path.contains("(Clone)") || panel.nodes[index].path.contains("Handle") || panel.nodes[index].path.contains("Final Notch/")) {
        let node = &panel.nodes[index];
        if node.path.contains(if int { "SplitAmountStep" } else { "SplitAmountInt" }) {
            continue;
        }
        println!("{index} {:?}: {:?}; colour {:?}", node.path, panel.placed(index, [1920.0, 1080.0]).rect, node.image.map(|image| image.color));
    }
    Ok(())
}

pub fn splitdialog(assets: &mut Assets) -> Result<()> {
    let panel = trp_data::interface::screen::split_dialog(&assets.install)?;
    match trp_data::interface::screen::split_look(&assets.install) {
        Ok(look) => {
            let name = |index: usize| look.panel.nodes[index].path.clone();
            println!("window {:?}, caption {:?}, title {:?}", name(look.window), name(look.caption), name(look.title));
            for (what, button) in [("close", look.close), ("accept", look.accept)] {
                println!("{what} {:?}: tints {:?} on {:?}", name(button.object), button.tints, button.tinted.map(name));
            }
            for (what, slider) in [("step", look.step), ("int", look.int)] {
                println!(
                    "{what} {:?}: tints {:?} on {:?}; handle {:?}; field {:?} text {:?}; notch {:?} not available {:?} into {:?}; selection {:?} caret {} wide, {} a second, own colour {:?}; selects all {}",
                    name(slider.object), slider.tints, slider.tinted.map(name), name(slider.handle), name(slider.input), name(slider.text), name(slider.notch), slider.not_available.map(name), name(slider.container), slider.selection, slider.caret_width, slider.blink_rate, slider.caret_color, slider.select_all
                );
            }
        }
        Err(e) => println!("the scripts' references could not be read: {e:#}"),
    }
    println!("{} objects, from {}; outside files {:?}", panel.nodes.len(), panel.file, panel.externals);
    for index in 0..panel.nodes.len() {
        if let Some((key, case)) = trp_data::interface::screen::localized(&panel, index) {
            println!("localized {index} {:?}: {key:?} case {case}", panel.nodes[index].path);
        }
    }
    let size = std::env::var("PROBE_SIZE").ok().and_then(|s| s.split_once('x').and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?]))).unwrap_or([1920.0, 1080.0]);
    for (index, node) in panel.nodes.iter().enumerate() {
        let placed = panel.placed(index, size).rect;
        let scripts: Vec<(&str, usize)> = node.scripts.iter().map(|(name, fields)| (name.as_str(), fields.len())).collect();
        let image = node.image.map(|image| (image.sprite, image.color, image.kind));
        let text = node.text.as_ref().map(|t| (t.text.clone(), t.font, t.size, t.style, t.horizontal, t.vertical, t.color, t.margin, t.auto_size, t.spacing, t.kerning));
        println!("{index} {:?}{}: at {:?} size {:?} offset {:?} anchors {:?}-{:?} pivot {:?}; image {:?}; text {:?}; group {:?}; element {:?}; scripts {:?}", node.path, if node.active { "" } else { " (off)" }, placed, node.layout.size_delta, node.layout.offset, node.layout.anchor_min, node.layout.anchor_max, node.layout.pivot, image, text, node.group, node.element, scripts);
    }
    Ok(())
}

pub fn glow(assets: &mut Assets) -> Result<()> {
    let glow = trp_data::interface::screen::malfunction_glow(&assets.install)?;
    println!("{:?} placed on 1920 by 1080 {:?}", glow.panel.nodes[0].layout, glow.panel.placed_in(0, [1920.0, 1080.0]).rect);

    for (kind, sprite, curve) in &glow.glows {
        let last = curve.keys.last().map_or(0.0, |key| key.time);
        println!("kind {kind}: {sprite:?}, {} keys over {last:.4} s; at 0.11 {:.3}, 0.25 {:.3}", curve.keys.len(), curve.evaluate(0.11), curve.evaluate(0.25));
    }
    Ok(())
}

pub fn inspectpreview(assets: &mut Assets) -> Result<()> {
    let preview = trp_data::interface::edit_build::InspectPreview::load(&assets.install)?;
    println!("camera: fov {} near {} far {} at {:?}; photographic exposure {:?}", preview.fov, preview.near, preview.far, preview.camera_position, preview.exposure);
    for light in &preview.lights {
        println!("  {:?}: kind {} on {} colour {:?} intensity {} range {} at {:?} turned {:?} mask {:#x}", light.name, light.kind, light.on, light.color, light.intensity, light.range, light.position, light.rotation, light.culling_mask);
    }
    Ok(())
}
