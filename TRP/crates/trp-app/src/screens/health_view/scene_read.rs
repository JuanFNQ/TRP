//! Reading the scene, and the battle part split, taken and rejoined.

use super::*;

impl HealthLook {
    /// Reads the panels from the scene file that holds the inventory
    /// screen (`file`), and with `TARKOV UI` on (`pictures`) the pictures
    /// their objects draw and the clips their animators play; off, the
    /// health panel's layout alone, where the app's own look puts its
    /// boxes. In two parts: the
    /// battle screen's (`read_battle_part`) is handed to `start` as soon as
    /// it is read, for the first frame; the rest is read on with the same
    /// reader, so that its pictures' places go on from the first part's and
    /// no font is read twice, and returned, to be merged in where it comes
    /// (`merge_rest`, `ScreensComing`). The battle screen's part does not
    /// hang on the health panel being read (a health panel that could not
    /// be read is not to leave the battle screen's panels unread too).
    /// And with the rest, whatever the options, the inspect window's own
    /// preview (`inspect`), which its three users each read on the frame
    /// they first wanted it (the OVERALL tab's camera made in the warm-up,
    /// 7 to 70 ms; the first inspect window; the first edit build screen).
    #[allow(clippy::too_many_arguments)]
    pub fn read(install: &trp_data::Install, words: &HashMap<String, String>, file: &str, pictures: bool, weight_limits: Option<[f32; 2]>, layouts: &[String], clothing: bool, workbench: bool, start: impl FnOnce(Self)) -> Self {
        let mut look = Self::read_screens(install, words, file, pictures, weight_limits, layouts, clothing, workbench, start);
        look.inspect = match trp_data::interface::edit_build::InspectPreview::load(install) {
            Ok(inspect) => Some(std::sync::Arc::new(inspect)),
            Err(e) => {
                eprintln!("the inspect window's own preview could not be read; the OVERALL tab's character keeps the gear page's light, the inspect window and the edit build screen their scene's: {e:#}");
                None
            }
        };
        look
    }

