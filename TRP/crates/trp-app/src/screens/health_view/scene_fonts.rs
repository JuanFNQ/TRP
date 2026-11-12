//! The scene's fonts, keys, sprites and finds by content.

use super::*;

impl HealthLook {
    /// The fonts the texts of these font assets are drawn in (each asset's
    /// own font, kept whole in the build's file beside it), their faces as
    /// the game draws from them, their bold typefaces, effects and
    /// materials: those not read already (`FontsRead`).
    pub(super) fn read_fonts(&mut self, install: &trp_data::Install, pictures: &mut trp_data::interface::screen::ScenePictures, (mut assets, text_materials): FontAssets, fonts_read: &mut FontsRead) {
        assets.retain(|asset| !fonts_read.tried.contains(asset));
        // And each one's bold typeface, where it has one.
        for (name, id) in assets.clone() {
            if let Some(bold) = trp_data::interface::screen::TmpFont::read(install, &name, id).ok().and_then(|font| font.bold) {
                self.bold_fonts.insert((name, id), bold.clone());
                if !fonts_read.tried.contains(&bold) {
                    assets.insert(bold);
                }
            }
        }
        fonts_read.tried.extend(assets.iter().cloned());
        for (name, id) in &assets {
            if let Some(effect) = TextEffect::of(install, name, *id) {
                self.text_effects.insert((name.clone(), *id), effect);
            }
        }
        let kept_fonts = &mut fonts_read.kept;
        let mut tmp_built: HashMap<(String, i64), trp_data::interface::tmp_text::TmpFace> = HashMap::new();
        for (name, id) in assets {
            if !kept_fonts.contains_key(&name) {
                let read = trp_data::interface::screen::build_fonts(install, &name).unwrap_or_else(|e| {
                    eprintln!("the game's fonts could not be read from {name}; the app's is used: {e:#}");
                    Vec::new()
                });
                kept_fonts.insert(name.clone(), read);
            }
            let asset = trp_data::interface::screen::script_name(install, &name, id).unwrap_or_default();
            if !asset.is_empty() {
                self.font_names.insert((name.clone(), id), asset.clone());
            }
            let by_face =|| trp_data::interface::screen::TmpFont::read(install, &name, id).ok().and_then(|face| trp_data::interface::screen::font_by_face(&face.family, &face.style, &kept_fonts[&name]));
            let font_file = trp_data::interface::screen::font_for(&asset, &kept_fonts[&name]).or_else(by_face).map(|(_, font)| font.clone());
            match &font_file {
                Some(font) => self.font_data.push(((name.clone(), id), font.clone())),
                None => eprintln!("no font of the game's is had for its font asset {asset:?}; the app's is used"),
            }
            // The asset as the game draws from it: its glyphs, its
            // atlas, its font's kerning, its own material.
            let face = trp_data::interface::screen::TmpFont::read(install, &name, id).and_then(|font| {
                let (atlas_file, atlas_id) = anyhow::Context::context(font.atlases.first().cloned(), "the font asset has no atlas")?;
                let atlas = pictures.texture(&atlas_file, atlas_id)?;
                Ok(trp_data::interface::tmp_text::TmpFace::new(font, &atlas, font_file.as_deref()))
            });
            match face {
                Ok(face) => {
                    tmp_built.insert((name.clone(), id), face);
                }
                Err(e) => eprintln!("the font asset {asset:?} could not be read to draw from; its texts are drawn the app's way: {e:#}"),
            }
            if let Ok(material) = trp_data::interface::screen::font_material(install, &name, id) {
                self.tmp_default_material.insert((name.clone(), id), material);
            }
        }
        // Each asset with its bold typeface's face, where it has one.
        let bolds: std::collections::BTreeSet<(String, i64)> = tmp_built.values().filter_map(|face| face.font.bold.clone()).collect();
        for key in bolds {
            if let Some(face) = tmp_built.remove(&key) {
                self.tmp_faces.insert(key, std::sync::Arc::new(face));
            }
        }
        for (key, mut face) in tmp_built {
            face.bold = face.font.bold.as_ref().and_then(|bold| self.tmp_faces.get(bold)).cloned();
            self.tmp_faces.insert(key, std::sync::Arc::new(face));
        }
        // The materials: the assets' own and the texts' own, those not read.
        let wanted: std::collections::BTreeSet<(String, i64)> = self.tmp_default_material.values().cloned().chain(text_materials).filter(|key| fonts_read.materials.insert(key.clone())).collect();
        for (file, id) in wanted {
            match trp_data::interface::screen::material_properties(install, &file, id) {
                Ok(props) => {
                    self.tmp_materials.insert((file, id), trp_data::interface::tmp_text::TmpMaterial::of(&props));
                }
                Err(e) => eprintln!("a text's material could not be read; texts drawn with it are drawn the app's way: {e:#}"),
            }
        }
    }

