//! The screens' fonts, the tag window's asked sizes, and the panels' animation clips.

use super::*;

/// The fonts every panel's texts are drawn in, the tag window's texts' preferred sizes, and the clips the
/// panels' animators play.
pub(super) fn read_fonts_and_clips(look: &mut HealthLook, install: &trp_data::Install, panel: &trp_data::interface::screen::HealthPanel, reader: &mut PictureReader, mut fonts_read: &mut FontsRead) {
    // The fonts its texts are drawn in: each font asset's own
    // font, kept whole in the build's file beside it.
    let items = look.items.as_ref().map_or([None, None, None], |items| [items.slot.as_ref(), items.grid.as_ref(), items.quick.as_ref()]);
    let containers = look.containers.as_ref().map(|views| &views.panel);
    let tabs = look.tab_bar.as_ref().map(|bar| &bar.panel);
    let stash = look.stash.as_ref().map(|stash| &stash.panel);
    let quick = look.quick.as_ref().map(|quick| &quick.panel);
    let back = look.tab_bar.as_ref().and_then(|bar| bar.back.as_ref());
    let battle = look.battle.as_ref().map_or([None, None], |views| [Some(&views.stance), Some(&views.health)]);
    let battle_quick = look.battle_quick.as_ref().map(|quick| &quick.panel);
    let info = look.info.as_ref();
    let menu = look.menu.as_ref();
    let tag_window = look.tag_window.as_ref();
    let submenu = look.submenu.as_ref();
    let overall = look.overall.as_ref().map(|overall| &overall.panel);
    let overall_line = look.overall.as_ref().and_then(|overall| overall.line.as_ref());
    let edit_build = look.edit_build.as_ref().map_or([None, None, None, None], |eb| [Some(&eb.scene.panel), Some(&eb.slot), Some(&eb.empty), eb.part.as_ref().map(|(part, _)| part)]);
    let info_rows = look.info_rows.as_ref().map_or([None, None, None, None], |rows| [Some(&rows.bar.0), Some(&rows.compact.0), Some(&rows.dropdown.0), rows.mod_slot.as_ref().map(|(panel, _)| panel)]);
    let action = look.battle.as_ref().and_then(|views| views.action.as_ref()).map(|action| &action.panel);
    let grid_window = look.grid_window.as_ref().map(|window| &window.panel);
    let repair_window = look.repair.as_ref();
    let split_window = look.split.as_ref().map(|split| &split.panel);
    let using = look.using.as_ref().map(|using| &using.panel);
    let version = look.version.as_ref().map(|(label, _)| label).or(look.tooltip.as_ref());
    // (The tooltip of lines is in the same font asset.)
    let tooltip = look.tooltip.as_ref();
    let skill_panels: Vec<&trp_data::interface::screen::ScenePanel> = look.skills.as_ref().map(|skills| skills.panels()).unwrap_or_default();
    let assets: std::collections::BTreeSet<(String, i64)> = [Some(&panel.panel), panel.strip.as_ref(), panel.foot.as_ref(), panel.gear_panel.as_ref(), items[0], items[1], items[2], containers, tabs, stash, quick, back, version, tooltip, battle[0], battle[1], battle_quick, info, menu, tag_window, submenu, overall, overall_line, edit_build[0], edit_build[1], edit_build[2], edit_build[3], info_rows[0], info_rows[1], info_rows[2], info_rows[3], action, grid_window, repair_window, split_window, using]
        .into_iter()
        .flatten()
        .chain(skill_panels.iter().copied())
        .flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| Some((scene.file_of(text.font.0)?, text.font.1)))))
        .collect();
    // The materials the texts name for themselves, where they do.
    let text_materials: std::collections::BTreeSet<(String, i64)> = [Some(&panel.panel), panel.strip.as_ref(), panel.foot.as_ref(), panel.gear_panel.as_ref(), items[0], items[1], items[2], containers, tabs, stash, quick, back, version, tooltip, battle[0], battle[1], battle_quick, info, menu, tag_window, submenu, overall, overall_line, edit_build[0], edit_build[1], edit_build[2], edit_build[3], info_rows[0], info_rows[1], info_rows[2], info_rows[3], action, grid_window, repair_window, split_window, using]
        .into_iter()
        .flatten()
        .chain(skill_panels.iter().copied())
        .flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| text.material).and_then(|(file, id)| Some((scene.file_of(file)?, id)))))
        .collect();
    look.read_fonts(install, &mut *reader.pictures, (assets, text_materials), &mut fonts_read);
    // What the tag window's texts and its grid of colours ask of
    // its layout (TextMeshPro's preferred size; the grid's one
    // row), which the layout here does not ask of them itself.
    if let Some(mut panel) = look.tag_window.take() {
        let asks = |panel: &trp_data::interface::screen::ScenePanel, index: usize, words: &str| -> Option<[f32; 2]> {
            let text = panel.nodes[index].text.as_ref()?;
            let font = &look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1))?.font;
            let width = font.width(words, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
            Some([width, tmp_line(font, text.size) + text.margin[1] + text.margin[3]])
        };
        let element = |preferred: [f32; 2]| trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred, flexible: [-1.0, -1.0] };
        if let Some(colors) = panel.find(TAG_COLORS_PANEL) {
            let width = panel.nodes[colors].element.map_or(360.0, |element| element.preferred[0]);
            panel.nodes[colors].element = Some(trp_data::interface::screen::Element { preferred: [width, TAG_CELL], ..element([width, TAG_CELL]) });
        }
        let counter = format!("{0}/{0}", look.tag_limit);
        if let Some((index, size)) = panel.find(TAG_COUNT).and_then(|index| Some((index, asks(&panel, index, &counter)?))) {
            panel.nodes[index].element = Some(element(size));
        }
        if let Some((index, size)) = panel.find(TAG_SAVE_SIZE).and_then(|index| Some((index, asks(&panel, index, "SAVE")?))) {
            panel.nodes[index].element = Some(element(size));
        }
        look.tag_window = Some(panel);
    }
    // The scene's clips that the panels' animators play.
    let shared: Vec<String> = panel.panel.externals.iter().map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).filter(|name| name.starts_with("sharedassets")).collect();
    match trp_data::interface::screen::scene_clips(install, &mut *reader.pictures, &shared) {
        Ok(clips) => {
            use trp_data::unity::anim::{path_hash, Clip};
            let curve = |clip: &Clip, path: &str, property: &str| clip.others.iter().find(|b| b.path == path_hash(path) && b.attribute == path_hash(property)).map(|b| b.curve);
            for (_, _, clip) in &clips {
                if let (Some(alpha), true) = (curve(clip, "BackGlowImage", "m_Color.a"), clip.name == "Normal") {
                    let mut values = Vec::new();
                    clip.sample(0.0, &mut values);
                    look.glow_alpha = values.get(alpha).copied();
                }
                if let (Some(alpha), true) = (curve(clip, "BackGlowImage", "m_Color.a"), clip.name == "Highlighted") {
                    look.glow_lit = Some((clip.clone(), alpha));
                }
                if let (Some(height), Some(alpha), true) = (curve(clip, "", "m_AnchoredPosition.y"), curve(clip, "", "m_Color.a"), clip.name == "BuffPaneActive") {
                    look.bob = Some((clip.clone(), height, alpha));
                }
            }
        }
        Err(e) => eprintln!("the health panel's animations could not be read; its glows show and its arrows stand still: {e:#}"),
    }
}
