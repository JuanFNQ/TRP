//! Reading the game's screens into HealthLook.

use super::*;

mod fonts_clips;
mod inventory_panels;
mod overall_edit_build;
mod sprites_read;
mod windows_read;

/// What reads a panel's pictures: the panel, which of its objects are shown, and the textures kept.
type ReadPanel<'r> = dyn FnMut(&trp_data::interface::screen::ScenePanel, &[Shown], &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Vec<Option<Picture>> + 'r;

impl HealthLook {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn read_screens(install: &trp_data::Install, words: &HashMap<String, String>, file: &str, pictures: bool, weight_limits: Option<[f32; 2]>, layouts: &[String], clothing: bool, workbench: bool, start: impl FnOnce(Self)) -> Self {
        let mut look = Self { tarkov_ui: pictures, weight_limits, ..Self::default() };
        // The pictures' reader, the same for both parts.
        let mut scene_pictures = match pictures.then(|| trp_data::interface::screen::ScenePictures::new(install)) {
            Some(Ok(scene_pictures)) => Some(scene_pictures),
            Some(Err(e)) => {
                eprintln!("the health panel's pictures could not be read; it is drawn without them: {e:#}");
                None
            }
            None => None,
        };
        let mut reader = scene_pictures.as_mut().map(|pictures| PictureReader { pictures, textures: HashMap::new(), missing: Vec::new() });
        let mut fonts_read = FontsRead::default();
        if let Some(reader) = reader.as_mut() {
            look.read_battle_part(install, words, file, reader, &mut fonts_read);
        }
        // The pictures the start makes (`make_pictures`), not made again.
        let started = look.textures.len();
        start(look.split_battle_part());
        let panel = match trp_data::interface::screen::HealthPanel::load(install, file) {
            Ok(panel) => panel,
            Err(e) => {
                eprintln!("the install's health panel could not be read; the HEALTH tab shows none of it: {e:#}");
                return look;
            }
        };
        look.health.shown = health_shown(&panel);
        if let Some(strip) = &panel.strip {
            let scripted: Vec<usize> = panel.strip_figures.iter().filter_map(|(_, f)| f.as_ref()).flat_map(|f| [f.ok_glow, f.glow, f.up, f.down, f.rate]).flatten().collect();
            look.strip.shown = shown_of(&strip.nodes, &[], &[], &scripted);
        }
        if let Some(foot) = &panel.foot {
            look.foot.shown = shown_of(&foot.nodes, &[], &[], &[]);
        }
        if let Some(border) = &panel.border {
            look.border.shown = shown_of(&border.nodes, &[], &[], &[]);
        }
        if let Some(gear) = &panel.gear_panel {
            // Each slot's borders, fill and picture as it is full or empty
            // (`SlotView.SetSlotGraphics`); its header's arrow only out of
            // a raid (`SlotViewHeader.Show`, the header not clickable).
            let scripted: Vec<usize> = panel.slot_views.iter().flat_map(|(_, p)| [p.full_border, p.back_image, p.background, p.empty_border]).flatten().collect();
            let arrows: Vec<usize> = panel.slot_views.iter().filter_map(|(_, p)| gear.find(&format!("{}/ArrowHolder", gear.nodes[p.header?].path))).collect();
            look.gear.shown = shown_of(&gear.nodes, &[], &arrows, &scripted);
        }
        if !pictures {
            // The OVERALL tab's layout, where the app's own look puts its
            // boxes.
            look.overall = overall_panel(install, file, clothing);
            look.panel = Some(panel);
            return look;
        }
        let Some(mut reader) = reader else {
            look.panel = Some(panel);
            return look;
        };
        let mut read = |scene: &trp_data::interface::screen::ScenePanel, shown: &[Shown], kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>| reader.panel(scene, shown, file, kept);
        inventory_panels::read_inventory_panels(&mut look, install, file, layouts, &panel, &mut read);
        windows_read::read_windows(&mut look, install, &mut read);
        let edit_build = overall_edit_build::read_overall_and_edit_build(&mut look, install, file, clothing, workbench, &mut read);
        drop(read);
        sprites_read::read_sprites(&mut look, install, file, &mut reader, edit_build);
        fonts_clips::read_fonts_and_clips(&mut look, install, &panel, &mut reader, &mut fonts_read);
        for texture in look.textures.iter_mut().take(started) {
            *texture = None;
        }
        look.panel = Some(panel);
        look
    }
}
