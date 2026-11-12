//! The OVERALL panel, its model camera and lights.

use super::*;

/// The inventory screen's OVERALL tab (`OverallScreen`; inventory spec,
/// "The OVERALL tab"): the whole panel, and the figures its scripts keep
/// that its drawing goes by: each clothing dropdown's (`DropDownBox`: its
/// ground's colour under the pointer, the most of its list in sight) and
/// the model's turn by a drag (`XCoordRotation._rotationSpeed`, degrees a
/// pixel).
#[derive(Debug, Clone)]
pub struct OverallPanel {
    pub panel: ScenePanel,
    pub dropdown_highlight: Option<[f32; 4]>,
    pub dropdown_most_seen: Option<f32>,
    /// A line of a dropdown's list, tinted as the mouse is on it (its
    /// template's `Button`, where its transition is the tint: none here, its
    /// transition being an animator's it has not got); and the template the
    /// lines are made from (`_elementTemplate`), as a panel of its own.
    pub line_tints: Option<Tints>,
    pub line: Option<ScenePanel>,
    /// The scroll view of a dropdown's list (`ScrollRectNoDrag`).
    pub list_scroll: Option<Scrolling>,
    pub turn_a_pixel: Option<f32>,
    /// What draws the character into its view (`CameraImage.targetCamera`).
    pub camera: Option<ModelCamera>,
    /// The lights that camera switches on while it draws
    /// (`CameraLightSwitcher.Lights`: the eight of `PlayerMVObjectLights`),
    /// each placed and turned in the view's own axes as the camera is; and
    /// its PRISM (`PrismEffects`, only its filmic tonemap on). Read once,
    /// with the panel: the one set the game lights its menu figure with.
    pub lights: Vec<crate::interface::edit_build::PreviewLight>,
    pub prism: Option<crate::drawing::prism::PrismSettings>,
    /// The side's pictures the character panel's script puts on its side's
    /// icon (`_bearSprite`, `_usecSprite`, `_noSprite`), each its file and
    /// number.
    pub side_sprites: [Option<(String, i64)>; 3],
    /// Each text the install's words fill (`LocalizedText`), by its
    /// object: the key and the case the words are put in.
    pub localized: Vec<(usize, String, u32)>,
}

/// The camera that draws the character into the OVERALL tab's model view
/// (`CameraImage.targetCamera`, the scene's `Camera_inventory`): its field
/// of view up and down and its near and far planes (degrees, metres), and
/// where it stands and how it is turned in the view's own axes, which the
/// character stands in too (`PlayerModelView.Show`), the install's axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelCamera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub place: [f32; 3],
    pub turn: [f32; 4],
}

impl OverallPanel {
    /// Reads it from the scene file that holds the inventory screen.
    pub fn load(install: &Install, file: &str) -> Result<Self> {
        const OVERALL: &str = "/InventoryScreen/Overall Panel";
        let scene = Scene::load(install, file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.nodes[node].name == "Overall Panel" && scene.path(node).ends_with(OVERALL)).with_context(|| format!("{file} has no OVERALL panel"))?;
        let panel = ScenePanel { file: file.to_owned(), ..ScenePanel::of(&scene, top) };
        let below = |path: &str| (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(&format!("{OVERALL}/{path}")));
        // A dropdown's own fields end with its ground's colour under the
        // pointer, four pointers (twelve bytes each: the scroll area's
        // layout, the bar, the arrows up and down) and the most of its
        // list in sight.
        let dropdown = below("LeftSide/ClothingPanel/Upper/ClothingSelector").and_then(|node| scene.behaviour(node, "DropDownBox")).map(|script| scene.fields(script));
        let tail = |fields: &[u8], back: usize| fields.len().checked_sub(back).and_then(|at| fields.get(at..at + 4)).map(|b| f32::from_le_bytes(b.try_into().expect("four")));
        let dropdown_most_seen = dropdown.and_then(|fields| tail(fields, 4));
        let dropdown_highlight = dropdown.and_then(|fields| {
            let at = 4 + 4 * 12 + 16;
            Some([tail(fields, at)?, tail(fields, at - 4)?, tail(fields, at - 8)?, tail(fields, at - 12)?])
        });
        let template = panel.find("LeftSide/ClothingPanel/Upper/ClothingSelector/Element");
        let line_tints = template.and_then(|line| panel.script(line, "Button")).and_then(selectable).and_then(|(tints, _)| tints);
        let line = template.map(|line| panel.subtree(line));
        let list_scroll = panel.find("LeftSide/ClothingPanel/Upper/ClothingSelector/Open Panel/Scroll Area").and_then(|area| Scrolling::of(&panel, area, "ScrollRectNoDrag"));
        let turn_a_pixel = below("LeftSide/CharacterPanel/PlayerModelView/DragTrigger")
            .and_then(|node| scene.behaviour(node, "XCoordRotation"))
            .and_then(|script| scene.fields(script).get(..4).map(|b| f32::from_le_bytes(b.try_into().expect("four"))));
        let camera = match below("LeftSide/CharacterPanel/PlayerModelView") {
            Some(view) => model_camera(install, file, &scene, view).with_context(|| format!("{file}: the OVERALL tab's camera"))?,
            None => None,
        };
        let (lights, prism) = match below("LeftSide/CharacterPanel/PlayerModelView") {
            Some(view) => model_lights(&scene, view).with_context(|| format!("{file}: the OVERALL tab's lights"))?,
            None => (Vec::new(), None),
        };
        // The character panel's script's fields end with fifteen pointers,
        // its own: the side's three pictures are the ninth to the
        // eleventh.
        let window = panel.find("LeftSide").and_then(|side| panel.script(side, "InventoryPlayerModelWithStatsWindow"));
        let sprite = |n: usize| -> Option<(String, i64)> {
            let fields = window?;
            let at = fields.len().checked_sub(15 * 12)? + n * 12;
            let to = pointer(&mut Reader::new(fields.get(at..)?)).ok()?;
            (to.path != 0).then(|| Some((panel.file_of(to.file)?, to.path))).flatten()
        };
        let side_sprites = [sprite(8), sprite(9), sprite(10)];
        let localized = (0..panel.nodes.len()).filter_map(|index| localized(&panel, index).map(|(key, case)| (index, key, case))).collect();
        Ok(Self { panel, dropdown_highlight, dropdown_most_seen, line_tints, line, list_scroll, turn_a_pixel, camera, lights, prism, side_sprites, localized })
    }
}