    /// The battle screen's part of the screens (`read`): what the first
    /// frame may show (the stance's and the health's panels in a raid, the
    /// quick bar, the interaction's pointer and the mount's, the effects'
    /// icons, the examine's panel, the version line) and the inventory's bar
    /// of tabs, which the screen is laid out by as the run begins
    /// (`inventory_view::spawn`); and the fonts of their texts.
    pub(super) fn read_battle_part(&mut self, install: &trp_data::Install, words: &HashMap<String, String>, file: &str, reader: &mut PictureReader, fonts_read: &mut FontsRead) {
        let mut read = |scene: &trp_data::interface::screen::ScenePanel, shown: &[Shown], kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>| reader.panel(scene, shown, file, kept);
        // The bar of tabs, its tabs laid out at the width the game
        // works out from their words in their fonts.
        match trp_data::interface::screen::TabBar::load(install, file) {
            Ok(mut bar) => {
                let keys: Vec<&str> = bar.tabs.iter().map(|tab| tab.key.as_str()).collect();
                let texts = trp_data::character::inventory::words_of(words, &keys);
                let say = |key: &str| texts.get(&key.to_lowercase()).cloned();
                let width = bar.width(install, &say);
                bar.lay_out(width);
                self.tab_words = bar.tabs.iter().map(|tab| trp_data::interface::screen::TabBar::words(tab, &say)).collect();
                self.tab_width = width;
                // Each tab's versions shown as it is chosen and as
                // the mouse is on it; its words' samples, which
                // only size it, never.
                let turned: Vec<usize> = bar.tabs.iter().flat_map(|tab| [tab.selected, tab.hover]).flatten().collect();
                let scripted: Vec<usize> = bar.tabs.iter().flat_map(|tab| [tab.normal, tab.selected, tab.hover, tab.idle]).flatten().collect();
                // The prestige tab's mark of a level ready to finish
                // (`NewState`, the screen's `_prestigeNewLevelIcon`):
                // off unless the next level's task can be finished and
                // has not been seen (`PrestigeLevelAvailabilityChanged`),
                // which a character with no prestige never has.
                let mut never: Vec<usize> = bar.tabs.iter().flat_map(|tab| tab.size_definers.clone()).collect();
                never.extend((0..bar.panel.nodes.len()).filter(|&index| bar.panel.nodes[index].name == "NewState"));
                self.tabs_drawn.shown = shown_of(&bar.panel.nodes, &turned, &never, &scripted);
                self.tabs_drawn.pictures = read(&bar.panel, &self.tabs_drawn.shown, &mut self.textures);
                // The back button at rest (`DefaultUIButtonAnimation`,
                // normal: its ground clear, its words in their own
                // colour), as the scene keeps it.
                if let Some(back) = &bar.back {
                    self.back_drawn.shown = shown_of(&back.nodes, &[], &[], &[]);
                    self.back_drawn.pictures = read(back, &self.back_drawn.shown, &mut self.textures);
                }
                self.tab_bar = Some(bar);
            }
            Err(e) => eprintln!("the game's bar of tabs could not be read; the app's tabs are drawn: {e:#}"),
        }
        // The battle screen's panels of the character: the stance's
        // figures shown as its script has them; the speed's notch
        // made by the script (`battle_view`); the effects' icon
        // made for each effect.
        self.battle_modes = crate::panels::battle_view::BattleModes::read(install);
        match trp_data::interface::screen::BattleViews::load(install, file) {
            Ok(views) => {
                if let Some(why) = &views.action_missing {
                    eprintln!("the game's interaction panel could not be read; the app's prompt is drawn: {why}");
                }
                // (And the mount's icon and arrows, which its script
                // turns on: `battle_view`.)
                let figures: Vec<usize> = views.figures.iter().map(|(object, _)| *object).chain(views.mount.iter().flat_map(|mount| mount.scripted())).collect();
                self.stance_drawn.shown = shown_of(&views.stance.nodes, &[], &[], &figures);
                self.stance_drawn.pictures = read(&views.stance, &self.stance_drawn.shown, &mut self.textures);
                self.body_drawn.shown = shown_of(&views.health.nodes, &[], &[], &[]);
                self.body_drawn.pictures = read(&views.health, &self.body_drawn.shown, &mut self.textures);
                self.battle = Some(views);
            }
            Err(e) => eprintln!("the game's panels of the stance and of the health in a raid could not be read; the app's meters are drawn: {e:#}"),
        }
        match trp_data::interface::screen::QuickViews::load_battle(install, file) {
            Ok(quick) => {
                // The magazine picker's cancel and every view's arrow, off in
                // the scene, read to be shown by their scripts
                // (`battle_view::draw_picker`).
                let picker = quick.ammo_selector.as_ref();
                let cancel: Vec<usize> = picker
                    .map(|picker| {
                        (0..quick.panel.nodes.len())
                            .filter(|&index| {
                                let mut above = Some(index);
                                while let Some(at) = above {
                                    if at == picker.cancel {
                                        return true;
                                    }
                                    above = quick.panel.nodes[at].parent;
                                }
                                false
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let scripted: Vec<usize> = cancel.into_iter().chain(quick.slots.iter().filter_map(|slot| slot.arrow)).collect();
                let turned_on: Vec<usize> = picker.map(|picker| picker.object).into_iter().collect();
                self.battle_quick_drawn.shown = shown_of(&quick.panel.nodes, &turned_on, &[], &scripted);
                self.battle_quick_drawn.pictures = read(&quick.panel, &self.battle_quick_drawn.shown, &mut self.textures);
                self.battle_quick = Some(quick);
            }
            Err(e) => eprintln!("the game's quick bar in a raid could not be read; none is drawn: {e:#}"),
        }
        // The options tab's icons, the game's GAME SETTINGS button's.
        let icons = self.tab_bar.as_ref().map_or([None, None], |bar| bar.settings_icons.clone());
        for (at, icon) in icons.into_iter().enumerate() {
            if let Some((file, id)) = icon {
                self.options_icons[at] = reader.sprite(&file, id, &mut self.textures);
            }
        }
        // The tabs' fills cut to their masks, from the pictures' pixels.
        if let Some(bar) = &self.tab_bar {
            self.tab_fills = masked_fills(&bar.panel, &self.tabs_drawn, &self.textures, &mut HashMap::new());
        }
        // The sprites the stance panel's script puts on it.
        if let Some(views) = &self.battle {
            let wanted: Vec<(i32, i64)> = views.fill_sprites.iter().flatten().chain(&views.noise_sprites).copied().collect();
            for (outside, id) in wanted {
                let Some(name) = views.stance.file_of(outside) else { continue };
                if let Some(picture) = reader.sprite(&name, id, &mut self.textures) {
                    self.battle_sprites.insert((outside, id), picture);
                }
            }
            // The interaction panel's pointer: the hand, the hand
            // refused, the sense's dot (`UIPointer`).
            if let Some(action) = &views.action {
                for (outside, id) in [action.hover_sprite, action.unavailable_sprite, action.sense_sprite].into_iter().flatten() {
                    let Some(name) = action.panel.file_of(outside) else { continue };
                    if let Some(picture) = reader.sprite(&name, id, &mut self.textures) {
                        self.battle_sprites.insert((outside, id), picture);
                    }
                }
            }
        }
        // The game scene's panel of an examine, shown as its script
        // shows it (`ShowGameObject`: the scene keeps it off).
        match trp_data::interface::screen::using_panel(install) {
            Ok(using) => {
                self.using_drawn.shown = shown_of(&using.panel.nodes, &[0], &[], &[]);
                self.using_drawn.pictures = reader.panel(&using.panel, &self.using_drawn.shown, &using.panel.file, &mut self.textures);
                self.using = Some(using);
            }
            Err(e) => eprintln!("the game's panel of an examine could not be read; none is drawn: {e:#}"),
        }
        // The health's effects' icons (`StaticIcons.EffectIcons`).
        match trp_data::interface::screen::effect_icons(install) {
            Ok(icons) => {
                for (name, (file, id)) in icons {
                    if let Some(picture) = reader.sprite(&file, id, &mut self.textures) {
                        self.effect_pictures.insert(name, picture);
                    }
                }
            }
            Err(e) => eprintln!("the game's icons of the health's effects could not be read; the effects are written in words: {e:#}"),
        }
        match trp_data::interface::screen::version_label(install) {
            Ok(version) => self.version = Some(version),
            Err(e) => eprintln!("the game's line of its version could not be read; none is drawn: {e:#}"),
        }
        // Their fonts.
        let battle = self.battle.as_ref().map_or([None, None], |views| [Some(&views.stance), Some(&views.health)]);
        let battle_quick = self.battle_quick.as_ref().map(|quick| &quick.panel);
        let action = self.battle.as_ref().and_then(|views| views.action.as_ref()).map(|action| &action.panel);
        let using = self.using.as_ref().map(|using| &using.panel);
        let version = self.version.as_ref().map(|(label, _)| label);
        let tabs = self.tab_bar.as_ref().map(|bar| &bar.panel);
        let back = self.tab_bar.as_ref().and_then(|bar| bar.back.as_ref());
        let panels: Vec<&trp_data::interface::screen::ScenePanel> = [battle[0], battle[1], battle_quick, action, using, version, tabs, back].into_iter().flatten().collect();
        let sets = font_assets(&panels);
        self.read_fonts(install, &mut *reader.pictures, sets, fonts_read);
    }

    /// The battle screen's part read (`read_battle_part`), for the start:
    /// its fields moved out, but for what the rest's read goes on to want,
    /// kept as copies (the pictures read so far, which the rest's places go
    /// on from and its fills may cut; the fonts' faces, names and
    /// materials).
    pub(super) fn split_battle_part(&mut self) -> Self {
        Self {
            tarkov_ui: self.tarkov_ui,
            weight_limits: self.weight_limits,
            version: self.version.take(),
            using: self.using.take(),
            using_drawn: std::mem::take(&mut self.using_drawn),
            effect_pictures: std::mem::take(&mut self.effect_pictures),
            battle: self.battle.take(),
            stance_drawn: std::mem::take(&mut self.stance_drawn),
            body_drawn: std::mem::take(&mut self.body_drawn),
            battle_quick: self.battle_quick.take(),
            battle_quick_drawn: std::mem::take(&mut self.battle_quick_drawn),
            battle_sprites: std::mem::take(&mut self.battle_sprites),
            battle_modes: std::mem::take(&mut self.battle_modes),
            tab_bar: self.tab_bar.take(),
            tabs_drawn: std::mem::take(&mut self.tabs_drawn),
            back_drawn: std::mem::take(&mut self.back_drawn),
            tab_words: std::mem::take(&mut self.tab_words),
            tab_width: self.tab_width,
            tab_fills: std::mem::take(&mut self.tab_fills),
            options_icons: std::mem::take(&mut self.options_icons),
            textures: self.textures.clone(),
            font_data: std::mem::take(&mut self.font_data),
            font_names: self.font_names.clone(),
            bold_fonts: self.bold_fonts.clone(),
            text_effects: self.text_effects.clone(),
            tmp_faces: self.tmp_faces.clone(),
            tmp_materials: self.tmp_materials.clone(),
            tmp_default_material: self.tmp_default_material.clone(),
            ..Self::default()
        }
    }

    /// The battle screen's part put back where the read was not split over
    /// a thread (`read` on the frame's thread): as one read, made into
    /// pictures and fonts at the start (`make_pictures`).
    pub fn rejoin(&mut self, mut start: Self) {
        for (at, texture) in std::mem::take(&mut start.textures).into_iter().enumerate() {
            if let Some(kept) = self.textures.get_mut(at) {
                *kept = texture;
            }
        }
        self.take_battle_part(start);
    }

    /// The rest of the screens come (`ScreensComing`): its fields, with the
    /// battle screen's part kept as the start made it (its pictures and
    /// fonts already on the graphics card) and the rest's pictures and fonts
    /// made now. What the start set on the look (the skills' figures) is
    /// kept.
    pub fn merge_rest(&mut self, mut rest: Self, images: &mut Assets<Image>, fonts: &mut Assets<Font>) {
        let made_images = std::mem::take(&mut self.images);
        let made_fonts = std::mem::take(&mut self.fonts);
        rest.level_experience = std::mem::take(&mut self.level_experience);
        rest.skill_effectiveness = self.skill_effectiveness.take();
        let start = std::mem::take(self);
        rest.take_battle_part(start);
        rest.fonts = made_fonts;
        make_fonts(&mut rest, fonts);
        let started = made_images.len();
        let textures = std::mem::take(&mut rest.textures);
        rest.images = made_images;
        rest.images.extend(textures.into_iter().skip(started).map(|texture| make_image(texture, images)));
        finish_looks(&mut rest);
        *self = rest;
    }

    /// The battle screen's part's fields, moved in from the look the start
    /// had (`split_battle_part`'s): those the rest's read left empty.
    fn take_battle_part(&mut self, mut start: Self) {
        self.version = start.version.take();
        self.using = start.using.take();
        self.using_drawn = std::mem::take(&mut start.using_drawn);
        self.effect_pictures = std::mem::take(&mut start.effect_pictures);
        self.battle = start.battle.take();
        self.stance_drawn = std::mem::take(&mut start.stance_drawn);
        self.body_drawn = std::mem::take(&mut start.body_drawn);
        self.battle_quick = start.battle_quick.take();
        self.battle_quick_drawn = std::mem::take(&mut start.battle_quick_drawn);
        self.battle_sprites = std::mem::take(&mut start.battle_sprites);
        self.battle_modes = std::mem::take(&mut start.battle_modes);
        self.tab_bar = start.tab_bar.take();
        self.tabs_drawn = std::mem::take(&mut start.tabs_drawn);
        self.back_drawn = std::mem::take(&mut start.back_drawn);
        self.tab_words = std::mem::take(&mut start.tab_words);
        self.tab_width = start.tab_width;
        self.tab_fills = std::mem::take(&mut start.tab_fills);
        self.options_icons = std::mem::take(&mut start.options_icons);
        // (The fonts' maps the rest's read kept copies of and went on with.)
        let mut font_data = std::mem::take(&mut start.font_data);
        font_data.append(&mut self.font_data);
        self.font_data = font_data;
    }
}
