//! The outline, the bones and gear points, and the slots placed and prepared.

use super::*;

pub(crate) fn target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let mut target = Image::new_uninit(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    images.add(target)
}

/// The outline's mask: a picture of that size whose figures are kept as
/// drawn (white or black).
pub(super) fn mask_target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let mut target = Image::new_uninit(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 }, TextureDimension::D2, TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    images.add(target)
}

/// The slot outlined while the mouse is on its view (`ModdingScreenSlotView.
/// HoverStartHandler`: `HighlightMod(_modBone, Color, overriding)`; off as it
/// leaves, `HoverEndHandler`), none while the preview is turned: every mesh
/// on the slot's bone and under it that is drawn (`GetComponentsInChildren
/// <Renderer>`, those on) drawn again white into the mask, the line in the
/// slot's colour (its class's, red while it is a vital part missing); the
/// mask's camera drawing on the frames the preview's does. The game's
/// outline of a part under the mouse on the model itself (`Update`'s ray
/// against the parts' colliders) is not built.
pub(super) fn outline(
    mut commands: Commands,
    mut state: ResMut<EditBuild>,
    mut cameras: Query<&mut Camera>,
) {
    let state = &mut *state;
    let (true, Some(camera), Some(mask_camera)) = (state.open, state.preview.camera, state.preview.mask_camera) else { return };
    // (A locked slot outlines nothing: `HoverStartHandler`.)
    let locked = |id: &SlotId| state.slots.iter().any(|slot| (slot.owner, slot.index) == *id && slot.locked);
    let slot = state.hovered.filter(|id| !state.dragging && state.shown.is_some() && !locked(id));
    let wanted = (slot, state.preview.model, state.preview.size);
    if state.preview.outlined != Some(wanted) {
        state.preview.outlined = Some(wanted);
        for copy in state.preview.copies.drain(..) {
            commands.entity(copy).try_despawn();
        }
        // Each object under the slot's bone (its own model's objects follow
        // their parents').
        let bone = slot.and_then(|slot| state.preview.bone_nodes.get(&slot).copied());
        let mut under = vec![false; state.preview.nodes.len()];
        if let Some(bone) = bone.filter(|&bone| bone < under.len()) {
            under[bone] = true;
            for node in bone + 1..under.len() {
                under[node] = state.preview.parents[node].is_some_and(|parent| under[parent]);
            }
        }
        if let Some(material) = state.preview.mask_material.clone() {
            for (node, mesh) in state.preview.drawn.iter().filter(|(node, _)| under.get(*node).copied().unwrap_or(false)) {
                let copy = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::IDENTITY, RenderLayers::layer(MASK_LAYER), bevy::light::NotShadowCaster, ChildOf(state.preview.nodes[*node]))).id();
                state.preview.copies.push(copy);
            }
        }
        let color = slot.filter(|_| !state.preview.copies.is_empty()).and_then(|slot| state.slots.iter().find(|shown| (shown.owner, shown.index) == slot)).map(class_color);
        debug!("the edit build screen's outline: slot {slot:?}, bone {bone:?}, {} meshes, {color:?}", state.preview.copies.len());
        commands.entity(camera).insert(crate::screens::preview_outline::PreviewOutline::of(color, state.preview.size));
        state.preview.frames = state.preview.frames.max(DRAWN_TURNED);
    }
    // The mask drawn with the preview, while a part is outlined.
    let drawing = !state.preview.copies.is_empty() && cameras.get(camera).is_ok_and(|camera| camera.is_active);
    if let Ok(mut mask) = cameras.get_mut(mask_camera) {
        if mask.is_active != drawing {
            mask.is_active = drawing;
        }
    }
}