/// The key and the case of a text the install's words fill
/// (`LocalizedText`: its own key, or the text's words where it has none,
/// `GetTextFromObject`; `EStringCase`).
pub fn localized(panel: &ScenePanel, index: usize) -> Option<(String, u32)> {
    let fields = panel.script(index, "LocalizedText")?;
    let mut r = Reader::new(fields);
    skip_odin(&mut r).ok()?;
    let key = string(&mut r).ok()?;
    let count = r.u32().ok()? as usize;
    r.skip(12 * count).ok()?;
    let case = r.u32().ok()?;
    let key = if key.is_empty() { panel.nodes[index].text.as_ref()?.text.clone() } else { key };
    Some((key, case))
}

/// Words put in a text's case (`EStringCase`: none, lower, upper, title).
pub fn in_case(words: String, case: u32) -> String {
    match case {
        1 => words.to_lowercase(),
        2 => words.to_uppercase(),
        3 => words.split(' ').map(|word| word.chars().take(1).flat_map(char::to_uppercase).chain(word.chars().skip(1).flat_map(char::to_lowercase)).collect::<String>()).collect::<Vec<_>>().join(" "),
        _ => words,
    }
}

/// The camera a model view's `CameraImage` names, if it is one of the
/// scene's own: its figures by the engine's order of a camera's fields
/// (2022.3: its object, whether it is on, how it clears and to what, its
/// kinds of projection and fit, the physical camera's figures and its
/// rectangle on the screen, then the near and far planes and the field of
/// view), and its place and turn from its own transform up to the view's.
/// The lights a model view's camera switches on while it draws
/// (`CameraLightSwitcher`: its list of `Light`s, each turned on before the
/// camera draws and off after), each read as the edit build screen's are
/// (`edit_build::read_light`), on where its object is, and placed and
/// turned in the view's own axes from its transform up (the scale of what
/// it hangs from moving it, as `PlayerMVObjectLights`' (2, 2, 1) does; a
/// light's range is its own, unscaled); and the camera's PRISM, where it
/// has one switched on. None of either where the view names no camera of
/// its scene.
fn model_lights(scene: &Scene, view: usize) -> Result<(Vec<crate::interface::edit_build::PreviewLight>, Option<crate::drawing::prism::PrismSettings>)> {
    const LIGHT: i32 = 108;
    let Some(script) = scene.behaviour(view, "CameraImage") else { return Ok((Vec::new(), None)) };
    let to = pointer(&mut Reader::new(scene.fields(script)))?;
    let Some(camera) = (to.file == 0).then(|| scene.part_owner(to.path)).flatten() else { return Ok((Vec::new(), None)) };
    let prism = scene.behaviour(camera, "PrismEffects").filter(|prism| prism.enabled).map(|prism| crate::drawing::prism::PrismSettings::read(scene.fields(prism))).transpose()?;
    // Its place and turn in the view's axes: each transform's own from the
    // object up to the view's (as `model_camera` walks them).
    let in_view = |node: usize| -> ([f32; 3], [f32; 4]) {
        let (mut place, mut turn, mut at) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0], Some(node));
        while let Some(here) = at.filter(|&here| here != view) {
            let local = scene.local(here).unwrap_or(Local { rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3], scale: [1.0; 3] });
            let moved = rotate(local.rotation, [place[0] * local.scale[0], place[1] * local.scale[1], place[2] * local.scale[2]]);
            place = [moved[0] + local.position[0], moved[1] + local.position[1], moved[2] + local.position[2]];
            turn = multiply(local.rotation, turn);
            at = scene.nodes[here].parent;
        }
        (place, turn)
    };
    let Some(switcher) = scene.behaviour(camera, "CameraLightSwitcher") else { return Ok((Vec::new(), prism)) };
    let mut r = Reader::new(scene.fields(switcher));
    let count = r.u32()? as usize;
    let mut lights = Vec::with_capacity(count);
    for _ in 0..count {
        let light = pointer(&mut r)?;
        let Some(node) = (light.file == 0).then(|| scene.part_owner(light.path)).flatten() else { continue };
        let Some(bytes) = scene.component(node, LIGHT) else { continue };
        let mut light = crate::interface::edit_build::read_light(bytes).with_context(|| scene.path(node))?;
        light.name = scene.nodes[node].name.clone();
        // Off as the file keeps it: the switcher puts it on as the camera
        // draws, where its object is.
        light.on = scene.nodes[node].active;
        (light.position, light.rotation) = in_view(node);
        lights.push(light);
    }
    Ok((lights, prism))
}

