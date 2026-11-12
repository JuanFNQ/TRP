//! The edit build and OVERALL panels, the picture reader and what is shown.

use super::*;

/// The edit build screen's panels from the install: its scene, a slot's
/// view, the empty view its script names, a part's view.
pub(super) fn edit_build_panels(install: &trp_data::Install) -> anyhow::Result<(trp_data::interface::edit_build::EditBuildScene, trp_data::interface::screen::ScenePanel, trp_data::interface::screen::ScenePanel, trp_data::interface::screen::ScenePanel)> {
    use anyhow::Context;
    let scene = trp_data::interface::edit_build::EditBuildScene::load(install)?;
    let paths = trp_data::interface::screen::resource_paths(install)?;
    let prefab = |wanted: &str| -> anyhow::Result<trp_data::interface::screen::ScenePanel> {
        let (_, file, id) = paths.iter().find(|(path, ..)| path == wanted).with_context(|| format!("no resource is at {wanted}"))?;
        let scene = trp_data::interface::screen::Scene::load_prefab(install, file, *id)?;
        let mut panel = trp_data::interface::screen::ScenePanel::of(&scene, 0);
        panel.file = file.clone();
        Ok(panel)
    };
    let slot = prefab("prefabs/ugui/modding_screen_slot")?;
    let part = prefab("prefabs/ugui/layouts/modding_selectable_layout")?;
    // The empty view: the prefab its template pointer's script is on.
    let fields = slot.script(0, "ModdingScreenSlotView").context("the slot's view has no script")?;
    let pointer = fields.get(EB_EMPTY_FIELD..EB_EMPTY_FIELD + 12).context("the slot's view's script is short")?;
    let (outside, id) = (i32::from_le_bytes(pointer[..4].try_into()?), i64::from_le_bytes(pointer[4..].try_into()?));
    let file = slot.file_of(outside).context("the empty view's file is not had")?;
    let empty = trp_data::interface::screen::Scene::load_prefab_of_script(install, &file, id)?;
    let mut empty = trp_data::interface::screen::ScenePanel::of(&empty, 0);
    empty.file = file.clone();
    Ok((scene, slot, empty, part))
}