/// Each slot's bone in the model (`ContainerCollectionView.GetBoneForSlot`,
/// as `ObjectsFactory.AttachMods` found it): the object named after the
/// slot in its item's own model, at any depth (the Mk47's stock's under
/// its model's `mod_stock_000`, which is no slot of it), the shallowest
/// as the model's parts were hung on it (`Hierarchy::find_below`); not
/// into a part's model, the last object hung from a bone named after a
/// slot of the item that holds one, which is that part's top.
pub(super) fn bones(build: &Item, names: &[String], parents: &[Option<usize>]) -> HashMap<SlotId, usize> {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); parents.len()];
    for (child, parent) in parents.iter().enumerate() {
        if let Some(parent) = parent.filter(|&parent| parent < children.len()) {
            children[parent].push(child);
        }
    }
    let mut out = HashMap::new();
    fn walk(item: &Item, top: usize, names: &[String], children: &[Vec<usize>], out: &mut HashMap<SlotId, usize>) {
        let holds = |name: &str| item.kind.slots.iter().zip(&item.slots).any(|(holder, part)| holder.name == name && part.as_ref().is_some_and(rules::is_part));
        let find = |name: &str| -> Option<usize> {
            let mut queue = std::collections::VecDeque::from([top]);
            while let Some(node) = queue.pop_front() {
                if node != top && names[node] == name {
                    return Some(node);
                }
                let below = &children[node];
                let own = if node != top && holds(&names[node]) { below.len().saturating_sub(1) } else { below.len() };
                queue.extend(below[..own].iter().copied());
            }
            None
        };
        for (index, (holder, part)) in item.kind.slots.iter().zip(&item.slots).enumerate() {
            let Some(bone) = find(&holder.name) else { continue };
            out.insert((item.id, index), bone);
            if let Some(part) = part.as_ref().filter(|part| rules::is_part(part)) {
                if let Some(&part_top) = children[bone].iter().max() {
                    walk(part, part_top, names, children, out);
                }
            }
        }
    }
    if !names.is_empty() {
        walk(build, 0, names, &children, &mut out);
    }
    out
}

/// The places of the app's for a helmet's or armour's slots that its model
/// has no object of the slot's name for (the soft armour's inserts, the
/// plates, a helmet's own armour: the game hangs nothing there, an armour
/// plate never, and its screens show no view for such a slot,
/// `ObjectsFactory.AttachMods`, `ItemObserveScreen.CreateModSlotViews`),
/// so that the armor bench can change them (its MODIFY ARMOR):
/// each within the box the model's meshes fill, in its first object's axes
/// (Bevy's), by the words of its name: the front or the back (a nape too),
/// the left or the right side (`_l`, `_r` at its end too), the top (a
/// collar or a shoulder too) or the bottom (a groin); a name with none of
/// these, the middle. Each lies 0.85 of the way out from the middle to the
/// box's side (on the model more than at the box's corner), a soft
/// insert's half the way, behind its plate; a front's or a back's alone,
/// which seen from the front would be one point, the front's higher by 0.4
/// of the way to the top and the back's lower, an insert's by 0.15. A
/// helmet's model stands on its y and faces the engine's forward (Bevy's −z);
/// armour's and a vest's lie on their backs, their up the engine's z
/// (Bevy's −z) and their front its −y, as the game's turn of them to be
/// seen from the front says (`seen_whole`); the wearer's right is +x for
/// either. `PROVISIONAL`, the app's: no slot named `mod_`, which the game
/// finds by its object.
pub(super) fn gear_points(build: &Item, found: &HashMap<SlotId, usize>, low: Vec3, high: Vec3) -> Vec<(SlotId, Vec3)> {
    let middle = (low + high) / 2.0;
    let lying = seen_whole(build);
    let mut out = Vec::new();
    for slot in rules::build_slots(build) {
        if found.contains_key(&(slot.owner, slot.index)) || slot.name.starts_with("mod_") {
            continue;
        }
        let name = slot.name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|word| name.contains(word));
        // Where it lies: to the wearer's right, up, to the front, each from
        // −1 to 1.
        let (mut right, mut up, mut front) = (0.0f32, 0.0f32, 0.0f32);
        if has(&["front", "eyes", "jaw", "face"]) {
            front = 1.0;
        }
        if has(&["back", "nape"]) {
            front = -1.0;
        }
        if has(&["left", "ears"]) || name.ends_with("_l") {
            right = -1.0;
        }
        if has(&["right"]) || name.ends_with("_r") {
            right = 1.0;
        }
        if has(&["top", "collar", "shoulder"]) {
            up = 1.0;
        }
        if has(&["groin"]) {
            up = -1.0;
        }
        let soft = has(&["soft"]);
        if front != 0.0 && right == 0.0 && up == 0.0 {
            up = front * if soft { 0.15 } else { 0.4 };
        }
        let reach = if soft { 0.5 } else { 0.85 };
        let half = (high - low) / 2.0;
        // (Lying, up is Bevy's −z and the front its −y; standing, up its y
        // and the front its −z.)
        let x = middle.x + right * reach * half.x;
        let at = if lying {
            Vec3::new(x, middle.y - front * reach * half.y, middle.z - up * reach * half.z)
        } else {
            Vec3::new(x, middle.y + up * reach * half.y, middle.z - front * reach * half.z)
        };
        out.push(((slot.owner, slot.index), at));
    }
    out
}

