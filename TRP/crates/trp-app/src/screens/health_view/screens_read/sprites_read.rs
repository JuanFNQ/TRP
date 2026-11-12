//! The pictures the screens' scripts put on them, and the repair, split and tooltip windows read.

use super::*;

/// The edit build screen's and the SKILLS tab's pictures, the bars', marks', slots', icons' and the OVERALL tab's,
/// the crate's stripes, and the repair window, the window that asks how many and the two tooltips.
pub(super) fn read_sprites(look: &mut HealthLook, install: &trp_data::Install, file: &str, mut reader: &mut PictureReader, mut edit_build: Option<EditBuildLook>) {
    // The edit build screen's pictures its scripts put on it.
    if let Some(screen) = edit_build.as_mut() {
        for name in EB_BONES {
            let Some((file, id)) = trp_data::interface::screen::resource_sprite(install, &format!("Modding Screen Icons/{name}")) else { continue };
            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                screen.bones.insert(name.to_owned(), picture);
            }
        }
        for (at, name) in ["button_mod_dropdown_open", "button_mod_dropdown_close"].iter().enumerate() {
            screen.dropdown[at] = trp_data::interface::screen::resource_sprite(install, name).and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures));
        }
    }
    look.edit_build = edit_build;
    // The SKILLS tab: its page, a skill's two views, the
    // tooltip, and the pictures its scripts put on them.
    look.skills = crate::screens::skills_view::SkillsLook::read(install, file, &mut reader, &mut look.textures);
    // The inspect window's bar's pictures and the attributes'
    // icons.
    if let Some(rows) = look.info_rows.as_mut() {
        let figures = rows.bar_look.bar.clone().unwrap_or_default();
        rows.add = figures.add_sprite.and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures));
        rows.sub = figures.sub_sprite.and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures));
        rows.unchecked_chamber = rows.unchecked_sprite.clone().and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures));
        if rows.unchecked_chamber.is_none() {
            eprintln!("the parts' panel's picture of a chamber not checked could not be read; such a slot shows none");
        }
    }
    // The armour classes' marks (`Sprite Assets/armor_classes`).
    match trp_data::interface::screen::tmp_sprite_names(install, "Sprite Assets/armor_classes") {
        Ok(marks) => {
            for (name, (file, id)) in marks {
                let Some(class) = name.strip_prefix("armor_classes_").and_then(|class| class.parse::<u32>().ok()) else { continue };
                if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                    look.class_marks.insert(class, picture);
                }
            }
        }
        Err(e) => eprintln!("the game's marks of the armour classes could not be read; a tooltip shows none: {e:#}"),
    }
    // The slots' pictures, by their names.
    match trp_data::interface::screen::resource_sprites(install, "Slots/") {
        Ok(sprites) => {
            for (name, (file, id)) in sprites.into_iter().filter(|(name, _)| !name.contains('/')) {
                if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                    look.slot_pictures.insert(name, picture);
                }
            }
        }
        Err(e) => eprintln!("the game's pictures of the slots could not be read; an empty slot of a thing's parts shows none: {e:#}"),
    }
    match trp_data::interface::screen::attribute_icons(install) {
        Ok(icons) => {
            for (name, (file, id)) in icons {
                if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                    look.attribute_icons.insert(name, picture);
                }
            }
        }
        Err(e) => eprintln!("the game's icons of a thing's figures could not be read; its rows have none: {e:#}"),
    }
    match trp_data::interface::screen::figure_icons(install) {
        Ok(tables) => {
            for (number, table) in tables.into_iter().enumerate() {
                for (name, (file, id)) in table {
                    let name = if number == 2 { trp_sim::condition::health::BuffKind::of_name(&name).name().to_owned() } else { name };
                    if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                        look.figure_icons[number].insert(name, picture);
                    }
                }
            }
        }
        Err(e) => eprintln!("the game's icons of medicine's figures could not be read; their rows have none: {e:#}"),
    }
    // The pictures the OVERALL tab's scripts put on it: the
    // level's rank, the side's (the BEAR's, the side of the
    // character's starting kit, `PROVISIONAL`) and the
    // prestige's (none: `_noSprite`).
    if let Some(overall) = &look.overall {
        let [bear, _, none] = overall.side_sprites.clone();
        let ranks = RANKS.map(|name| (name, trp_data::interface::screen::resource_sprite(install, &format!("Level Icons/{name}"))));
        // The name's mark and colour by the profile's member category
        // (`ChatSpecialIcon.Show`: `GetDataByMemberCategory`'s sprite
        // and `IconColor`; the default category without a profile).
        let category = install.profile_name().map_or(0, |(_, category)| category);
        let member = match trp_data::setup::hard_settings::MemberIcons::load(install) {
            Ok(icons) => icons.of(category).cloned(),
            Err(e) => {
                eprintln!("the game's member marks could not be read; the name keeps its own colour and mark: {e:#}");
                None
            }
        };
        look.member_colour = member.as_ref().map(|member| member.colour);
        let member_sprite = member.and_then(|member| (member.sprite.0 == 0 && member.sprite.1 != 0).then(|| ("resources.assets".to_owned(), member.sprite.1)));
        for (name, sprite) in ranks.into_iter().chain([("side", bear), ("prestige", none), ("member", member_sprite)]) {
            let Some((file, id)) = sprite else { continue };
            if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                look.overall_pictures.insert(name, picture);
            }
        }
    }
    // The crate panel's stripes cut to their masks, from the pictures'
    // pixels (the tabs' fills with the battle screen's part).
    if let Some(stash) = &look.stash {
        look.stash_fills = masked_fills(&stash.panel, &look.stash_drawn, &look.textures, &mut HashMap::new());
    }
    // Each container slot's empty picture (`SearchableSlotView.
    // Show`: none for the pockets).
    for slot in ["TacticalVest", "Backpack", "SecuredContainer"] {
        let Some((file, id)) = trp_data::interface::screen::resource_sprite(install, &format!("Slots/Equipment/{slot}")) else { continue };
        if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
            look.container_backs.insert(slot.to_owned(), picture);
        }
    }
    // A part's marks (`ItemViewFactory.LoadModClassIconSprite`,
    // `LoadModIconSprite`): its class's and its kind's.
    let marks = MOD_CLASSES.iter().map(|(_, name)| (format!("type_{name}"), format!("Mod Types/icon_type_mod_{name}")));
    let kinds = MOD_KINDS.iter().map(|(_, name)| (format!("mod_{name}"), format!("Mod Tactical Types/icon_mod_{name}")));
    // And a plate's armour class's, which a plate that may be
    // taken out shows instead (`ItemViewStats.
    // GetArmorPlateClassIconName`).
    let plates = (1..=6).map(|class| (format!("plate_{class}"), format!("Mod Types/icon_type_mod_armor_plate_{class}")));
    for (key, path) in marks.chain(kinds).chain(plates) {
        let Some((file, id)) = trp_data::interface::screen::resource_sprite(install, &path) else { continue };
        if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
            look.mod_pictures.insert(key, picture);
        }
    }
    // The icons of the menu's lines (`InteractionButtonsContainer`:
    // `Characteristics/Icons/<line>`).
    for line in trp_sim::items::item_menu::Line::ALL {
        let Some((file, id)) = trp_data::interface::screen::resource_sprite(install, &format!("Characteristics/Icons/{}", line.name())) else { continue };
        if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
            look.menu_icons.insert(line.name(), picture);
        }
    }
    // The wish list's: Remove's as the menu's, the groups' from
    // the static icons, and the outlined ones of a view.
    if let Some((file, id)) = trp_data::interface::screen::resource_sprite(install, "Characteristics/Icons/RemoveFromWishlist") {
        if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
            look.menu_icons.insert("RemoveFromWishlist", picture);
        }
    }
    match trp_data::interface::screen::wishlist_icons(install) {
        Ok([plain, outlined]) => {
            for (name, (file, id)) in plain {
                if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                    look.menu_icons.insert(name, picture);
                    look.wish_icons.insert(name, picture);
                }
            }
            for (name, (file, id)) in outlined {
                if let Some(picture) = reader.sprite(&file, id, &mut look.textures) {
                    look.wish_outlines.insert(name, picture);
                }
            }
        }
        Err(e) => eprintln!("the game's icons of the wish list could not be read; its lines and marks have none: {e:#}"),
    }
    // The repair window, as its scripts leave it for a kit: its
    // warnings' templates copied as they are needed, its loader,
    // its repair's progress, the rank and standing (a trader's),
    // the slider shown by its script.
    match trp_data::interface::screen::repair_window(install) {
        Ok(panel) => {
            let scripted: Vec<usize> = REPAIR_SCRIPTED.iter().filter_map(|path| panel.find(path)).collect();
            let turned: Vec<usize> = REPAIR_TEMPLATES.iter().filter_map(|path| panel.find(path)).collect();
            let by_script: Vec<usize> = scripted.iter().chain(&turned).copied().collect();
            look.repair_drawn.shown = shown_of(&panel.nodes, &turned, &[], &by_script);
            look.repair_drawn.pictures = reader.panel(&panel, &look.repair_drawn.shown, file, &mut look.textures);
            look.repair_figures = trp_data::interface::screen::repair_figures(&panel).unwrap_or_else(|e| {
                eprintln!("the repair window's scripts could not be read; its kit's icons and colours are the prefab's: {e:#}");
                trp_data::interface::screen::RepairFigures::default()
            });
            let figures = look.repair_figures.clone();
            look.repair_icons = [figures.in_stash, figures.to_pay].map(|sprite| sprite.and_then(|(file, id)| reader.sprite(&file, id, &mut look.textures)));
            look.repair = Some(panel);
        }
        Err(e) => eprintln!("the game's repair window could not be read; the app's is drawn: {e:#}"),
    }
    // The window that asks how many, its pictures read for both
    // its sliders (the prefab has both on).
    match trp_data::interface::screen::split_look(install) {
        Ok(split) => {
            let shown = shown_of(&split.panel.nodes, &[], &[], &[]);
            look.split_pictures = reader.panel(&split.panel, &shown, &split.panel.file, &mut look.textures);
            look.split = Some(split);
        }
        Err(e) => eprintln!("the game's window that asks how many could not be read; the app's is drawn: {e:#}"),
    }
    // The game's two tooltips, the simple one and the one of
    // lines, with their grounds, frames and frames' shadows (the
    // template of a line on, to read its separator).
    match trp_data::interface::screen::simple_tooltip(install) {
        Ok(tooltip) => {
            look.tooltip_drawn.shown = shown_of(&tooltip.nodes, &[], &[], &[]);
            look.tooltip_drawn.pictures = reader.panel(&tooltip, &look.tooltip_drawn.shown, &tooltip.file, &mut look.textures);
            look.tooltip = Some(tooltip);
        }
        Err(e) => eprintln!("the game's tooltip could not be read; the app's look is used for it: {e:#}"),
    }
    match trp_data::interface::screen::multi_line_tooltip(install) {
        Ok(tooltip) => {
            let rows: Vec<usize> = tooltip.find("MultiLineRow").into_iter().collect();
            look.multi_tooltip_drawn.shown = shown_of(&tooltip.nodes, &rows, &[], &[]);
            look.multi_tooltip_drawn.pictures = reader.panel(&tooltip, &look.multi_tooltip_drawn.shown, &tooltip.file, &mut look.textures);
            look.multi_tooltip = Some(tooltip);
        }
        Err(e) => eprintln!("the game's tooltip of lines could not be read; the app's look is used for it: {e:#}"),
    }
    if !reader.missing.is_empty() {
        eprintln!("the inventory's pictures of {} could not be read; they are left out", reader.missing.join(", "));
    }
}
