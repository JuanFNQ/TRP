//! The windows read: a thing's inspect window with its rows, a container's grids, a tag's and the right-click menus.

use super::*;

/// The window that shows a thing and its rows, the window of a container's grids, the window that edits a tag,
/// the menu of a right click and the menu of a line's own lines, with their pictures.
pub(super) fn read_windows(look: &mut HealthLook, install: &trp_data::Install, read: &mut ReadPanel) {
    // The window that shows a thing, as its script leaves it
    // for a thing with no tag (`InfoWindow.UpdateTag`), not on
    // the wish list (`_wishlistIcon`) and with its preview drawn
    // (its loader gone); the panel of the parts on it off
    // (`_modsPanel`, on for a thing with slots: not built).
    match trp_data::interface::screen::info_window(install) {
        Ok(mut info) => {
            let off: Vec<usize> = INFO_OFF.iter().filter_map(|path| info.find(path)).collect();
            for &index in &off {
                info.nodes[index].active = false;
            }
            // (Its loader while the model is made, and a
            // weapon's durability row: as its script shows
            // them.)
            let scripted: Vec<usize> = [INFO_TAG, INFO_WISH, INFO_LOADER, INFO_DURABILITY, INFO_MODS, INFO_MASTERING, INFO_MASTERING_BADGE, INFO_CORNER].iter().filter_map(|path| info.find(path)).collect();
            if let Some(wish) = info.find(INFO_WISH) {
                info.nodes[wish].active = false;
            }
            // (The buttons' templates and the mastering row's
            // value read as on, for the copies made of them.)
            let turned: Vec<usize> = scripted.iter().copied().chain(INFO_BUTTON_TEMPLATES.iter().chain([&INFO_MASTERING_VALUE]).filter_map(|path| info.find(path))).collect();
            look.info_drawn.shown = shown_of(&info.nodes, &turned, &off, &scripted);
            look.info_stretch = trp_data::interface::screen::stretch_limit(&info);
            look.info_drawn.pictures = read(&info, &look.info_drawn.shown, &mut look.textures);
            look.info_mass_font = info.find(INFO_MASS).and_then(|mass| info.nodes[mass].text.as_ref()).and_then(|text| trp_data::interface::screen::TmpFont::read(install, &info.file_of(text.font.0)?, text.font.1).ok());
            // Its rows' templates: each whole, its value on
            // (`SetValues`), a bar's comparison never.
            match trp_data::interface::screen::info_rows(install, &info) {
                Ok(rows) => {
                    let mut drawn_of = |panel: &trp_data::interface::screen::ScenePanel| {
                        let turned: Vec<usize> = panel.find("Value").into_iter().collect();
                        let never: Vec<usize> = panel.find("bars/CompareDelta").into_iter().collect();
                        let mut drawn = Drawn { shown: shown_of(&panel.nodes, &turned, &never, &[]), pictures: Vec::new() };
                        drawn.pictures = read(panel, &drawn.shown, &mut look.textures);
                        drawn
                    };
                    let (bar, compact, dropdown) = (drawn_of(&rows.bar), drawn_of(&rows.compact), drawn_of(&rows.dropdown));
                    let bar_look = trp_data::interface::screen::row_look(&rows.bar, "CharacteristicPanel").unwrap_or_default();
                    let compact_look = trp_data::interface::screen::row_look(&rows.compact, "CompactCharacteristicPanel").unwrap_or_default();
                    // A slot's pieces that its script turns on
                    // and off (`SetSlotGraphics`, `Show`); a
                    // revolver's drum's border never here.
                    let mod_slot = rows.mod_slot.map(|panel| {
                        let scripted: Vec<usize> = MOD_SLOT_PIECES.iter().filter_map(|path| panel.find(path)).collect();
                        let never: Vec<usize> = panel.find("CanvasGroup/ActiveCamoraBorder").into_iter().collect();
                        let mut drawn = Drawn { shown: shown_of(&panel.nodes, &[], &never, &scripted), pictures: Vec::new() };
                        drawn.pictures = read(&panel, &drawn.shown, &mut look.textures);
                        (panel, drawn)
                    });
                    let unchecked_sprite = rows.unchecked_chamber;
                    look.info_rows = Some(InfoRowsLook { bar: (rows.bar, bar), compact: (rows.compact, compact), dropdown: (rows.dropdown, dropdown), bar_look, compact_look, add: None, sub: None, mod_slot, unchecked_chamber: None, unchecked_sprite });
                }
                Err(e) => eprintln!("the game's rows of a thing's figures could not be read; none are listed: {e:#}"),
            }
            look.info = Some(info);
        }
        Err(e) => eprintln!("the game's window that shows a thing could not be read; the app's is drawn: {e:#}"),
    }
    // The window of a container's grids, as its script leaves it
    // in a raid with the install's `PriorityWindowMode` (Auto):
    // its priority toggle off (`GridWindow.Show`), as are its
    // tag, its sorting, its search and its loader in the prefab;
    // so its caption's row holds the icon, the caption and the
    // cross. The icon and the cross ask the row for their
    // pictures' own sizes where their elements set none (an
    // `Image`'s preferred size, which the panel's layout does
    // not have of itself). PROVISIONAL: a picture's pixel taken
    // as a unit (the sprite's `pixelsPerUnit` against the
    // canvas's not read); the icon is then 19 wide, the words
    // from 24.
    match trp_data::interface::screen::grid_window(install) {
        Ok(mut window) => {
            if let Some(priority) = window.priority {
                window.panel.nodes[priority].active = false;
            }
            // Its search's parts (`SearchableView` on the window:
            // the caption's button, the unsearched panel), on and
            // off as the search stands (`grid_window_built`).
            let search: Vec<usize> = WINDOW_SEARCH_PARTS.iter().filter_map(|path| window.panel.find(path)).collect();
            let turned: Vec<usize> = [WINDOW_SEARCH_BUTTON, WINDOW_UNSEARCHED].iter().filter_map(|path| window.panel.find(path)).collect();
            look.grid_window_drawn.shown = shown_of(&window.panel.nodes, &turned, &[], &search);
            look.grid_window_drawn.pictures = read(&window.panel, &look.grid_window_drawn.shown, &mut look.textures);
            for node in [window.icon, window.close] {
                let Some(picture) = look.grid_window_drawn.pictures[node] else { continue };
                let element = window.panel.nodes[node].element.unwrap_or(trp_data::interface::screen::Element { ignore: false, min: [-1.0; 2], preferred: [-1.0; 2], flexible: [-1.0; 2] });
                let preferred = [0, 1].map(|axis| if element.preferred[axis] >= 0.0 { element.preferred[axis] } else { picture.size[axis] });
                window.panel.nodes[node].element = Some(trp_data::interface::screen::Element { preferred, ..element });
            }
            look.grid_window = Some(window);
        }
        Err(e) => eprintln!("the game's window of a container's grids could not be read; the app's is drawn: {e:#}"),
    }
    // The window that edits a tag: the seven cells its colours'
    // panel makes from its template (`TagColorsPanel.Show`),
    // drawn as the prefab's own seven of the same colours lie
    // (where its grid puts them: 25 by 25, 15 apart, one row),
    // each cell's border, the field's text and its placeholder
    // left to its script.
    match trp_data::interface::screen::tag_window(install) {
        Ok(mut panel) => {
            let cells: Vec<usize> = (0..TAG_COLORS).filter_map(|n| panel.find(&tag_cell(n))).collect();
            for &cell in &cells {
                panel.nodes[cell].active = true;
            }
            let mut scripted: Vec<usize> = (0..TAG_COLORS).filter_map(|n| panel.find(&format!("{}/Border", tag_cell(n)))).collect();
            scripted.extend([TAG_TEXT, TAG_PLACEHOLDER].iter().filter_map(|path| panel.find(path)));
            look.tag_limit = panel.find(TAG_INPUT).and_then(|input| panel.script(input, "ValidationInputField")).and_then(trp_data::interface::screen::input_limit).unwrap_or(TAG_LIMIT);
            look.tag_field = panel.find(TAG_INPUT).and_then(|input| trp_data::interface::screen::input_field(&panel, input, "ValidationInputField").ok());
            // The field's text and placeholder kept off its edges
            // by their margins (left, top, right, bottom), put
            // into their rectangles: the panel's drawing lays a
            // text in its rectangle as it is.
            let fields: Vec<usize> = [TAG_TEXT, TAG_PLACEHOLDER].iter().filter_map(|path| panel.find(path)).collect();
            for index in fields {
                let Some(margin) = panel.nodes[index].text.as_ref().map(|text| text.margin) else { continue };
                let layout = &mut panel.nodes[index].layout;
                layout.offset = [layout.offset[0] + margin[0], layout.offset[1] - margin[1]];
                layout.size_delta = [layout.size_delta[0] - margin[0] - margin[2], layout.size_delta[1] - margin[1] - margin[3]];
                if let Some(text) = panel.nodes[index].text.as_mut() {
                    text.margin = [0.0; 4];
                }
            }
            look.tag_window_drawn.shown = shown_of(&panel.nodes, &[], &[], &scripted);
            look.tag_window_drawn.pictures = read(&panel, &look.tag_window_drawn.shown, &mut look.textures);
            look.tag_window = Some(panel);
        }
        Err(e) => eprintln!("the game's window that edits a tag could not be read; the app's is drawn: {e:#}"),
    }
    // The menu of a right click, its templates of a line on.
    match trp_data::interface::screen::context_menu(install) {
        Ok(menu) => {
            let templates: Vec<usize> = MENU_TEMPLATES.iter().filter_map(|path| menu.find(path)).collect();
            look.menu_drawn.shown = shown_of(&menu.nodes, &templates, &[], &[]);
            look.menu_drawn.pictures = read(&menu, &look.menu_drawn.shown, &mut look.textures);
            let caption = menu.find(&format!("{}/Text", MENU_TEMPLATES[0])).and_then(|text| menu.nodes[text].text.as_ref());
            look.menu_font = caption.and_then(|text| trp_data::interface::screen::TmpFont::read(install, &menu.file_of(text.font.0)?, text.font.1).ok());
            look.menu = Some(menu);
        }
        Err(e) => eprintln!("the game's menu of a right click could not be read; the app's is drawn: {e:#}"),
    }
    // The menu of a line's own lines, from the same templates.
    match trp_data::interface::screen::context_submenu(install) {
        Ok(mut sub) => {
            // Its holder of lines is kept off, and shown with
            // the menu (`ShowMenu`: the holder's `Show`).
            if let Some(holder) = sub.find(MENU_LINES) {
                sub.nodes[holder].active = true;
            }
            let templates: Vec<usize> = MENU_TEMPLATES.iter().filter_map(|path| sub.find(path)).collect();
            look.submenu_drawn.shown = shown_of(&sub.nodes, &templates, &[], &[]);
            look.submenu_drawn.pictures = read(&sub, &look.submenu_drawn.shown, &mut look.textures);
            look.submenu = Some(sub);
        }
        Err(e) => eprintln!("the game's menu of a line's own lines could not be read; the app's is drawn: {e:#}"),
    }
}
