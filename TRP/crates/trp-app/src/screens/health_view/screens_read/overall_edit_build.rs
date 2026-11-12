//! The OVERALL tab and the edit build screen read.

use super::*;

/// The OVERALL tab as it shows in a raid, and the edit build screen with the workbench on (returned, its pictures
/// still to put on it).
pub(super) fn read_overall_and_edit_build(look: &mut HealthLook, install: &trp_data::Install, file: &str, clothing: bool, workbench: bool, read: &mut ReadPanel) -> Option<EditBuildLook> {
    // The OVERALL tab, as it shows in a raid: its clothing panel
    // on where the options' `CLOTHING` is (the game's only out of
    // one, `ShowClothingPanel`: a departure of the app's), its lists'
    // scroll areas as high as they show of their lines (at most
    // `_maxVisibleHeight`: the install's suits are many more
    // than that holds, `DropDownBox.Update`), its back button
    // and its lines' template never (the inventory's own back
    // button is the tab bar's; the lines are drawn from the
    // template, `line`), its lists, their arrows and the
    // loading spinner as its scripts have them.
    if let Some(mut overall) = overall_panel(install, file, clothing) {
        let panel = &overall.panel;
        let never: Vec<usize> = ["BackButton"].into_iter().chain(OVERALL_SELECTORS.map(|selector| format!("{selector}/Element")).iter().map(String::as_str).collect::<Vec<_>>()).filter_map(|path| panel.find(path)).collect();
        let mut scripted: Vec<usize> = OVERALL_SELECTORS.iter().flat_map(|selector| ["Open Panel", "Button/Closed Image", "Button/Open Image"].map(|end| format!("{selector}/{end}"))).filter_map(|path| panel.find(&path)).collect();
        scripted.extend(panel.find(&format!("{OVERALL_VIEW}/Progress Spinner")));
        look.overall_drawn.shown = shown_of(&panel.nodes, &scripted, &never, &scripted);
        look.overall_drawn.pictures = read(panel, &look.overall_drawn.shown, &mut look.textures);
        // The experience's icon as large as its sprite in its row (an
        // image's own preferred size, `Image.preferredWidth`, which
        // the layout here does not ask of it).
        if let Some(icon) = overall.panel.find(OVERALL_EXPERIENCE_ICON) {
            if let Some(size) = look.overall_drawn.pictures.get(icon).copied().flatten().map(|picture| picture.size) {
                let element = overall.panel.nodes[icon].element.get_or_insert(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
                element.preferred = size;
            }
        }
        if let Some(line) = &overall.line {
            look.line_drawn.shown = shown_of(&line.nodes, &[], &[], &[]);
            look.line_drawn.pictures = read(line, &look.line_drawn.shown, &mut look.textures);
        }
        look.overall = Some(overall);
    }
    // The edit build screen, with the workbench on.
    let mut edit_build = None;
    if workbench {
        match edit_build_panels(install) {
            Ok((scene, slot, empty, part)) => {
                let panel = &scene.panel;
                let find = |path: &str| panel.find(path);
                let turned: Vec<usize> = [EB_DROPDOWN, EB_READY].iter().filter_map(|path| find(path)).collect();
                let never: Vec<usize> = EB_NEVER.iter().filter_map(|path| find(path)).collect();
                let mut scripted = turned.clone();
                scripted.extend(find(EB_LOADER));
                scripted.extend((0..panel.nodes.len()).filter(|&index| panel.nodes[index].path.starts_with("Toggle Group/") && panel.nodes[index].path.ends_with("/Checkmark/Mark")));
                let mut screen = Drawn { shown: shown_of(&panel.nodes, &turned, &never, &scripted), pictures: Vec::new() };
                screen.pictures = read(panel, &screen.shown, &mut look.textures);
                let slot_turned: Vec<usize> = [EB_SLOT_BUTTON].iter().filter_map(|path| slot.find(path)).collect();
                let slot_scripted: Vec<usize> = [EB_SLOT_BUTTON, EB_SLOT_NAME, EB_SLOT_FADE].iter().filter_map(|path| slot.find(path)).collect();
                let mut slot_drawn = Drawn { shown: shown_of(&slot.nodes, &slot_turned, &[], &slot_scripted), pictures: Vec::new() };
                slot_drawn.pictures = read(&slot, &slot_drawn.shown, &mut look.textures);
                let mut empty_drawn = Drawn { shown: shown_of(&empty.nodes, &[], &[], &[]), pictures: Vec::new() };
                empty_drawn.pictures = read(&empty, &empty_drawn.shown, &mut look.textures);
                // A part's view as the grid's is read: what its
                // scripts turn on for a part.
                let named = |name: &str| (0..part.nodes.len()).filter(|&index| part.nodes[index].name == name).collect::<Vec<_>>();
                let part_turned: Vec<usize> = ["ModIconContainer", "Loader"].iter().flat_map(|name| named(name)).collect();
                let mut part_drawn = Drawn { shown: shown_of(&part.nodes, &part_turned, &[], &[]), pictures: Vec::new() };
                part_drawn.pictures = read(&part, &part_drawn.shown, &mut look.textures);
                // The characteristics panel: its templates and
                // the mastering row never drawn in it, the
                // expanded rows and the INFO button by the
                // fold, each bar's comparison by its figures.
                let stats = panel.find("CharacteristicsPanel").and_then(|top| {
                    let sub = panel.subtree(top);
                    let find = |path: &str| sub.find(path);
                    let template = |path: &str| find(path).map(|at| sub.subtree(at));
                    let (bar, compact) = (template(EB_CHAR_TEMPLATE)?, template(EB_COMPACT_TEMPLATE)?);
                    let never: Vec<usize> = [EB_CHAR_TEMPLATE, EB_COMPACT_TEMPLATE, "CharacteristicsExpanded/Mastering"].iter().filter_map(|path| find(path)).collect();
                    let scripted: Vec<usize> = ["CharacteristicsExpanded", "InfoButton", "CharacteristicsExpanded/Durability/bars/CompareDelta", "CharacteristicsExpanded/Weight/bars/CompareDelta"].iter().filter_map(|path| find(path)).collect();
                    let mut drawn = Drawn { shown: shown_of(&sub.nodes, &scripted, &never, &scripted), pictures: Vec::new() };
                    drawn.pictures = read(&sub, &drawn.shown, &mut look.textures);
                    let mut row_drawn = |row: &trp_data::interface::screen::ScenePanel| {
                        let turned: Vec<usize> = row.find("Value").into_iter().collect();
                        let scripted: Vec<usize> = row.find("bars/CompareDelta").into_iter().collect();
                        let mut drawn = Drawn { shown: shown_of(&row.nodes, &turned, &[], &scripted), pictures: Vec::new() };
                        drawn.pictures = read(row, &drawn.shown, &mut look.textures);
                        drawn
                    };
                    let (bar_drawn, compact_drawn) = (row_drawn(&bar), row_drawn(&compact));
                    let bar_look = trp_data::interface::screen::row_look(&bar, "CharacteristicPanel").unwrap_or_default();
                    let compact_look = trp_data::interface::screen::row_look(&compact, "CompactCharacteristicPanel").unwrap_or_default();
                    let weight_look = find("CharacteristicsExpanded/Weight").and_then(|row| trp_data::interface::screen::row_look(&sub.subtree(row), "CharacteristicPanel")).unwrap_or_else(|| bar_look.clone());
                    Some(EbStatsLook { panel: sub, drawn, bar: (bar, bar_drawn), compact: (compact, compact_drawn), bar_look, compact_look, weight_look })
                });
                edit_build = Some(EditBuildLook {
                    scene: std::sync::Arc::new(scene),
                    screen,
                    slot,
                    slot_drawn,
                    empty,
                    empty_drawn,
                    part: Some((part, part_drawn)),
                    part_look: None,
                    bones: HashMap::new(),
                    dropdown: [None, None],
                    stats,
                });
            }
            Err(e) => eprintln!("the game's edit build screen could not be read; the app's look of it is drawn: {e:#}"),
        }
    }
    edit_build
}