/// The slots shown, by the toggles, of the build the preview shows; and
/// where each lies this frame: its bone through the preview's camera onto
/// the screen, and its view on the ellipse (`UpdatePositions`), in the
/// screen's units from its top left. Worked out only when the turn, the
/// model, the window or the slots change.
pub(super) fn place(mut state: ResMut<EditBuild>, window: Query<&Window, With<PrimaryWindow>>, cameras: Query<(&Camera, &GlobalTransform)>) {
    let state = &mut *state;
    if !state.open {
        return;
    }
    let (Ok(window), Some(scene)) = (window.single(), state.scene.clone()) else { return };
    // The slots shown (`CheckVisibility`): a dropdown whose slot is hidden
    // is shut. None while the model of a build just changed is made: the
    // game takes every slot's view away as a part is picked
    // (`RefreshWeapon`: `ClearModSlots`) and makes them again once the new
    // model is in (`CreateModSlotViews`), so nothing of the build before
    // can be clicked meanwhile.
    let refreshing = state.preview.key.as_deref() != Some(state.build_key.as_str());
    if state.slots_for != Some((state.preview.model, state.toggles, refreshing)) {
        state.slots_for = Some((state.preview.model, state.toggles, refreshing));
        let toggles = state.toggles;
        state.slots = if refreshing { Vec::new() } else { state.shown.as_ref().map(|shown| rules::build_slots(shown).into_iter().filter(|slot| toggles[class_toggle(slot.class)]).collect()).unwrap_or_default() };
        if state.menu.is_some_and(|menu| !state.slots.iter().any(|slot| (slot.owner, slot.index) == menu)) {
            state.menu = None;
        }
        state.slots_made += 1;
    }
    let size = UVec2::new(window.physical_width(), window.physical_height());
    let wanted = (state.yaw, state.tilt, state.preview.model, size, state.slots_made, state.preview.camera_z);
    if state.placed_for == Some(wanted) {
        return;
    }
    let Some(Ok((camera, _))) = state.preview.camera.map(|camera| cameras.get(camera)) else { return };
    // (The camera where it stands this frame: its own place is moved by
    // the zoom only as the frame ends.)
    let eye = GlobalTransform::from(Transform::from_translation(convert::position([0.0, scene.height, state.preview.camera_z])));
    let eye = &eye;
    let s = window.height() / HIGH;
    let wide = window.width() / s;
    let turned = rotator_place(&scene, state.yaw, state.tilt).compute_affine();
    let mut points: Vec<(usize, [f32; 2])> = Vec::new();
    for (at, slot) in state.slots.iter().enumerate() {
        let Some(bone) = state.preview.bones.get(&(slot.owner, slot.index)) else { continue };
        let world = (turned * *bone).translation;
        let Ok(pixel) = camera.world_to_viewport(eye, Vec3::from(world)) else { continue };
        // (The picture spans the window: its pixels to the screen's units;
        // the game's from the panel's bottom left.)
        let units = pixel * (HIGH / state.preview.size.y.max(1) as f32);
        points.push((at, [units.x, HIGH - units.y]));
    }
    // A camera not yet worked out places nothing: tried again next frame.
    if points.is_empty() && !state.slots.is_empty() && !state.preview.bones.is_empty() {
        return;
    }
    state.placed_for = Some(wanted);
    let ellipse = scene.panel.find("Ellipse Rect").map(|index| scene.panel.rect(index, [wide, HIGH])).unwrap_or([0.0, 0.0, wide * 0.6, 650.0]);
    let (a, b) = (ellipse[2] / 2.0, ellipse[3] / 2.0);
    let centre = [wide / 2.0, HIGH / 2.0];
    let views = rules::place_slots(&points.iter().map(|(_, p)| *p).collect::<Vec<_>>(), centre, a.max(b), b.min(a));
    let mut placed = vec![None; state.slots.len()];
    for ((at, bone), view) in points.iter().zip(views) {
        placed[*at] = Some(([bone[0], HIGH - bone[1]], [view[0], HIGH - view[1]]));
    }
    state.placed = placed;
}