fn model_camera(install: &Install, file: &str, scene: &Scene, view: usize) -> Result<Option<ModelCamera>> {
    let Some(script) = scene.behaviour(view, "CameraImage") else { return Ok(None) };
    let to = pointer(&mut Reader::new(scene.fields(script)))?;
    if to.file != 0 || to.path == 0 {
        return Ok(None);
    }
    let mut build = BuildFile::open(install, file)?;
    let mut object = |id: i64| -> Result<(i32, Vec<u8>)> {
        let found = *build.meta.object(id).with_context(|| format!("no object {id}"))?;
        Ok((found.class_id, build.bytes(found.offset, found.size)?))
    };
    let (class_id, camera) = object(to.path)?;
    if class_id != 20 {
        bail!("the view's camera is an object of class {class_id}");
    }
    let mut r = Reader::new(&camera);
    let owner = pointer(&mut r)?;
    r.skip(116 - 12)?;
    let (near, far, fov) = (r.f32()?, r.f32()?, r.f32()?);
    // The object's first part is its transform.
    let (_, owner) = object(owner.path)?;
    let mut r = Reader::new(&owner);
    r.skip(4)?;
    let mut transform = pointer(&mut r)?.path;
    let view_transform = scene.transforms.iter().find(|(_, &node)| node == view).map(|(&id, _)| id);
    let (mut place, mut turn) = ([0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0]);
    for _ in 0..32 {
        if Some(transform) == view_transform {
            return Ok(Some(ModelCamera { fov, near, far, place, turn }));
        }
        let (_, bytes) = object(transform)?;
        let mut r = Reader::new(&bytes);
        pointer(&mut r)?;
        let q = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        let at = [r.f32()?, r.f32()?, r.f32()?];
        let scale = [r.f32()?, r.f32()?, r.f32()?];
        let children = r.i32()?.max(0) as usize;
        r.skip(12 * children)?;
        let father = pointer(&mut r)?;
        // What was had so far is in this transform's own axes.
        let moved = rotate(q, [place[0] * scale[0], place[1] * scale[1], place[2] * scale[2]]);
        place = [moved[0] + at[0], moved[1] + at[1], moved[2] + at[2]];
        turn = multiply(q, turn);
        if father.path == 0 {
            break;
        }
        transform = father.path;
    }
    Ok(None)
}

/// A point turned by a rotation (x, y, z, w).
pub(super) fn rotate([x, y, z, w]: [f32; 4], [px, py, pz]: [f32; 3]) -> [f32; 3] {
    // v + 2w(q×v) + 2q×(q×v)
    let (cx, cy, cz) = (y * pz - z * py, z * px - x * pz, x * py - y * px);
    let (dx, dy, dz) = (y * cz - z * cy, z * cx - x * cz, x * cy - y * cx);
    [px + 2.0 * (w * cx + dx), py + 2.0 * (w * cy + dy), pz + 2.0 * (w * cz + dz)]
}

/// One rotation after another: `b`, then `a` (x, y, z, w).
fn multiply([ax, ay, az, aw]: [f32; 4], [bx, by, bz, bw]: [f32; 4]) -> [f32; 4] {
    [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]
}
