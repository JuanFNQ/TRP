//! A scene's panel: its nodes, row groups, elements and lookups.

use super::*;

/// A part of a screen as its scene has it: an object and everything under
/// it, in the order the screen draws them (each over those before), each
/// with its layout and what it draws.
#[derive(Debug, Clone, Default)]
pub struct ScenePanel {
    pub nodes: Vec<PanelNode>,
    /// The scene's outside files, which its sprites' pointers count in.
    pub externals: Vec<String>,
    /// The file it is read from, where a pointer that names no outside
    /// file leads (empty where the caller knows it).
    pub file: String,
    /// Each of its objects, and each script on them, by the number a
    /// pointer of the scene's own gives it.
    pub(super) ids: HashMap<i64, usize>,
    /// Each of its objects by its transform's number.
    pub(super) transforms: HashMap<i64, usize>,
}

#[derive(Debug, Clone)]
pub struct PanelNode {
    pub name: String,
    /// Its path below the panel's top (`Chest/ProgressBarImage/Value`);
    /// the top's own is empty.
    pub path: String,
    pub parent: Option<usize>,
    /// Whether it is on as the scene has it.
    pub active: bool,
    pub layout: Layout,
    pub image: Option<ImageLook>,
    pub text: Option<TextLook>,
    /// Its scripts: each one's class and own fields.
    pub scripts: Vec<(String, Vec<u8>)>,
    /// The row its children are laid out in while the game runs, if it
    /// has one, and what it asks of the row it is in.
    pub group: Option<RowGroup>,
    pub element: Option<Element>,
}

/// A horizontal layout group (`HorizontalLayoutGroup`), read by the
/// engine's order of its fields: its padding (left, right, top, bottom),
/// the corner it lays out from (`TextAnchor`, 0 to 8), the space between
/// its children, whether it makes them grow and whether it sets their
/// size, across and up and down.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RowGroup {
    /// A column (`VerticalLayoutGroup`) rather than a row.
    pub vertical: bool,
    pub padding: [f32; 4],
    pub alignment: u32,
    pub spacing: f32,
    pub expand: [bool; 2],
    pub control: [bool; 2],
}

/// What a child asks of the group it is in (`LayoutElement`): its least,
/// preferred and flexible width and height, each below 0 where not set,
/// or to be left out of the layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Element {
    pub ignore: bool,
    pub min: [f32; 2],
    pub preferred: [f32; 2],
    pub flexible: [f32; 2],
}

pub(super) fn row_group(fields: &[u8]) -> Option<RowGroup> {
    let read = |r: &mut Reader| -> Result<RowGroup> {
        let padding = [r.i32()? as f32, r.i32()? as f32, r.i32()? as f32, r.i32()? as f32];
        let (alignment, spacing) = (r.u32()?, r.f32()?);
        let expand = [flag(r)?, flag(r)?];
        let control = [flag(r)?, flag(r)?];
        Ok(RowGroup { vertical: false, padding, alignment, spacing, expand, control })
    };
    read(&mut Reader::new(fields)).ok()
}

pub(super) fn element(fields: &[u8]) -> Option<(Element, i32)> {
    let read = |r: &mut Reader| -> Result<(Element, i32)> {
        let ignore = flag(r)?;
        let (min, preferred, flexible) = (pair(r)?, pair(r)?, pair(r)?);
        // (Its priority, 1 unless set: an older file without it reads 1.)
        let priority = r.i32().unwrap_or(1);
        Ok((Element { ignore, min, preferred, flexible }, priority))
    };
    read(&mut Reader::new(fields)).ok()
}