/// The OVERALL tab as the scene has it, as it shows in a raid: its clothing
/// panel on where the options' `CLOTHING` is (the game's only out of one,
/// `ShowClothingPanel`: a departure of the app's), its lists' scroll areas as
/// high as they show of their lines (at most `_maxVisibleHeight`: the
/// install's suits are many more than that holds, `DropDownBox.Update`),
/// the level's icon kept to its shape (`SetLevelIcon`), the dropdowns'
/// words and their lines' put within their margins.
pub(super) fn overall_panel(install: &trp_data::Install, file: &str, clothing: bool) -> Option<trp_data::interface::screen::OverallPanel> {
    let mut overall = match trp_data::interface::screen::OverallPanel::load(install, file) {
        Ok(overall) => overall,
        Err(e) => {
            eprintln!("the game's OVERALL tab could not be read; none is drawn: {e:#}");
            return None;
        }
    };
    let most_seen = overall.dropdown_most_seen.unwrap_or(120.0);
    let panel = &mut overall.panel;
    for selector in OVERALL_SELECTORS {
        if let Some(area) = panel.find(&format!("{selector}/Open Panel/Scroll Area")) {
            let element = panel.nodes[area].element.get_or_insert(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
            element.min[1] = most_seen;
        }
    }
    if let Some(clothing_panel) = panel.find(OVERALL_CLOTHING).filter(|_| clothing) {
        below_the_box(panel, clothing_panel);
    }
    if let Some(icon) = panel.find("LeftSide/CharacterPanel/Level Panel/Level Icon") {
        if let Some(image) = panel.nodes[icon].image.as_mut() {
            image.preserve_aspect = true;
        }
    }
    let boxes: Vec<usize> = OVERALL_SELECTORS.iter().filter_map(|selector| panel.find(&format!("{selector}/Text"))).collect();
    inset_by_margins(panel, &boxes);
    if let Some(line) = overall.line.as_mut() {
        let texts: Vec<usize> = (0..line.nodes.len()).filter(|&index| line.nodes[index].text.is_some()).collect();
        inset_by_margins(line, &texts);
    }
    Some(overall)
}

/// The clothing panel put on below the character's box rather than in its
/// column: left out of the
/// column's layout (as `LayoutElement.ignoreLayout`), so the box keeps the
/// height the game gives it with the panel off, and laid where the column
/// would lay it after the box (the column's spacing below its foot, as
/// wide as it, its own height). A list that would then run past the
/// screen's foot (at 1080 units high, which every screen is in its units)
/// opens above its box instead, its box's mirror (the game's lists only
/// drop down, `DropDownBox`: its panel is never where TRP puts it).
fn below_the_box(panel: &mut trp_data::interface::screen::ScenePanel, clothing_panel: usize) {
    let Some(column) = panel.nodes[clothing_panel].parent else { return };
    let spacing = panel.nodes[column].group.map_or(0.0, |group| group.spacing);
    let node = &mut panel.nodes[clothing_panel];
    node.active = true;
    let element = node.element.get_or_insert(trp_data::interface::screen::Element { ignore: false, min: [-1.0, -1.0], preferred: [-1.0, -1.0], flexible: [-1.0, -1.0] });
    element.ignore = true;
    let height = element.preferred[1].max(element.min[1]).max(node.layout.size_delta[1]);
    node.layout.anchor_min = [0.0, 0.0];
    node.layout.anchor_max = [1.0, 0.0];
    node.layout.pivot = [0.5, 1.0];
    node.layout.offset = [0.0, -spacing];
    node.layout.size_delta = [0.0, height];
    let screen = [1920.0, crate::screens::inventory_view::SCREEN_HEIGHT];
    for selector in OVERALL_SELECTORS {
        let Some(open) = panel.find(&format!("{selector}/Open Panel")) else { continue };
        let [_, y, _, h] = panel.placed_in(open, screen).rect;
        if y + h > screen[1] {
            let layout = &mut panel.nodes[open].layout;
            layout.anchor_min[1] = 1.0 - layout.anchor_min[1];
            layout.anchor_max[1] = 1.0 - layout.anchor_max[1];
            layout.pivot[1] = 1.0 - layout.pivot[1];
            layout.offset[1] = -layout.offset[1];
        }
    }
}

/// Texts of a panel kept off their rectangles' edges by their margins (left,
/// top, right, bottom: TextMeshPro's), put into their rectangles instead,
/// about their pivots: the panels' drawing lays a text in its rectangle as
/// it is.
pub(crate) fn inset_by_margins(panel: &mut trp_data::interface::screen::ScenePanel, texts: &[usize]) {
    for &index in texts {
        let Some(margin) = panel.nodes[index].text.as_ref().map(|text| text.margin) else { continue };
        let [left, top, right, bottom] = margin;
        let layout = &mut panel.nodes[index].layout;
        let pivot = layout.pivot;
        layout.offset = [layout.offset[0] + left - pivot[0] * (left + right), layout.offset[1] + bottom - pivot[1] * (top + bottom)];
        layout.size_delta = [layout.size_delta[0] - left - right, layout.size_delta[1] - top - bottom];
        if let Some(text) = panel.nodes[index].text.as_mut() {
            text.margin = [0.0; 4];
        }
    }
}

/// What reads the game's pictures for its panels, each texture once.
pub(crate) struct PictureReader<'a> {
    pub(super) pictures: &'a mut trp_data::interface::screen::ScenePictures,
    pub(super) textures: HashMap<(String, i64), usize>,
    pub(super) missing: Vec<String>,
}

