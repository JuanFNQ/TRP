//! Tests that read the HEALTH, GEAR, battle bar, wish list and inspect panels.

use super::*;

/// The inventory's health panel as the install's scene has it
/// (`docs/spec/health.md`, "The HEALTH tab"): its seven views and their
/// pieces named by the panel's own scripts, its figures, the left panel's
/// size on a screen 1920 by 1080, and its pictures read through trees
/// borrowed from a bundle; the views' glow hidden as their animator
/// stands, and the arrows' bob, by the scene's own clips.
#[test]
fn the_health_panel_is_read_as_the_scene_has_it() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let gear = trp_data::interface::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::interface::screen::HealthPanel::load(&install, &gear.file).expect("the health panel");
    let views = &health.views;
    assert_eq!(views.parts.len(), 7);
    let names: Vec<&str> = views.parts.iter().map(|view| health.panel.nodes[view.view].name.as_str()).collect();
    assert_eq!(names, ["Head", "Chest", "Stomach", "Left Arm", "Right Arm", "Left Leg", "Right Leg"]);
    for view in &views.parts {
        for piece in [view.value, view.bar, view.border, view.shape, view.broken, view.effects] {
            assert!(piece.is_some(), "{}: a piece of its view is not named", health.panel.nodes[view.view].name);
        }
    }
    for name in ["OverallHealth", "Poisoning", "Radiation", "BloodPressure", "Energy", "Hydration", "Temperature"] {
        assert!(views.parameter(name).and_then(|figure| figure.current).is_some(), "{name}");
    }
    assert!(views.parameter("Weight").is_none(), "the health tab has no weight");
    // The left side's width (a share of the screen and a size) shared by
    // its two children, with the space between them.
    let left = health.left_panel([1920.0, 1080.0]);
    assert!((left[0] - 604.163).abs() < 0.01 && (left[1] - 902.0).abs() < 0.01, "{left:?}");
    let [x, y, w, h] = health.gear_in(left);
    assert!((x - (left[0] - 494.0) / 2.0).abs() < 0.01 && (y - 100.0).abs() < 0.01 && w == 494.0 && h == 650.0, "{:?}", [x, y, w, h]);
    // The chest's view where its anchors and offset put it.
    let chest = views.parts[1].view;
    let [cx, cy, cw, ch] = health.panel.rect(chest, left);
    assert!((cx - (left[0] * 0.5 - 50.9)).abs() < 0.01 && (cy - (902.0 - (0.887_820_5 * 902.0 - 162.948_12))).abs() < 0.01 && cw == 130.0 && ch == 19.0, "{:?}", [cx, cy, cw, ch]);
    // The silhouette's picture and its texture.
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&install).expect("the trees to read pictures by");
    let silhouette = health.panel.find("Silhouette/Character Silhouette").expect("the silhouette");
    let (file, id) = health.panel.nodes[silhouette].image.and_then(|image| image.sprite).expect("its sprite");
    let name = health.panel.externals[file as usize - 1].clone();
    let cut = pictures.sprite(&name, id).expect("the sprite");
    let texture = pictures.texture(&cut.texture.0, cut.texture.1).expect("its texture");
    assert!(texture.width >= cut.rect[2] as u32 && texture.height >= cut.rect[3] as u32 && !texture.data.is_empty(), "{cut:?}");
    let covers = cut.covers();
    assert!(covers.iter().all(|c| (0.0..=1.0).contains(c)) && covers[0] < covers[2] && covers[1] < covers[3], "{covers:?}");
    // The scene's clips: the views' glow standing, and the arrows' bob.
    let shared: Vec<String> = health.panel.externals.iter().filter(|name| name.starts_with("sharedassets")).cloned().collect();
    let clips = trp_data::interface::screen::scene_clips(&install, &mut pictures, &shared).expect("the scene's clips");
    let bound = |clip: &Clip, path: &str, property: &str| clip.others.iter().find(|b| b.path == path_hash(path) && b.attribute == path_hash(property)).map(|b| b.curve);
    let standing = clips.iter().find(|(_, _, clip)| clip.name == "Normal" && bound(clip, "BackGlowImage", "m_Color.a").is_some()).expect("the views' standing clip");
    let mut values = Vec::new();
    standing.2.sample(0.0, &mut values);
    assert_eq!(values[bound(&standing.2, "BackGlowImage", "m_Color.a").unwrap()], 0.0, "the glow is hidden standing");
    let bob = clips.iter().find(|(_, _, clip)| clip.name == "BuffPaneActive").expect("the arrows' bob");
    assert!(bob.2.looping && bound(&bob.2, "", "m_AnchoredPosition.y").is_some(), "{}", bob.2.name);
}

