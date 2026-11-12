//! The screen's components, and opening, keys, releasing and shutting it.

use super::*;

/// What is drawn, and for what: the screen's layers, under and over one
/// another in the game's order (the preview's picture; the caption; the
/// slots' lines; the slots; the dropdown; the rest of the screen; the
/// notice), each slot's nodes and what its view shows.
#[derive(Default)]
pub(super) struct Drawn {
    pub(super) root: Option<Entity>,
    pub(super) picture: Option<Entity>,
    pub(super) under: Option<Entity>,
    pub(super) lines: Option<Entity>,
    pub(super) slots: Option<Entity>,
    pub(super) dropdown: Option<Entity>,
    pub(super) over: Option<Entity>,
    pub(super) notice: Option<Entity>,
    pub(super) scale: Option<(f32, f32)>,
    pub(super) statics: Option<Statics>,
    pub(super) statics_for: Option<(u64, [bool; 3])>,
    pub(super) slots_for: Option<u64>,
    pub(super) boxes: Vec<SlotNodes>,
    pub(super) dropdown_for: Option<Option<SlotId>>,
    pub(super) notice_for: Option<Option<String>>,
    /// The characteristics panel's layer, what it was drawn for, and its
    /// bars' comparison pieces (each with its colour), which pulse.
    pub(super) stats: Option<Entity>,
    pub(super) stats_for: Option<(u64, u64, bool)>,
    pub(super) pulse: Vec<(usize, Color)>,
}

/// A slot's nodes: its own node (raised while the mouse is on it,
/// `SetAsLastSibling`) and its view's, and what the view shows.
pub(super) struct SlotNodes {
    pub(super) id: SlotId,
    pub(super) own: Entity,
    pub(super) view: Entity,
    pub(super) shown: BoxShown,
}

/// What a slot's view shows that changes while it stands: its dropdown's
/// button (on the mouse or open), and that button's picture.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct BoxShown {
    pub(super) button: bool,
    pub(super) open: bool,
}

/// What the screen's own objects show: for gear, the caption the game's
/// own screen for a thing not a weapon has (`WeaponModdingScreen.Show`:
/// "ITEM OVERLOOK", which the install's texts leave as it is), in place of
/// the edit build screen's "WEAPON BUILDS".
#[derive(Clone, PartialEq)]
pub(super) struct Statics {
    pub(super) gear: bool,
    pub(super) name: String,
    pub(super) toggles: [bool; 3],
    pub(super) ready: bool,
}

/// The game's screen made ready to draw: its panel with what its pictures
/// and texts ask of its layout put on it, its texts' words, and the
/// objects drawn by the app's systems; a slot view's view object and where
/// its pivot and its menu's anchor lie (`Frame::Is([0, 0])`).
pub(super) struct Prepared {
    pub(super) screen: ScenePanel,
    /// The characteristics panel's top in the screen.
    pub(super) stats: Option<usize>,
    pub(super) words: HashMap<usize, String>,
    pub(super) empty_words: HashMap<usize, String>,
    pub(super) sub_caption: Option<usize>,
    pub(super) loader: Option<usize>,
    pub(super) marks: [Option<usize>; 4],
    pub(super) toggle: [Option<usize>; 4],
    pub(super) buttons: Vec<usize>,
    pub(super) save: Option<usize>,
    pub(super) assemble: Option<usize>,
    pub(super) assemble_button: Option<usize>,
    pub(super) ready: Option<usize>,
    pub(super) ready_button: Option<usize>,
    pub(super) back: Option<usize>,
    pub(super) dropdown: Option<usize>,
    pub(super) none_holder: Option<usize>,
    pub(super) grid: Option<usize>,
    pub(super) caption: Vec<usize>,
    pub(super) preview_panel: Option<usize>,
    pub(super) menu_item: Option<usize>,
    pub(super) item_rect: [f32; 4],
    pub(super) menu_anchor: [f32; 2],
    pub(super) bone_icon: Option<usize>,
    pub(super) bone_panel: Option<usize>,
    pub(super) bone_back: Option<usize>,
    pub(super) bone_name: Option<usize>,
    pub(super) slot_button: Option<usize>,
    pub(super) slot_fade: Option<usize>,
    pub(super) slot_debug: Option<usize>,
}