impl PictureReader<'_> {
    /// A sprite of that file and number as a picture, its texture among
    /// those kept (`kept`) once.
    pub(crate) fn sprite(&mut self, name: &str, id: i64, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Option<Picture> {
        let cut = self.pictures.sprite(name, id).ok().filter(|cut| !cut.turned)?;
        let texture = match self.textures.get(&cut.texture) {
            Some(&texture) => texture,
            None => {
                let texture = self.pictures.texture(&cut.texture.0, cut.texture.1).ok()?;
                kept.push(Some(texture));
                self.textures.insert(cut.texture.clone(), kept.len() - 1);
                kept.len() - 1
            }
        };
        let [x, y, w, h] = cut.rect;
        Some(Picture { texture, cut: Rect::new(x, y, x + w, y + h), covers: cut.covers(), border: cut.border, size: [cut.whole[2], cut.whole[3]] })
    }

    /// The pictures of a panel's objects that show, by their places.
    pub(crate) fn panel(&mut self, scene: &trp_data::interface::screen::ScenePanel, shown: &[Shown], file: &str, kept: &mut Vec<Option<trp_data::unity::extract::TextureData>>) -> Vec<Option<Picture>> {
        let mut out = vec![None; scene.nodes.len()];
        for (index, node) in scene.nodes.iter().enumerate() {
            if shown[index] == Shown::Never {
                continue;
            }
            let Some((outside, id)) = node.image.and_then(|image| image.sprite) else { continue };
            let name = scene.file_of(outside).or_else(|| (outside == 0).then(|| file.to_owned()));
            out[index] = name.and_then(|name| self.sprite(&name, id, kept));
            if out[index].is_none() {
                self.missing.push(node.path.clone());
            }
        }
        out
    }
}

/// Which of the health panel's objects are drawn in a raid (`Shown`): as
/// `shown_of` has them, with the views and their effects' panels turned on
/// as the panel shows (`BodyPartView.Show`, `EffectsPanel.Show`); never the
/// legend or the damage panels, which are for the damage of a raid gone by
/// (`InventoryScreenHealthPanel.Show`, given none in a raid).
pub(super) fn health_shown(panel: &trp_data::interface::screen::HealthPanel) -> Vec<Shown> {
    let views = &panel.views;
    let turned_on: Vec<usize> = views.parts.iter().flat_map(|view| [Some(view.view), view.effects]).flatten().collect();
    let never: Vec<usize> = views.legend.into_iter().chain(views.parts.iter().filter_map(|view| view.damage)).collect();
    let mut scripted: Vec<usize> = views.parts.iter().filter_map(|view| view.broken).collect();
    for (_, figure) in &views.parameters {
        if let Some(f) = figure {
            scripted.extend([f.ok_glow, f.glow, f.up, f.down, f.rate].into_iter().flatten());
        }
    }
    shown_of(&panel.panel.nodes, &turned_on, &never, &scripted)
}

/// Which of a panel's objects are drawn: those on as the scene has them or
/// turned on as the panel shows, with all they hang from (the panel's top
/// is shown by its tab); never those of `never` or under them; as their
/// script has it, those of `scripted`.
pub(crate) fn shown_of(nodes: &[trp_data::interface::screen::PanelNode], turned_on: &[usize], never: &[usize], scripted: &[usize]) -> Vec<Shown> {
    let on = |index: usize| nodes[index].active || turned_on.contains(&index);
    (0..nodes.len())
        .map(|index| {
            let mut above = nodes[index].parent;
            while let Some(at) = above {
                if never.contains(&at) || (nodes[at].parent.is_some() && !on(at)) {
                    return Shown::Never;
                }
                above = nodes[at].parent;
            }
            if never.contains(&index) {
                Shown::Never
            } else if scripted.contains(&index) {
                Shown::ByScript
            } else if on(index) || nodes[index].parent.is_none() {
                Shown::Always
            } else {
                Shown::Never
            }
        })
        .collect()
}