/// The GEAR tab's strip of figures as its row lays it out while the game
/// runs (`HorizontalLayoutGroup`: four panels asking 208 each in a strip
/// of 532.08 on a 16:9 screen, so 133.02 each from 36 units in), what its
/// script names, and the game's own fonts kept whole in `resources.assets`
/// (the panel's texts drawn in Bender).
#[test]
fn the_gear_tab_s_strip_and_the_game_s_fonts_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let gear = trp_data::interface::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::interface::screen::HealthPanel::load(&install, &gear.file).expect("the health panel");
    let strip = health.strip.as_ref().expect("the strip of figures");
    let left = health.left_panel([1920.0, 1080.0]);
    let names = ["Weight", "Overall Health", "Hydration", "Energy"];
    for (n, name) in names.iter().enumerate() {
        let index = strip.find(name).unwrap_or_else(|| panic!("{name}"));
        let [x, y, w, h] = strip.placed_in(index, left).rect;
        let width = (0.620_111_8 * left[0] + 157.430_18) / 4.0;
        assert!((w - width).abs() < 0.05 && (x - (left[0] * 0.189_944_1 - 157.430_18 / 2.0 + n as f32 * width)).abs() < 0.05, "{name}: {:?}", [x, y, w, h]);
        assert!((y - (902.0 - 20.6 - 46.8)).abs() < 0.05 && (h - 46.8).abs() < 0.01, "{name}: {:?}", [x, y, w, h]);
    }
    for name in ["Weight", "OverallHealth", "Hydration", "Energy"] {
        assert!(trp_data::interface::screen::figure_named(&health.strip_figures, name).and_then(|f| f.current).is_some(), "{name}");
    }
    let weight = trp_data::interface::screen::figure_named(&health.strip_figures, "Weight").unwrap();
    assert!(weight.alternative[3] > 0.0, "the weight has a colour of its own between its limits");
    // The fonts: Bender kept whole, and the panel's font assets made from it.
    let fonts = trp_data::interface::screen::build_fonts(&install, "resources.assets").expect("the build's fonts");
    let bender = fonts.iter().find(|(name, _)| name == "Jovanny Lemonad - Bender").expect("Bender");
    assert_eq!(&bender.1[..4], b"OTTO");
    let header = health.panel.find("Chest/Header/Text").and_then(|text| health.panel.nodes[text].text.clone()).expect("a header's text");
    let file = health.panel.externals[header.font.0 as usize - 1].clone();
    let asset = trp_data::interface::screen::script_name(&install, &file, header.font.1).expect("its font asset's name");
    assert_eq!(trp_data::interface::screen::font_for(&asset, &fonts).map(|(name, _)| name.as_str()), Some("Jovanny Lemonad - Bender"), "{asset}");
}