#[derive(Component)]
pub(super) struct ScreenRoot;

#[derive(Component)]
pub(super) struct PreviewPicture;

/// A slot's node placed each frame: its line, its bone's node or its view's
/// node, by the slot's place among those shown.
#[derive(Component, Clone, Copy)]
pub(super) enum SlotPiece {
    Line(usize),
    Bone(usize),
    View(usize),
}

/// A slot's view's area the mouse is on.
#[derive(Component)]
pub(super) struct SlotHover(pub(super) SlotId);

/// The app's look's loader.
#[derive(Component)]
pub(super) struct AppLoader;

/// The app's dropdown's list, scrolled by the wheel.
#[derive(Component)]
pub(super) struct DropdownList;

/// A part's view, in a slot's view or a dropdown (`ModdingSelectableItemView`):
/// the part it shows, for its name's tooltip, its menu and its double click.
#[derive(Component, Clone)]
pub(super) struct PartView(pub(super) Item);

/// A part in a locked slot (an armour's soft insert): no menu of a right
/// click on it and no inspecting by a double click; its name still shows under the mouse.
#[derive(Component)]
pub(super) struct PartLocked;

/// A dropdown's part the slot would refuse (`SelectableItemView.
/// SetAvailability` off: `SelectCheck` failed), with the game's reason,
/// which its tooltip shows at once in place of its name.
#[derive(Component, Clone)]
pub(super) struct PartRefused(pub(super) String);

/// What the game's view of a part refused is drawn with
/// (`SetUnlockStatus`: the whole view at an alpha of 0.3, here the
/// dropdown's black laid over it at 0.7; `_unavailableBorder`, the
/// prefab's `UnavailableBorder`: the view's frame, `border_generic`, in
/// pure red).
pub(super) const REFUSED_DIM: f32 = 0.7;

pub(super) const REFUSED_BORDER: Color = Color::srgb(1.0, 0.0, 0.0);

#[derive(Component, Clone)]
pub(super) enum Press {
    Open(SlotId),
    Pick(SlotId, Option<Arc<Kind>>),
    Toggle(usize),
    Assemble,
    Back,
    Shut,
    /// The characteristics panel folded or unfolded (`SwitchState`).
    Info,
}

/// The build the characteristics are compared with while the mouse rests on
/// a part in a dropdown (`OnCompareRequired`): the build as it stands with
/// a bare copy of that part in its slot (`AddWithoutRestrictions` of a clone
/// of the dropdown's, which the game makes bare); none while it rests on
/// none, or on a slot no longer in the build.
pub(super) fn compared_build(build: &Item, under: Option<&(SlotId, Option<Arc<Kind>>)>) -> Option<Item> {
    let ((owner, index), kind) = under?;
    build.find(*owner)?;
    let mut compared = build.clone();
    rules::select(&mut compared, *owner, *index, kind.as_ref());
    Some(compared)
}

/// The helmet or the armour worn that the armor bench's line opens the
/// screen for: the headwear worn where it has slots (a helmet; a hat or a
/// cap has none); the body armour worn, else a rig worn with armour's own
/// slots (an armoured rig: a slot not a `mod_`'s, as the storage cabinets
/// sort them). None for the weapon in hand, which is the hands'.
pub(crate) fn worn_gear(equipment: &trp_sim::items::inventory::Equipment, ask: crate::world::workbench::BuildFor) -> Option<&Item> {
    use crate::world::workbench::BuildFor;
    match ask {
        BuildFor::Weapon => None,
        BuildFor::Helmet => equipment.worn("Headwear").filter(|item| !item.kind.slots.is_empty()),
        BuildFor::Armor => equipment.worn("ArmorVest").or_else(|| equipment.worn("TacticalVest").filter(|rig| rig.kind.slots.iter().any(|slot| !slot.name.starts_with("mod_")))),
    }
}

/// What the preview's model is asked for by: a weapon's build's key (its
/// rig's, `trp_sim::inventory::rig_key`), the gear's every part in its
/// slots (`icons::parts_key`, as its picture: `icons::model_switched` hangs
/// them on it, `fit_parts`).
pub(super) fn key_of(build: &Item, gear: bool) -> String {
    if gear { crate::screens::icons::parts_key(build) } else { trp_sim::items::inventory::rig_key(build) }
}