    /// The panels' own texts that are keys to the install's words: the
    /// health panel's views' headers, and the slots' names, which the slot
    /// views' headers say (`SlotViewHeader.Show`).
    pub fn keys(&self) -> Vec<String> {
        let Some(game) = &self.panel else { return Vec::new() };
        let headers = game.views.parts.iter().filter_map(|view| game.panel.find(&format!("{}/Header/Text", game.panel.nodes[view.view].path))).filter_map(|text| game.panel.nodes[text].text.as_ref().map(|text| text.text.clone()));
        // And the containers' slots', which their rows' headers say.
        let containers = ["TacticalVest", "Pockets", "Backpack", "SecuredContainer"].map(str::to_owned);
        // And what the panel for a container opened calls what it holds.
        let mut keys: Vec<String> = headers.chain(game.slot_views.iter().map(|(slot, _)| slot.to_string())).chain(containers).chain(["LOOT".to_owned()]).collect();
        // And the OVERALL tab's texts the install's words fill.
        keys.extend(self.overall.iter().flat_map(|overall| overall.localized.iter().map(|(_, key, _)| key.clone())));
        // And the SKILLS tab's.
        keys.extend(self.skills.iter().flat_map(|skills| skills.page.localized.iter().map(|(_, key, _)| key.clone())));
        // And the edit build screen's, its slots' and its empty view's.
        if let Some(eb) = &self.edit_build {
            for panel in [&eb.scene.panel, &eb.slot, &eb.empty] {
                keys.extend((0..panel.nodes.len()).filter_map(|index| trp_data::interface::screen::localized(panel, index)).map(|(key, _)| key));
            }
        }
        keys
    }

    /// The edit build screen as the game draws it, where it was read.
    pub(crate) fn edit_build(&self) -> Option<&EditBuildLook> {
        self.edit_build.as_ref()
    }

    /// One of the game's pictures as drawn by hand, in that colour.
    pub(crate) fn sprite_of(&self, picture: Picture, color: Color) -> Option<Sprite> {
        let image = self.images.get(picture.texture)?.clone()?;
        Some(Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color })
    }

    /// A part's kind's mark (`Mod Tactical Types/icon_mod_<kind>`) by its
    /// kind's name.
    pub(crate) fn mod_mark(&self, kind: &str) -> Option<Picture> {
        self.mod_pictures.get(&format!("mod_{kind}")).copied()
    }

    /// What a panel's pictures and texts ask of the layout groups that lay
    /// them out, put on them where no `LayoutElement` of theirs says
    /// (`Image.preferredWidth`: a simple or filled picture its sprite's
    /// size, a sliced or tiled one its border's; TextMeshPro's preferred
    /// size: its words' width in its font asset and a line's height, with
    /// its margins), so that the panel's groups and fitters lay out as the
    /// game's: the layout here asks nothing of either by itself. `words`:
    /// what the texts say where not what the scene has; an object with a
    /// group of its own asks what its group does.
    pub(crate) fn ask_by_content(&self, panel: &mut trp_data::interface::screen::ScenePanel, drawn: &Drawn, words: &HashMap<usize, String>) {
        self.ask_by_content_under(panel, drawn, words, None);
    }

    /// The same, only for what lies under one object of the panel (itself
    /// among it), where one is named.
    pub(crate) fn ask_by_content_under(&self, panel: &mut trp_data::interface::screen::ScenePanel, drawn: &Drawn, words: &HashMap<usize, String>, top: Option<usize>) {
        let under = |panel: &trp_data::interface::screen::ScenePanel, index: usize| {
            let Some(top) = top else { return true };
            let mut at = Some(index);
            while let Some(here) = at {
                if here == top {
                    return true;
                }
                at = panel.nodes[here].parent;
            }
            false
        };
        for index in 0..panel.nodes.len() {
            if panel.nodes[index].group.is_some() || !under(panel, index) {
                continue;
            }
            let mut asked: Option<[f32; 2]> = None;
            if let (Some(image), Some(Some(picture))) = (panel.nodes[index].image, drawn.pictures.get(index)) {
                let per = image.pixels_per_unit.max(1e-3);
                let [l, b, r, t] = picture.border;
                asked = Some(match image.kind {
                    1 | 2 => [(l + r) / per, (b + t) / per],
                    _ => [picture.size[0] / per, picture.size[1] / per],
                });
            }
            if let Some(text) = panel.nodes[index].text.as_ref() {
                let said = words.get(&index).cloned().unwrap_or_else(|| text.text.clone());
                let said = if text.style & 16 != 0 { said.to_uppercase() } else { said };
                if let Some(face) = panel.file_of(text.font.0).and_then(|file| self.tmp_faces.get(&(file, text.font.1))) {
                    let width = face.font.width(&said, text.size, text.spacing, text.style & 1 != 0) + text.margin[0] + text.margin[2];
                    let height = tmp_line(&face.font, text.size) + text.margin[1] + text.margin[3];
                    asked = Some(asked.map_or([width, height], |own| [own[0].max(width), own[1].max(height)]));
                }
            }
            let Some(asked) = asked else { continue };
            let element = panel.nodes[index].element.get_or_insert(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
            for axis in 0..2 {
                if element.preferred[axis] < 0.0 {
                    element.preferred[axis] = asked[axis];
                }
            }
        }
    }
}