/// The game's GEAR tab as `TARKOV UI` draws it (`docs/spec/inventory.md`,
/// "The game's own look"): the equipment panel's eleven slot views named
/// by its script with their borders, fills and pictures; the views of a
/// thing in a slot and in a grid read from the build's Resources; the
/// containers' column with its row template, its grid view and its rows'
/// spacing; a container's own layout of its grids; the slots' pictures;
/// and the database's colours and layouts of the items.
/// The battle screen's magazine picker (`AcceptableMagsPanel`: firing
/// spec, "The magazine picker"): put 37 pixels left of and 52 below the
/// weapon's view (the scene's offset, not the class's default), the
/// cancel's ground unchosen and chosen, a flow 748 wide padded 6 and
/// spaced 3, the cancel 64 square; two magazines of 64 by 127 and the
/// cancel lie in a row, 139 high.
#[test]
fn the_battle_bar_s_magazine_picker_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let gear = trp_data::interface::screen::GearPanel::load(&install).expect("the equipment panel");
    let quick = trp_data::interface::screen::QuickViews::load_battle(&install, &gear.file).expect("the battle screen's bar");
    let picker = quick.ammo_selector.as_ref().expect("the magazine picker");
    println!("{picker:?}");
    assert_eq!(picker.arrow_offset, [-37.0, -52.0]);
    assert_eq!(picker.padding, [6.0, 6.0, 6.0, 6.0]);
    assert_eq!((picker.spacing, picker.width, picker.cancel_size), ([3.0, 3.0], 748.0, [64.0, 64.0]));
    assert!((picker.normal[0] - 0.1254902).abs() < 1e-6 && (picker.selected[0] - 0.2509804).abs() < 1e-6);
    assert_eq!(quick.panel.nodes[picker.cancel_ground].name, "Color Panel");
    let (places, size) = picker.flow(&[[64.0, 127.0], [64.0, 127.0], [64.0, 64.0]]);
    assert_eq!(places, [[6.0, 6.0], [73.0, 6.0], [140.0, 6.0]]);
    assert_eq!(size, [748.0, 139.0]);
    // Twelve of them wrap: eleven to a row.
    let (places, _) = picker.flow(&[[64.0, 64.0]; 12]);
    assert_eq!((places[10], places[11]), ([676.0, 6.0], [6.0, 73.0]));
}