/// What an object's layout elements ask together (UGUI's
/// `LayoutGroup.rectChildren` and `LayoutUtility.GetLayoutProperty`): left
/// out only where every one of them says so, switched on or not; each size
/// from the switched-on ones that set it (0 or more), the highest priority
/// first and the largest of those. A slot's mastering slider has two, one
/// left out and one 10 units wide (`slot_layout`'s `MasteringSlider`): the
/// first alone made it left out.
fn elements(list: &[(bool, Element, i32)]) -> Option<Element> {
    if list.is_empty() {
        return None;
    }
    let ignore = list.iter().all(|(_, element, _)| element.ignore);
    let property = |of: &dyn Fn(&Element) -> f32| {
        let (mut value, mut most) = (-1.0f32, i32::MIN);
        for (_, element, priority) in list.iter().filter(|(on, ..)| *on) {
            let here = of(element);
            if *priority < most || here < 0.0 {
                continue;
            }
            if *priority > most {
                (value, most) = (here, *priority);
            } else if here > value {
                value = here;
            }
        }
        value
    };
    let both = |of: &dyn Fn(&Element) -> [f32; 2]| [property(&|e: &Element| of(e)[0]), property(&|e: &Element| of(e)[1])];
    Some(Element { ignore, min: both(&|e| e.min), preferred: both(&|e| e.preferred), flexible: both(&|e| e.flexible) })
}

/// Where an object of a panel is drawn: its left, top, width and height
/// from the top's top left corner, y down, and whether it is drawn
/// mirrored across and up and down (a scale of -1 on it or above it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub rect: [f32; 4],
    pub flip: [bool; 2],
}

impl ScenePanel {
    /// The object of a scene read with its scripts, and all under it.
    pub fn of(scene: &Scene, top: usize) -> Self {
        fn walk(scene: &Scene, index: usize, parent: Option<usize>, path: String, panel: &mut ScenePanel, transforms: &HashMap<usize, i64>, parts: &HashMap<usize, Vec<i64>>) {
            let node = &scene.nodes[index];
            let Some(layout) = node.layout else { return };
            let here = panel.nodes.len();
            // (An image or a text switched off draws nothing.)
            let on = |class: &str| scene.behaviour(index, class).filter(|script| script.enabled);
            let image = on("Image").and_then(|script| image_look(scene.fields(script)));
            let text = ["CustomTextMeshProUGUI", "TextMeshProUGUI"].iter().find_map(|class| on(class)).and_then(|script| text_look(scene.fields(script)));
            let mut scripts = Vec::new();
            for behaviour in scene.behaviours.iter().filter(|behaviour| behaviour.node == index) {
                panel.ids.insert(behaviour.id, here);
                scripts.push((scene.class_of(behaviour).unwrap_or("").to_owned(), scene.fields(behaviour).to_vec()));
            }
            if let Some((&id, _)) = scene.objects.iter().find(|(_, &object)| object == index) {
                panel.ids.insert(id, here);
            }
            if let Some(&id) = transforms.get(&index) {
                panel.transforms.insert(id, here);
            }
            for &id in parts.get(&index).into_iter().flatten() {
                panel.ids.entry(id).or_insert(here);
            }
            let group = scene
                .behaviour(index, "HorizontalLayoutGroup")
                .and_then(|script| row_group(scene.fields(script)))
                .or_else(|| scene.behaviour(index, "VerticalLayoutGroup").and_then(|script| row_group(scene.fields(script))).map(|group| RowGroup { vertical: true, ..group }));
            let listed: Vec<(bool, Element, i32)> = scene.behaviours_of(index, "LayoutElement").filter_map(|script| element(scene.fields(script)).map(|(element, priority)| (script.enabled, element, priority))).collect();
            let element = elements(&listed);
            panel.nodes.push(PanelNode { name: node.name.clone(), path: path.clone(), parent, active: node.active, layout, image, text, scripts, group, element });
            for &child in &node.children {
                let name = &scene.nodes[child].name;
                let below = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
                walk(scene, child, Some(here), below, panel, transforms, parts);
            }
        }
        let mut panel = Self { externals: scene.externals.clone(), ..Self::default() };
        let transforms: HashMap<usize, i64> = scene.transforms.iter().map(|(&id, &index)| (index, id)).collect();
        let mut parts: HashMap<usize, Vec<i64>> = HashMap::new();
        // (A transform's number stays its place's alone: `transform_at`.)
        for (&id, &index) in scene.components.iter().filter(|(id, _)| !scene.transforms.contains_key(id)) {
            parts.entry(index).or_default().push(id);
        }
        walk(scene, top, None, String::new(), &mut panel, &transforms, &parts);
        panel
    }