/// A bench's line chosen: the screen opens for the weapon in hand, or the
/// helmet or armour worn, its build a copy of it; what is read once is
/// read.
#[allow(clippy::too_many_arguments)]
pub(super) fn open(
    mut asked: ResMut<crate::world::workbench::WorkbenchAsked>,
    mut hands_asked: ResMut<crate::screens::inventory_view::AskedFor>,
    mut state: ResMut<EditBuild>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    look: Res<HealthLook>,
    mut game: ResMut<crate::GameData>,
    mut controller: ResMut<crate::player::controller::Controller>,
    mut sounds: MessageWriter<Sound>,
) {
    let Some(ask) = asked.0.take().filter(|_| !state.open) else { return };
    let gear = ask != crate::world::workbench::BuildFor::Weapon;
    let found = if gear { worn_gear(&carried.equipment, ask) } else { held.item.and_then(|id| carried.equipment.item.find(id)).filter(|item| is(item, "Weapon")) };
    let Some(weapon) = found else { return };
    let state = &mut *state;
    if state.scene.is_none() {
        state.scene = look.edit_build().map(|eb| eb.scene.clone());
    }
    if state.scene.is_none() {
        match trp_data::interface::edit_build::EditBuildScene::load(&game.unity.install) {
            Ok(scene) => state.scene = Some(Arc::new(scene)),
            Err(e) => warn!("the edit build screen's scene could not be read: {e:#}"),
        }
    }
    if state.inspect.is_none() {
        // (Read with the rest of the screens: `HealthLook::inspect`; where it
        // could not be, the edit build screen's own lights light it.)
        state.inspect = look.inspect.clone();
    }
    if state.parts.is_none() {
        let began = std::time::Instant::now();
        let db = &game.db;
        // (Those hidden as unfinished left out: `HIDE UNFINISHED ITEMS`,
        // `GameData::unfinished`.)
        let kinds: Vec<Arc<Kind>> = db.items.keys().filter(|id| !game.unfinished.contains(*id)).filter_map(|id| Kind::of(db, id).ok().map(Arc::new)).collect();
        let mut parts = rules::all_parts(kinds.iter().cloned());
        parts.sort_by(|a, b| a.name.cmp(&b.name));
        let mut gear_parts = rules::all_gear_parts(kinds);
        gear_parts.sort_by(|a, b| a.name.cmp(&b.name));
        info!("the edit build screen: {} parts of the game, {} things for gear, read in {:.2} s", parts.len(), gear_parts.len(), began.elapsed().as_secs_f32());
        state.parts = Some(Arc::new(parts));
        state.gear_parts = Some(Arc::new(gear_parts));
        let words: Vec<&str> = SLOT_WORDS.to_vec();
        state.texts = trp_data::character::inventory::words_of(&game.words, &words);
    }
    let template = weapon.kind.template.clone();
    if !state.places.contains_key(&template) {
        let key = game.db.items.get(&template).and_then(|item| item.prefab_key()).map(str::to_owned);
        let place = key.and_then(|key| trp_data::interface::icon::preview_place(&mut game.unity, &key).unwrap_or_else(|e| {
            warn!("{template}'s place in the preview: {e:#}");
            None
        }));
        state.places.insert(template, place);
    }
    (state.open, state.weapon, state.gear, state.build) = (true, Some(weapon.id), gear, Some(weapon.clone()));
    // The build compared with is the build as given (`_compareItem`), which
    // nothing is put in yet. PROVISIONAL: the characteristics open unfolded
    // (the panel's `_expanded` not read; a capture of the game shows it so).
    (state.compare, state.stats_folded) = (None, false);
    state.compare_made += 1;
    state.build_key = key_of(weapon, gear);
    state.made += 1;
    (state.yaw, state.tilt, state.pressed_at, state.dragging, state.menu, state.menu_seen, state.hovered, state.notice) = (0.0, 0.0, None, false, None, false, None, None);
    (state.shown, state.slots_for, state.placed_for) = (None, None, None);
    state.slots.clear();
    (state.menu_part, state.clicked) = (None, None);
    state.observed.clear();
    state.placed.clear();
    state.slots_made += 1;
    state.toggles = [true; 3];
    controller.edit_build = true;
    // The weapon put away while the screen stands open (the app's); the
    // hands left as they are for gear.
    if !gear {
        hands_asked.bench().put_away = true;
    }
    (state.give_back, state.built) = (None, false);
    sounds.write(Sound::Ui(Ui::MenuModdingOpen));
    info!("the edit build screen opened for {}", weapon.kind.name);
}