#[test]
fn the_game_s_gear_tab_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let gear = trp_data::interface::screen::GearPanel::load(&install).expect("the equipment panel");
    let health = trp_data::interface::screen::HealthPanel::load(&install, &gear.file).expect("the left panel");
    assert_eq!(health.slot_views.len(), 11, "{:?}", health.slot_views.iter().map(|(slot, _)| *slot).collect::<Vec<_>>());
    for (slot, parts) in &health.slot_views {
        for part in [parts.full_border, parts.back_image, parts.background, parts.empty_border, parts.header] {
            assert!(part.is_some(), "{slot}: a part of its view is not named");
        }
    }
    let gear_panel = health.gear_panel.as_ref().expect("the equipment panel's objects");
    assert!(gear_panel.find("CharacterGear").is_some(), "the character's outline behind the slots");
    // The views of a thing.
    let items = trp_data::interface::screen::ItemViews::load(&install).expect("the views of a thing");
    let slot = items.slot.as_ref().expect("a slot's view");
    let grid = items.grid.as_ref().expect("a grid's view");
    assert!(slot.find("Info Panel/Name").is_some() && grid.find("Info Panel/Caption").is_some());
    let caption = grid.find("Info Panel/Caption").and_then(|index| grid.nodes[index].text.clone()).expect("the caption's text");
    assert_eq!(caption.size, 11.0);
    // A grid's view framed: as large as its cells and a unit more (a 2 by
    // 1 thing, 127 by 64), its ground a unit inside its edges, its border
    // over the whole view kept a pixel to a pixel, `border_generic` sliced
    // 2 in (a line a pixel wide).
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&install).expect("the build's pictures");
    let ground = grid.find("Color Panel").expect("its ground");
    assert_eq!(grid.rect(ground, [127.0, 64.0]), [1.0, 1.0, 125.0, 62.0]);
    let edge = grid.find("Border").expect("its border");
    assert_eq!(grid.rect(edge, [127.0, 64.0]), [0.0, 0.0, 127.0, 64.0]);
    assert!(grid.nodes[edge].scripts.iter().any(|(class, _)| class == "PixelPerfectSpriteScaler"));
    let sprite_of = |panel: &trp_data::interface::screen::ScenePanel, index: usize, pictures: &mut trp_data::interface::screen::ScenePictures| {
        let (outside, id) = panel.nodes[index].image.and_then(|image| image.sprite).expect("a sprite");
        pictures.sprite(&panel.file_of(outside).expect("its file"), id).expect("the sprite")
    };
    let border = sprite_of(grid, edge, &mut pictures);
    assert_eq!((border.name.as_str(), border.border), ("border_generic", [2.0, 2.0, 2.0, 2.0]));
    // The containers' column.
    let containers = trp_data::interface::screen::ContainerViews::load(&install, &gear.file).expect("the containers' column");
    let column = containers.column.expect("its column of rows");
    assert!(column.vertical && column.spacing == 45.0 && column.padding[2] == 120.0, "{column:?}");
    assert!(containers.header.is_some() && containers.slot_panel.is_some());
    let row = containers.grids_row.expect("the default row of grids");
    assert_eq!(row.spacing, 6.0);
    let view = containers.grid.as_ref().expect("a grid's view");
    assert!(view.find("Background Tile").is_some() && view.find("Border").is_some());
    // Its frame 5 outside it, kept a pixel to a pixel: `border_shadow`
    // sliced 5 in, its fifth pixel the light line just outside the cells.
    let frame = view.find("Border").expect("its frame");
    assert_eq!(view.rect(frame, [64.0, 64.0]), [-5.0, -5.0, 74.0, 74.0]);
    assert!(view.nodes[frame].scripts.iter().any(|(class, _)| class == "PixelPerfectSpriteScaler"));
    let shadow = sprite_of(view, frame, &mut pictures);
    assert_eq!((shadow.name.as_str(), shadow.border), ("border_shadow", [5.0, 5.0, 5.0, 5.0]));
    // A container's own layout: the AVS's twelve grids in their order.
    let (_, grids) = trp_data::interface::screen::rig_layout(&install, "avs").expect("the AVS's layout");
    assert_eq!(grids.len(), 12);
    let layouts = trp_data::interface::screen::rig_layouts(&install, &["avs".to_owned()]);
    assert_eq!(layouts["avs"].1[0].rect, [0.0, -1.0, 64.0, 127.0]);
    // A view larger in its layout than its cells is sized to them about its
    // pivot (`GridView.OnGridResized`): the Rush 100's main grid, 320 by 448
    // in the prefab, 316 by 442 for its 5 by 7 cells.
    let rush = trp_data::interface::screen::rig_layouts(&install, &["rush_100".to_owned()]);
    let main = rush["rush_100"].1[3];
    let sized = main.sized([5, 7]);
    assert_eq!((main.size_delta, [sized[2], sized[3]]), ([320.0, 448.0], [316.0, 442.0]), "{main:?}");
    assert!(trp_data::interface::screen::resource_sprite(&install, "Slots/Equipment/TacticalVest").is_some());
    // The database's colours and layouts.
    let db = Database::load(&install).expect("database");
    assert!(db.items.values().filter(|item| item.background_color() != "default").count() > 100);
    assert!(db.items.values().filter(|item| !item.grid_layout_name().is_empty()).count() > 20);
    // The column's scroll view and its bar, by the pointers of their
    // scripts: a notch of the wheel 63 units, stopped at its ends, the bar
    // hidden while everything shows, from the bottom up, its handle tinted.
    let scrolling = containers.scrolling.expect("the column's scroll view");
    assert!(scrolling.vertical && !scrolling.across && scrolling.clamped && scrolling.hides && scrolling.sensitivity == 63.0, "{scrolling:?}");
    let at = |index: Option<usize>| index.map(|index| containers.panel.nodes[index].path.as_str());
    assert_eq!(at(scrolling.view), Some("Scrollview Parent/Containers Scrollview"));
    assert_eq!(at(scrolling.content), Some("Scrollview Parent/Containers Scrollview/Content"));
    let bar = scrolling.bar.expect("its bar");
    assert!(bar.bottom_up && bar.tinted == bar.handle, "{bar:?}");
    assert_eq!(at(bar.handle), Some("Scrollbar/Sliding Area/Handle"));
    let tints = bar.tints.expect("the bar's tints");
    assert!((tints.pressed[0] - 0.784).abs() < 1e-3 && (tints.highlighted[0] - 0.961).abs() < 1e-3 && (tints.fade - 0.1).abs() < 1e-6, "{tints:?}");
    // The handle up and down its sliding area, 10 in from the bar's ends,
    // 8 longer at each end than its share, and 6 wide.
    let handle = bar.handle_in(&containers.panel, [604.2, 902.0], 1.0, 0.5).expect("the handle's place");
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.05);
    assert!(near(handle, [596.2, 2.0, 6.0, 457.0]), "{handle:?}");
    // The screen's places at 1920 by 1080: the left side, the right's
    // panel, the left panel's ground and the left side's frame, whose
    // texture is sampled at its nearest pixel (its middle is clear).
    let screen = [1920.0, 1080.0];
    assert!(near(health.left_side_in(screen), [11.6, 48.0, 1213.3, 902.0]), "{:?}", health.left_side_in(screen));
    assert!(near(health.stash_in(screen).expect("the right's panel"), [1228.0, 75.0, 680.0, 873.0]), "{:?}", health.stash_in(screen));
    assert!(health.left_ground.is_some_and(|ground| (ground[3] - 0.498).abs() < 1e-3));
    let border = health.border.as_ref().expect("the left side's frame");
    let (outside, sprite) = border.nodes[0].image.and_then(|image| image.sprite).expect("the frame's sprite");
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&install).expect("the screens' pictures");
    let cut = pictures.sprite(&border.file_of(outside).expect("its file"), sprite).expect("the frame's sprite");
    assert!(pictures.texture(&cut.texture.0, cut.texture.1).expect("its texture").point);
    // The bar of tabs: eight tabs in the game's order, 26 over one another,
    // each its words' sample and 85 more; the widest is ACHIEVEMENTS.
    let mut bar = trp_data::interface::screen::TabBar::load(&install, &gear.file).expect("the bar of tabs");
    assert_eq!(bar.gap, -26.0);
    let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
    assert_eq!(keys, ["OVERALL", "GEAR", "HEALTH_TAB", "SKILLS", "MAP", "TASKS", "ACHIEVEMENTS", "Prestige"]);
    assert!(bar.tabs.iter().enumerate().all(|(order, tab)| tab.order == order as i32 && tab.width_adjustment == 85.0 && tab.normal.is_some() && tab.selected.is_some() && tab.hover.is_some()));
    // TextMeshPro's width of a text: the Bender asset's advances at its
    // sampling size of 48 and 2 hundredths of the size after every letter
    // but the last, as the scene keeps the tabs' samples (65.59).
    let sample = bar.panel.nodes[bar.tabs[0].size_definers[0]].text.clone().expect("the tab's sample");
    let font = trp_data::interface::screen::TmpFont::read(&install, &bar.panel.file_of(sample.font.0).expect("its font's file"), sample.font.1).expect("Bender's asset");
    assert_eq!((font.point_size, font.normal_spacing), (48.0, 2.0));
    assert!(font.bold.is_some(), "its bold typeface");
    assert_eq!(font.width("OVERALL", 16.0, 0.0, false), 65.59);
    let texts = trp_data::character::inventory::texts(&install, "en", &keys).expect("the install's words");
    let say = |key: &str| texts.get(&key.to_lowercase()).cloned();
    let width = bar.width(&install, &say);
    assert!((width - (font.width("ACHIEVEMENTS", 16.0, 0.0, false) + 85.0)).abs() < 1e-3, "{width}");
    bar.lay_out(width);
    let places: Vec<f32> = bar.tabs.iter().map(|tab| bar.panel.placed_in(tab.object, screen).rect[0]).collect();
    assert!(places.iter().enumerate().all(|(order, x)| (x - (10.0 + order as f32 * (width - 26.0))).abs() < 1e-3), "{places:?}");
    // A line's height as TextMeshPro has it: Bender's ascent to descent at
    // 16, 17.61, which with its padding (4 and 6) is the 27.61 the scene
    // keeps for the corpse panel's box of a container's name.
    assert_eq!(font.height(16.0), 17.61);
    // The panel for a container opened: what its script names, its 18
    // filters in a column 34 apart (a vertical group laid out), its scroll
    // view elastic with no elasticity (back at once) and its bar.
    let mut stash = trp_data::interface::screen::StashViews::load(&install, &gear.file).expect("the panel for a container opened");
    let path = |index: Option<usize>| index.map(|index| stash.panel.nodes[index].path.clone());
    assert_eq!(path(stash.name).as_deref(), Some("Simple Panel/Container Name/Text"));
    assert_eq!(path(stash.grids).as_deref(), Some("Simple Panel/Mask/Scroll Area/Content"));
    assert_eq!(stash.filter_tabs.len(), 18);
    let size = [680.0, 873.0];
    let tops: Vec<f32> = stash.filter_tabs.iter().filter_map(|tab| tab.normal).map(|normal| stash.panel.rect(normal, size)[1]).collect();
    assert!(tops.windows(2).all(|pair| (pair[1] - pair[0] - 34.0).abs() < 1e-3), "{tops:?}");
    let scrolling = stash.scrolling.expect("its scrolling");
    assert!(scrolling.elastic && scrolling.elasticity == 0.0 && scrolling.sensitivity == 63.0 && scrolling.bar.is_some_and(|bar| bar.bottom_up), "{scrolling:?}");
    // The quick-use panel: twelve views in the game's order of their
    // bindings, each with what its script names, 70 apart and 10 more
    // between the two groups, the panel where the game's screen has it.
    let mut quick = trp_data::interface::screen::QuickViews::load(&install, &gear.file).expect("the quick-use panel");
    let keys: Vec<&str> = quick.slots.iter().map(|slot| slot.key.as_str()).collect();
    assert_eq!(keys, ["Grenade", "ItemV", "Item1", "Item2", "Item3", "Item4", "Item5", "Item6", "Item7", "Item8", "Item9", "Item10"]);
    assert!(quick.slots.iter().all(|slot| slot.install.is_some() && slot.hot_key.is_some() && slot.caption.is_some() && slot.border.is_some()));
    for slot in quick.slots.clone() {
        quick.panel.nodes[slot.object].active = true;
    }
    let place = quick.quick_in(screen);
    assert!(near(place, [558.5, 935.0, 854.2, 115.0]), "{place:?}");
    let lefts: Vec<f32> = quick.slots.iter().map(|slot| quick.panel.rect(slot.object, [place[2], place[3]])[0]).collect();
    let expected = [0.0, 70.0, 140.0, 210.0, 280.0, 354.0, 424.0, 494.0, 564.0, 634.0, 704.0, 774.0];
    assert!(lefts.iter().zip(expected).all(|(left, wanted)| (left - wanted).abs() < 1e-3), "{lefts:?}");
    // Its header on, its row sized to it (`ContentSizeFitter`): above the
    // panel's top, as the game shows it.
    let header = stash.header.expect("its header");
    stash.panel.nodes[header].active = true;
    let [_, panel_top, ..] = stash.panel.rect(stash.simple, size);
    let [_, top, _, height] = stash.panel.rect(header, size);
    assert!(height == 20.0 && top + height <= panel_top, "{top} {height} {panel_top}");
}