impl HealthLook {
    /// A text's font: the game's, where it is had, else the app's.
    pub(super) fn font(&self, panel: &trp_data::interface::screen::ScenePanel, font: (i32, i64)) -> Handle<Font> {
        panel.file_of(font.0).and_then(|file| self.fonts.get(&(file, font.1)).cloned()).unwrap_or_default()
    }

    /// A font asset's own name, by its file and number, where it was read.
    pub(crate) fn asset_name(&self, asset: &(String, i64)) -> Option<&str> {
        self.font_names.get(asset).map(String::as_str)
    }

    /// One of the game's fonts by the end of its asset's own name ("Bender
    /// Normal SDF" of "Jovanny Lemonad - Bender Normal SDF"), where it was
    /// read, not one the game marks unused ("DON'T USE! ..."): for a page of
    /// the app's own drawn in the game's letters (the options tab).
    pub(crate) fn font_named(&self, name: &str) -> Option<Handle<Font>> {
        self.assets_named(name).find_map(|asset| self.fonts.get(asset).cloned())
    }

    /// The bold typeface of one of the game's fonts named as `font_named`
    /// takes it, as TextMeshPro draws its bold: the asset's weight table's
    /// 700 (Bender Normal SDF's is `DON'T USE! Jovanny Lemonad - Bender
    /// Bold SDF`, made from the font Bender Bold), where it was read.
    pub(crate) fn bold_named(&self, name: &str) -> Option<Handle<Font>> {
        self.assets_named(name).find_map(|asset| self.bold_fonts.get(asset).and_then(|bold| self.fonts.get(bold)).cloned())
    }

    fn assets_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a (String, i64)> + 'a {
        let fits = move |own: &str| (own == name || own.ends_with(&format!(" - {name}"))) && !own.starts_with("DON'T USE");
        self.font_names.iter().filter(move |(_, own)| fits(own)).map(|(asset, _)| asset)
    }

    /// The own names of the game's fonts read, for a line that says which
    /// were had.
    pub(crate) fn font_asset_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.font_names.iter().filter(|(asset, _)| self.fonts.contains_key(*asset)).map(|(_, name)| name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// The own name of the font asset a font was made from, where it is
    /// one of the game's.
    pub(crate) fn font_name(&self, font: AssetId<Font>) -> Option<&str> {
        self.fonts.iter().find(|(_, handle)| handle.id() == font).and_then(|(asset, _)| self.asset_name(asset))
    }

    /// A text's font as drawn in its style: bold in its font's bold
    /// typeface where it has one (as TextMeshPro takes it), else its own.
    /// What a text's font asset draws under its letters, if anything.
    pub fn text_effect(&self, panel: &trp_data::interface::screen::ScenePanel, font: (i32, i64)) -> Option<TextEffect> {
        panel.file_of(font.0).and_then(|file| self.text_effects.get(&(file, font.1)).copied())
    }

    /// A text's font asset and material as the game draws it, where both
    /// are had (`TARKOV UI`): the asset by its file and number, the
    /// material the text names for itself, else the asset's own.
    pub fn tmp_of(&self, panel: &trp_data::interface::screen::ScenePanel, text: &trp_data::interface::screen::TextLook) -> Option<((String, i64), (String, i64))> {
        let asset = (panel.file_of(text.font.0)?, text.font.1);
        let material = text.material.and_then(|(file, id)| Some((panel.file_of(file)?, id))).filter(|key| self.tmp_materials.contains_key(key)).or_else(|| self.tmp_default_material.get(&asset).cloned())?;
        (self.tmp_faces.contains_key(&asset) && self.tmp_materials.contains_key(&material)).then_some((asset, material))
    }

    pub fn styled_font(&self, panel: &trp_data::interface::screen::ScenePanel, font: (i32, i64), style: u32) -> Handle<Font> {
        let bold = (style & 1 != 0).then(|| panel.file_of(font.0).and_then(|file| self.bold_fonts.get(&(file, font.1)))).flatten();
        match bold.and_then(|bold| self.fonts.get(bold)) {
            Some(handle) => handle.clone(),
            None => self.font(panel, font),
        }
    }
}