/// The game's notice of a weapon built (`WEAPON_HAS_BEEN_BUILT`).
pub(super) const BUILT: &str = "Weapon has been built";

/// Whether a build lacks a part one of its items cannot do without (a
/// vital slot empty, `MissingVitalParts`): such a build is not built
///, its ASSEMBLE dim and its panel of a
/// build ready to assemble hidden.
pub(super) fn vital_missing(build: &Item) -> bool {
    rules::build_slots(build).iter().any(|slot| slot.missing)
}

/// The words the slots' names are looked up by (`GetModSlotName`), and the
/// notice's.
const SLOT_WORDS: [&str; 31] = [
    BUILT,
    "Magazine", "Stock", "Foregrip", "Barrel", "Charge", "Receiver", "Shaft", "PGrip", "mod_mount", "mod_scope", "mod_tactical", "mod_handguard", "mod_muzzle", "mod_sight_front", "mod_sight_rear", "mod_launcher", "mod_bipod", "mod_silencer",
    "mod_tactical_2", "mod_gas_block", "mod_equipment", "mod_equipment_000", "mod_equipment_001", "mod_nvg", "mod_flashlight", "mod_muzzle_001", "mod_reciever", "mod_pistol_grip", "mod_charge", "mod_mag_shaft",
];

/// Escape: the dropdown shut if open, else the screen (`TranslateCommand`).
pub(super) fn keys(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<EditBuild>, mut commands: Commands, mut sounds: MessageWriter<Sound>, mut menu: ResMut<crate::screens::item_menu_view::ItemMenu>) {
    if !state.open || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // (A part's menu of a right click first, then the dropdown.)
    if state.menu_part.take().is_some() && menu.0.take().is_some() {
        return;
    }
    if state.menu.take().is_some() {
        state.menu_seen = true;
        return;
    }
    shut(&mut state, &mut commands, &mut sounds);
}

/// The screen shut: the character has its keys back once the mouse's
/// left button is up, so the click that shut it (BACK) is no shot; and the
/// weapon put away for it is asked for in hand again (racked as it is
/// drawn where a change was built: `inventory_view::chamber_out`).
pub(super) fn release(mut state: ResMut<EditBuild>, buttons: Res<ButtonInput<MouseButton>>, mut controller: ResMut<crate::player::controller::Controller>, mut hands_asked: ResMut<crate::screens::inventory_view::AskedFor>) {
    if !state.open && controller.edit_build && !buttons.pressed(MouseButton::Left) {
        controller.edit_build = false;
    }
    if let Some((weapon, rack)) = state.give_back.take() {
        let bench = hands_asked.bench();
        (bench.take, bench.rack) = (Some(weapon), rack);
    }
}

/// The screen shut: its nodes, its preview and its camera go, and the
/// keys go back to the character as the left button is let go
/// (`release`; `Close`: `MenuModdingClose`). With a change not built the
/// game asks first (not built: it shuts).
pub(super) fn shut(state: &mut EditBuild, commands: &mut Commands, sounds: &mut MessageWriter<Sound>) {
    state.open = false;
    state.give_back = state.weapon.filter(|_| !state.gear).map(|weapon| (weapon, state.built));
    state.build = None;
    state.shown = None;
    state.menu = None;
    state.dragging = false;
    state.pressed_at = None;
    TURNING.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Some(root) = state.drawn.root {
        commands.entity(root).despawn();
    }
    state.drawn = Drawn::default();
    let preview = std::mem::take(&mut state.preview);
    state.released = preview.key.iter().chain(preview.asked.iter()).cloned().collect();
    for entity in preview.rotator.into_iter().chain(preview.camera).chain(preview.lights) {
        commands.entity(entity).try_despawn();
    }
    sounds.write(Sound::Ui(Ui::MenuModdingClose));
}