/// The wish list's menu and icons (`InteractionButtonsContainer.
/// SubMenuTemplate`, `StaticIcons.WishlistSprites`, `WishlistSpritesOutline`):
/// the item's menu's menu of lines found with its three templates, and the
/// five groups' icons twice over, the outlined ones others.
#[test]
fn the_wish_list_s_menu_and_icons_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let sub = trp_data::interface::screen::context_submenu(&install).expect("the menu of lines");
    for template in ["Button Template", "Wishlist Template", "Dispose Template"] {
        assert!(sub.find(&format!("InteractionButtonsContainer/{template}")).is_some(), "{template}");
    }
    let [plain, outlined] = trp_data::interface::screen::wishlist_icons(&install).expect("the icons");
    println!("plain {plain:?}\noutlined {outlined:?}");
    assert_eq!((plain.len(), outlined.len()), (5, 5));
    assert!(plain.iter().zip(&outlined).all(|((a, at), (b, bt))| a == b && at != bt));
}

/// The inspect window's rows (inspect-window spec, "A row"): its three
/// templates, by its script's pointers, and the attributes' icons.
#[test]
fn the_inspect_window_s_rows_and_icons_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let window = trp_data::interface::screen::info_window(&install).expect("the window");
    let rows = trp_data::interface::screen::info_rows(&install, &window).expect("its rows");
    for (panel, parts) in [(&rows.bar, ["Icon", "Name", "Value", "bars/Full"]), (&rows.compact, ["Icon", "Name", "Value", "Icon"]), (&rows.dropdown, ["Icon", "Name", "Hover", "Hover/Arrow"])] {
        for part in parts {
            assert!(panel.find(part).is_some(), "{part}");
        }
    }
    let icons = trp_data::interface::screen::attribute_icons(&install).expect("the icons");
    for id in ["Ergonomics", "CenterOfImpact", "RecoilUp", "Velocity", "ArmorClass", "ArmorPoints", "MaxAmmoDamage", "AmmoPenetrationPower"] {
        assert!(icons.iter().any(|(key, _)| key == id), "{id}");
    }
    // Its parts' panel's slot, from another file, 64 square with its name,
    // its picture and its filters' row; how far it stretches; the slots'
    // pictures by their names.
    let slot = rows.mod_slot.as_ref().expect("the parts' slot");
    for part in ["CanvasGroup/Background Image", "CanvasGroup/Text", "CanvasGroup/filters", "CanvasGroup/Full Border", "CanvasGroup/Empty Border"] {
        assert!(slot.find(part).is_some(), "{part}");
    }
    let limit = trp_data::interface::screen::stretch_limit(&window).expect("its stretch limit");
    println!("the window stretches to {limit:?}; its slot is in {}", slot.file);
    assert!(limit[0] >= 670.0 && limit[1] >= 500.0, "{limit:?}");
    // The armour classes' marks a text names (`<sprite name="armor_classes_N">`),
    // each the engine's sprite its glyph points to: class 1 the first glyph's
    // (`armor_classes_0`), class 5 the third's.
    let classes = trp_data::interface::screen::tmp_sprite_names(&install, "Sprite Assets/armor_classes").expect("the classes' marks");
    let of = |name: &str| classes.iter().find(|(own, _)| own == name).map(|(_, (_, id))| *id);
    assert_eq!(classes.len(), 6, "{classes:?}");
    assert!(of("armor_classes_1").is_some() && of("armor_classes_5").is_some() && of("armor_classes_1") != of("armor_classes_5"), "{classes:?}");
    let sprites = trp_data::interface::screen::resource_sprites(&install, "Slots/").expect("the slots' pictures");
    for name in ["mod_magazine", "mod_muzzle", "mod_stock", "patron_in_weapon", "armor_plate"] {
        assert!(sprites.iter().any(|(path, _)| path == name), "{name} among {} pictures", sprites.len());
    }
}