/// Which toggle shows a class of slot.
fn class_toggle(class: ModClass) -> usize {
    match class {
        ModClass::Vital => 0,
        ModClass::Functional => 1,
        ModClass::Gear => 2,
    }
}

pub(super) fn class_color(slot: &BuildSlot) -> Color {
    if slot.missing {
        return MISSING;
    }
    match slot.class {
        ModClass::Functional => FUNCTIONAL,
        ModClass::Vital => VITAL,
        ModClass::Gear => GEAR,
    }
}

/// A slot's bone's icon (`UpdateInfo`): its class's, full or empty.
pub(super) fn bone_icon(slot: &BuildSlot) -> &'static str {
    match (slot.class, slot.part.is_some()) {
        (ModClass::Functional, true) => "icon_mod_tactical_big_installed",
        (ModClass::Functional, false) => "icon_mod_tactical_big_nomod",
        (ModClass::Vital, true) => "icon_mod_vital_big_installed",
        (ModClass::Vital, false) => "icon_mod_vital_big_nomod",
        (ModClass::Gear, true) => "icon_mod_gear_big_installed",
        (ModClass::Gear, false) => "icon_mod_gear_big_nomod",
    }
}

/// The caption for gear (`Statics`).
pub(super) const ITEM_OVERLOOK: &str = "ITEM OVERLOOK";

/// Whether the screen's preview shows the thing from its front with the
/// camera fitted to it (`ItemObserveScreen.RefreshWeapon`: armour and a
/// vest turned (−90, 180, 0) and placed closest; anything else, a helmet
/// too, turned (0, −90, 0) with the camera where it was).
pub(super) fn seen_whole(item: &Item) -> bool {
    is(item, "Armor") || is(item, "Vest")
}

/// Whether the weapon in hand or the gear worn already has the build
/// (`rules::already_built`, the game's; for gear `rules::same_parts`).
pub(super) fn built(item: &Item, build: &Item, gear: bool) -> bool {
    if gear { rules::same_parts(item, build) } else { rules::already_built(item, build) }
}

/// A slot's name as its bone shows it (`GetModSlotName`, the install's
/// words, upper case); a slot of gear's own armour, which the game shows no
/// view for (`gear_points`), by its own name, its underscores spaces (the
/// app's: the game would name it as a tactical slot, and the install has no
/// words for it).
pub(super) fn slot_said(texts: &HashMap<String, String>, slot: &BuildSlot) -> String {
    if !slot.name.starts_with("mod_") {
        return slot.name.replace('_', " ").to_uppercase();
    }
    let key = rules::slot_words(&slot.name);
    texts.get(&key.to_lowercase()).cloned().unwrap_or_else(|| key.to_owned()).to_uppercase()
}

/// The marks of the kinds of part a slot takes (`SetFilters`): of each
/// kind its filters name (an item's own kind, `GetFilterTypes`) and its
/// excluded ones do not, its mark where it has one (`LoadModIconSprite`),
/// once each, in the filters' order.
pub(crate) fn filter_marks(db: &trp_data::Database, holder: &trp_data::character::inventory::Holder) -> Vec<&'static str> {
    let kind_of = |id: &String| -> Option<String> {
        let item = db.items.get(id)?;
        if item.node_type == "Node" { Some(id.clone()) } else { Some(item.parent.clone()) }
    };
    let refused: Vec<String> = holder.refuses.iter().filter_map(kind_of).collect();
    let mut marks: Vec<&'static str> = Vec::new();
    for kind in holder.takes.iter().filter_map(kind_of).filter(|kind| !refused.contains(kind)) {
        let names = db.class_names(&kind);
        let Some((_, mark)) = crate::screens::health_view::MOD_KINDS.iter().find(|(class, _)| names.iter().any(|name| name == class)) else { continue };
        if !marks.contains(mark) {
            marks.push(mark);
        }
    }
    marks
}