    /// A copy of an object and all under it, hung last under another and
    /// on (`Object.Instantiate(template, parent)` with `SetActive(true)`,
    /// as a script fills a row from a template it keeps off): the copy's
    /// number. The copies are named as the engine names them (`(Clone)`).
    pub fn copy_into(&mut self, template: usize, parent: usize) -> usize {
        let depth_under = |panel: &ScenePanel, mut at: usize| -> Option<usize> {
            let mut depth = 0;
            loop {
                if at == template {
                    return Some(depth);
                }
                at = panel.nodes[at].parent?;
                depth += 1;
            }
        };
        let mut under: Vec<(usize, usize)> = (0..self.nodes.len()).filter_map(|index| Some((depth_under(self, index)?, index))).collect();
        under.sort();
        let (top_path, top_name) = (self.nodes[template].path.clone(), self.nodes[template].name.clone());
        let into = &self.nodes[parent].path;
        let named = if into.is_empty() { format!("{top_name}(Clone)") } else { format!("{into}/{top_name}(Clone)") };
        let mut copies: HashMap<usize, usize> = HashMap::new();
        for (_, index) in under {
            let mut node = self.nodes[index].clone();
            if index == template {
                node.parent = Some(parent);
                node.active = true;
                node.name = format!("{top_name}(Clone)");
            } else {
                node.parent = node.parent.and_then(|above| copies.get(&above).copied());
            }
            node.path = format!("{named}{}", node.path.strip_prefix(top_path.as_str()).unwrap_or_default());
            copies.insert(index, self.nodes.len());
            self.nodes.push(node);
        }
        copies[&template]
    }

    /// One of its objects by its path below the top.
    pub fn find(&self, path: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.path == path)
    }

    /// One of its objects and all under it, as a panel of its own with that
    /// object at its top, on (a template the game makes copies of: a
    /// dropdown's line).
    pub fn subtree(&self, top: usize) -> Self {
        let below = |index: usize| {
            let mut at = Some(index);
            while let Some(here) = at {
                if here == top {
                    return true;
                }
                at = self.nodes[here].parent;
            }
            false
        };
        let kept: Vec<usize> = (0..self.nodes.len()).filter(|&index| below(index)).collect();
        let place = |index: usize| kept.iter().position(|&at| at == index);
        let prefix = format!("{}/", self.nodes[top].path);
        let nodes = kept
            .iter()
            .map(|&index| {
                let node = &self.nodes[index];
                let path = if index == top { String::new() } else { node.path.strip_prefix(&prefix).unwrap_or(&node.path).to_owned() };
                let parent = if index == top { None } else { node.parent.and_then(place) };
                PanelNode { path, parent, active: node.active || index == top, ..node.clone() }
            })
            .collect();
        let remap = |map: &HashMap<i64, usize>| map.iter().filter_map(|(&id, &index)| Some((id, place(index)?))).collect();
        Self { nodes, externals: self.externals.clone(), file: self.file.clone(), ids: remap(&self.ids), transforms: remap(&self.transforms) }
    }

    /// The fields of a script of that class on an object.
    pub fn script(&self, index: usize, class: &str) -> Option<&[u8]> {
        self.nodes[index].scripts.iter().find(|(name, _)| name == class).map(|(_, fields)| fields.as_slice())
    }

    /// The object of the panel that the pointer at that place in a
    /// script's fields leads to (none for one that is empty or leads
    /// outside the panel).
    pub fn pointer_at(&self, fields: &[u8], at: usize) -> Option<usize> {
        let to = pointer(&mut Reader::new(fields.get(at..)?)).ok()?;
        (to.file == 0 && to.path != 0).then(|| self.ids.get(&to.path).copied()).flatten()
    }

    /// The object of the panel whose place (its `RectTransform`) the
    /// pointer at that place in a script's fields leads to.
    pub fn transform_at(&self, fields: &[u8], at: usize) -> Option<usize> {
        let to = pointer(&mut Reader::new(fields.get(at..)?)).ok()?;
        (to.file == 0 && to.path != 0).then(|| self.transforms.get(&to.path).copied()).flatten()
    }

    /// Whether an object is on as the scene has it, it and everything
    /// between it and the top (the top itself is taken as shown).
    pub fn shown(&self, index: usize) -> bool {
        let node = &self.nodes[index];
        match node.parent {
            Some(parent) => node.active && self.shown(parent),
            None => true,
        }
    }
}