/// The rows' colours and pictures from their scripts.
#[test]
fn the_inspect_window_s_rows_colours_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let window = trp_data::interface::screen::info_window(&install).expect("the window");
    let rows = trp_data::interface::screen::info_rows(&install, &window).expect("its rows");
    let bar = trp_data::interface::screen::row_look(&rows.bar, "CharacteristicPanel").expect("the bar's look");
    let figures = bar.bar.expect("a bar's own");
    assert!((figures.sub[0] - 0.758).abs() < 0.01 && (figures.add[2] - 0.622).abs() < 0.01, "{figures:?}");
    assert!(figures.add_sprite.is_some() && figures.sub_sprite.is_some());
    let compact = trp_data::interface::screen::row_look(&rows.compact, "CompactCharacteristicPanel").expect("the compact row's look");
    assert!(compact.bar.is_none() && (compact.decreasing[0] - 0.769).abs() < 0.01, "{compact:?}");
}

/// The game scene's panel of an examine (`UsingPanel`) read with its
/// script's figures, and laid out as `courses10`'s `examine-multiple` has
/// it at 1920 by 1080 (its boxes at 2560 by 1440 over 4/3): its name a line
/// (15.4 high) or two (31.36), the caption's plate raised over two.
#[test]
fn the_panel_of_an_examine_is_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured");
        return;
    };
    let using = trp_data::interface::screen::using_panel(&install).expect("the using panel");
    assert_eq!(using.examine_color.map(|c| (c * 255.0).round()), [84.0, 193.0, 255.0, 255.0], "the examine's colour");
    let (caption, name, fill) = (using.caption.expect("caption"), using.name.expect("name"), using.fill.expect("fill"));
    let screen = [1920.0, 1080.0];
    let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(&b).all(|(a, b)| (a - b).abs() < 0.02);
    for (high, plate, item) in [(15.4, [867.0, 897.6, 186.0, 26.0], [862.0, 928.6, 196.0, 25.4]), (31.36, [867.0, 881.64, 186.0, 26.0], [862.0, 912.64, 196.0, 41.36])] {
        let mut panel = using.panel.clone();
        let element = panel.nodes[name].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
        panel.nodes[name].element = Some(trp_data::interface::screen::Element { preferred: [176.0, high], ..element });
        let plate_at = panel.placed_in(panel.nodes[caption].parent.expect("the caption's plate"), screen).rect;
        let item_at = panel.placed_in(panel.nodes[name].parent.expect("the name's plate"), screen).rect;
        assert!(close(plate_at, plate), "the caption's plate at {plate_at:?}, the take's {plate:?}");
        assert!(close(item_at, item), "the name's plate at {item_at:?}, the take's {item:?}");
    }
    let bar = using.panel.placed_in(fill, screen).rect;
    assert!(close(bar, [818.1, 967.0, 283.8, 4.9]), "the fill at {bar:?}");
}