/// The game's screen made ready to draw: what its pictures and texts ask
/// of its layout, its texts' words in the install's language.
pub(super) fn prepare(eb: &EditBuildLook, look: &HealthLook, said: &Words) -> Prepared {
    let mut screen = eb.scene.panel.clone();
    let localized = |panel: &ScenePanel| -> HashMap<usize, String> {
        (0..panel.nodes.len()).filter_map(|index| trp_data::interface::screen::localized(panel, index).map(|(key, case)| (index, trp_data::interface::screen::in_case(said.word(&key, &key), case)))).collect()
    };
    let words = localized(&screen);
    let empty_words = localized(&eb.empty);
    look.ask_by_content(&mut screen, &eb.screen, &words);
    let find = |path: &str| screen.find(path);
    let toggles = ["VitalParts", "FunctionalMods", "EquipmentParts", "OnlyAvailable"];
    let buttons = ["NewBuildButton", "OpenBuildButton", "SaveAsBuildButton", "SelectItemButton", "FindPartsButton", "PublishBuildtButton", "DeleteBuildButton"].iter().filter_map(|name| find(&format!("ButtonsPanel/ContextButtons/{name}"))).collect();
    let slot = &eb.slot;
    let menu_item = slot.find("modding_menu_item");
    let (item_rect, menu_anchor) = match (menu_item, slot.find("modding_menu_item/CanvasGroup/Menu Anchor")) {
        (Some(item), anchor) => {
            // About the view's pivot: where the slot's place on the ellipse
            // puts it (`SetSlotScreenPosition`).
            let rect = slot.rect(item, [0.0, 0.0]);
            let pivot = slot.nodes[item].layout.pivot;
            let at = [rect[0] + pivot[0] * rect[2], rect[1] + (1.0 - pivot[1]) * rect[3]];
            let anchor = anchor.map_or([rect[0] - at[0], rect[3]], |anchor| {
                let a = slot.rect(anchor, [0.0, 0.0]);
                [a[0] - at[0], a[1] - at[1]]
            });
            ([rect[0] - at[0], rect[1] - at[1], rect[2], rect[3]], anchor)
        }
        (None, _) => ([-42.5, -64.0, 85.0, 64.0], [-47.5, 8.0]),
    };
    Prepared {
        stats: find("CharacteristicsPanel"),
        sub_caption: find("Sub-caption"),
        loader: find(crate::screens::health_view::EB_LOADER),
        marks: toggles.map(|name| find(&format!("Toggle Group/{name}/Checkmark/Mark"))),
        toggle: toggles.map(|name| find(&format!("Toggle Group/{name}"))),
        buttons,
        save: find("ButtonsPanel/BottomButtons/Save"),
        assemble: find("ButtonsPanel/BottomButtons/Assemble"),
        assemble_button: find("ButtonsPanel/BottomButtons/Assemble/Button"),
        ready: find(crate::screens::health_view::EB_READY),
        ready_button: find(&format!("{}/Button", crate::screens::health_view::EB_READY)),
        back: find("BackButton"),
        dropdown: find(crate::screens::health_view::EB_DROPDOWN),
        none_holder: find(&format!("{}/ItemViewContainer", crate::screens::health_view::EB_DROPDOWN)),
        grid: find(&format!("{}/Container", crate::screens::health_view::EB_DROPDOWN)),
        caption: ["Caption", "Sub-caption"].iter().filter_map(|path| find(path)).collect(),
        preview_panel: find("Preview Panel"),
        menu_item,
        item_rect,
        menu_anchor,
        bone_icon: slot.find("Bone Icon"),
        bone_panel: slot.find(crate::screens::health_view::EB_SLOT_NAME),
        bone_back: slot.find(&format!("{}/Background", crate::screens::health_view::EB_SLOT_NAME)),
        bone_name: slot.find(&format!("{}/Name", crate::screens::health_view::EB_SLOT_NAME)),
        slot_button: slot.find(crate::screens::health_view::EB_SLOT_BUTTON),
        slot_fade: slot.find(crate::screens::health_view::EB_SLOT_FADE),
        slot_debug: slot.find("modding_menu_item/CanvasGroup/__DEBUG TEXT"),
        screen,
        words,
        empty_words,
    }
}

/// A full-screen layer of the screen, which takes no clicks itself.
pub(super) fn layer() -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }
}

/// A node of that rectangle (the screen's units), at `s` pixels a unit.
pub(super) fn rect_node(rect: [f32; 4], s: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(rect[0] * s), top: Val::Px(rect[1] * s), width: Val::Px(rect[2] * s), height: Val::Px(rect[3] * s), ..default() }
}

/// A point's node, which what it holds hangs about (the screen's units).
pub(super) fn point_node(at: [f32; 2], s: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(at[0] * s), top: Val::Px(at[1] * s), width: Val::Px(0.0), height: Val::Px(0.0), ..default() }
}