/// The member categories' marks (`ChatSpecialIconSettings`, OVERALL's
/// nickname and its mark): read, with a mark for the gold-crown category
/// (`UniqueId`, 2) whose colour is not white, and the priority's first
/// listed flag chosen for a category of several.
#[test]
fn the_member_categories_marks_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let icons = trp_data::setup::hard_settings::MemberIcons::load(&install).expect("the marks");
    eprintln!("priority {:?}", icons.priority);
    for icon in &icons.icons {
        eprintln!("{} {} {:?} {:?}", icon.name, icon.category, icon.sprite, icon.colour);
    }
    let crown = icons.of(2).expect("UniqueId's mark");
    assert_eq!(crown.category, 2);
    assert!(crown.colour[..3].iter().any(|&c| c < 0.99), "{crown:?}");
}

/// The game's text fields as the install keeps them (`docs/spec/
/// inventory.md`, "A text field"): the tag window's and the MASTERING
/// page's search, each with its text object, the same selection colour, a
/// caret one unit wide in the text's colour blinking 0.85 times a second,
/// all selected as it takes the keys, and its own most letters, 16 and 20.
#[test]
fn the_text_fields_figures_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let tag = trp_data::interface::screen::tag_window(&install).expect("the tag window");
    let input = tag.find("Inner/Contents/TagInput").expect("its field");
    let of_tag = trp_data::interface::screen::input_field(&tag, input, "ValidationInputField").expect("its field's figures");
    assert_eq!(tag.nodes[of_tag.text].path, "Inner/Contents/TagInput/Text Area/Text");
    let gear = trp_data::interface::screen::GearPanel::load(&install).expect("the equipment panel");
    let page = trp_data::interface::screen::SkillsScreen::load(&install, &gear.file).expect("the skills page");
    let of_search = trp_data::interface::screen::MasteringScreen::of(&page).expect("the mastering page").search.expect("its search field");
    for (field, limit) in [(of_tag, 16), (of_search, 20)] {
        assert!(field.selection.iter().zip([0.659, 0.808, 1.0, 0.753]).all(|(read, wanted)| (read - wanted).abs() < 0.001), "{field:?}");
        assert_eq!((field.caret_width, field.blink_rate, field.caret_color, field.select_all, field.limit), (1.0, 0.85, None, true, limit));
    }
}
