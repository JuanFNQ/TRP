//! Showing the ground and opening and shutting the screen.

use super::*;

/// A door's lines the game shows but has written as never to be chosen
/// (`InteractionContextHelper`: Bang & clear, Flash & clear, Move in),
/// which the app leaves out, since the game never does them. The simulation's menu
/// keeps them, as the game's.
pub(super) const NEVER_SHOWN: [Action; 3] = [Action::BangAndClear, Action::FlashAndClear, Action::MoveIn];

/// The game's text of the panel's error where a loose thing offers no line
/// (`InteractionContextHelper`: "No Space ({0})", the thing's name in it).
pub(super) const NO_SPACE: &str = "No Space ({0})";

/// The menu's three colours as the game's buttons have them
/// (`InteractionButton`): the chosen line's ground and the other lines'
/// words, the words of a line that cannot be chosen, and the dark of the
/// rest.
pub(super) const MENU_LIGHT: Color = Color::srgb(0.514, 0.514, 0.514);

pub(super) const MENU_DIM: Color = Color::srgb(0.25, 0.25, 0.25);

pub(super) const MENU_DARK: Color = Color::srgba(0.0, 0.0, 0.0, 0.85);

/// Draws each thing that lies on the ground as a plain box of about its
/// size in cells, and takes away those that were picked up. The boxes
/// are the app's; the game's models for loose items are not read.
pub(super) fn show_ground(
    mut commands: Commands,
    carried: Res<Carried>,
    things: Res<crate::world::loose::Loose>,
    shown: Query<(Entity, &LooseItem)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !carried.is_changed() && !things.is_changed() {
        return;
    }
    // A thing the physics has is drawn by its own model (`loose`); the
    // rest as a plain box.
    let boxed = |item: &Item| things.stand_in(item.id, &crate::world::loose::model_key(item));
    for (entity, loose) in &shown {
        if !carried.ground.iter().any(|(item, _)| item.id == loose.0 && boxed(item)) {
            commands.entity(entity).despawn();
        }
    }
    for (item, at) in carried.ground.iter().filter(|(item, _)| boxed(item) && !shown.iter().any(|(_, loose)| loose.0 == item.id)) {
        let [width, height] = item.cells();
        let size = Vec3::new(0.09 * width.min(5) as f32, 0.12, 0.09 * height.min(5) as f32);
        let colour = materials.add(StandardMaterial { base_color: Color::srgb(0.42, 0.44, 0.33), perceptual_roughness: 0.9, ..default() });
        commands.spawn((LooseItem(item.id), Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(colour), Transform::from_translation(*at + Vec3::Y * 0.06)));
    }
}

/// What the character has reached for and the screen waits on: a
/// container searched, to be shown when the wait after its search is
/// over; the container whose screen is open; or a thing on the ground, to
/// be taken when the hand has it.
#[derive(Default)]
pub(super) struct Reaching {
    pub(super) search: Option<usize>,
    pub(super) shown: Option<usize>,
    pub(super) take: Option<Id>,
    /// A thing on the ground to be examined where it lies when the hand
    /// has reached it (the world's EXAMINE).
    pub(super) examine: Option<Id>,
    /// The door being kicked, and the one being unlocked.
    kicked: Option<usize>,
    unlocked: Option<usize>,
    /// The backpack being dropped, until it is off the shoulders.
    pub(super) pack: Option<Id>,
}

/// How near the thing taken up must lie to be heard (`RemoveLootItem`'s
/// `TryPlayAtPoint` with a rolloff of 10).
const TAKEN_HEARD_WITHIN: f32 = 10.0;

/// `Tab` opens and shuts the screen, as the game's inventory key does.
/// `F` takes up the thing looked at on the ground, or does to the door,
/// container or trunk looked at what the game offers (`docs/spec/interactions.md`):
///
/// - A thing on the ground is taken only standing still, as in the game:
///   the left hand reaches for it, and 0.6 s on it goes into the first
///   place that takes it (`Movement::pick_up`). One not known is examined
///   where it lies (EXAMINE): the same reach, its examine begun 0.6 s on
///   and the reach held until it is over (`Movement::pick_up_to_examine`).
/// - A door is opened or shut, and so is a trunk or an open container
///   (`Thing::work`, `Movement::work`): the left hand makes the gesture the
///   thing asks of a character standing here, the character is held to
///   nothing, and the thing begins to move half a second or so later.
/// - A shut container is searched (`Movement::search`): its gesture, and
///   0.55 s after the key the screen opens with it beside the equipment,
///   which cuts the gesture short; unless the character has by then gone a
///   quarter of a metre further from it, or is in a jump, when it is told
///   to shut again. When its screen shuts it is told to shut.
///
/// Open, the mouse is the screen's: the view does not turn and the
/// weapon's keys do nothing (`Controller::inventory_open`). The inventory
/// key does nothing while a thing is being taken up.
pub(super) fn toggle(
    bindings: Res<Bindings>,
    options: Res<Options>,
    reach: Res<Reach>,
    mut range: ResMut<Range>,
    mut carried: ResMut<Carried>,
    mut controller: ResMut<Controller>,
    mut screen: Query<&mut Visibility, With<Screen>>,
    mut hands: Option<ResMut<HandsRig>>,
    camera: Query<&GlobalTransform, With<crate::weapons::weapon::Eye>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    options_tab: Option<Res<crate::screens::options_view::OptionsState>>,
    features: Res<crate::screens::options_view::Features>,
    mut selected: ResMut<Selected>,
    mut started: Local<bool>,
    mut reaching: Local<Reaching>,
    (back, mut back_held, mut asked, mut sounds, mut menu, real, skills, game, mut feedback, mut world_examine, mut statistics, mut clicks): (
        Query<&Interaction, With<GameBack>>,
        Local<bool>,
        ResMut<crate::world::workbench::WorkbenchAsked>,
        MessageWriter<Sound>,
        ResMut<crate::screens::item_menu_view::ItemMenu>,
        Res<Time<Real>>,
        Option<Res<crate::player::skills::Skills>>,
        Res<GameData>,
        ResMut<crate::weapons::wear::Feedback>,
        ResMut<crate::screens::examine_view::WorldExamine>,
        ResMut<crate::player::statistics::Statistics>,
        ResMut<Clicks>,
    ),
) {
    let first = !std::mem::replace(&mut *started, true) && options.inventory;
    // Escape a window over the screen or the window that asks how many
    // took this frame, before the step (`Clicks::escape_taken`).
    let escape_taken = std::mem::take(&mut clicks.escape_taken);
    // Escape with the menu of a right click up shuts it, its menu of lines
    // with it, and nothing else, with no sound (`ItemUiContext.
    // TranslateCommand`: the context menu closed and the key kept from the
    // rest), unless a window is open: the windows are `ItemUiContext`'s
    // own nodes, handed the key before it (`InputNode.TranslateInput`).
    // Before the screen and the window that asks how many (`UIInputRoot.
    // Add` keeps `ItemUiContext` the last of the interface's nodes, handed
    // the keys first: `InputNodeAbstract.TranslateInput`). `courses10`'s
    // `inspect-window`, frames 925 and 1613: gone on the key's frame, the
    // screen kept. (The edit build screen shuts its own: `edit_build::
    // keys`.)
    if options.keys_live() && controller.inventory_open && !controller.edit_build && carried.windows.is_empty() && !escape_taken && menu.0.is_some() && keys.just_pressed(KeyCode::Escape) {
        menu.0 = None;
        keys.clear_just_pressed(KeyCode::Escape);
    }
    // Not while a thing is taken up or unlocked, nor in the approach to a
    // door (`StateLocksInventory`).
    let working = controller.movement.acting() || controller.movement.locks_inventory();
    // The inventory's key and the key that works what is looked at, as
    // the install binds them (`Tab` and `F` in the one read).
    let tab = options.keys_live() && !working && !controller.edit_build && bindings.fired("Inventory");
    // Escape shuts the screen too (`InventoryScreen.TranslateCommand`),
    // and one thing a press, as the game's tree of screens hands it down:
    // a window over the screen has it first (`windows`), the window that
    // asks how many next (`ask`), and the screen only with neither up.
    // The options tab has it while a key is being chosen or a way of
    // being pressed asked for: there Escape keeps what stood.
    let taken = escape_taken || !carried.windows.is_empty() || carried.asking.is_some() || options_tab.as_deref().is_some_and(|tab| tab.takes_escape());
    let escape = options.keys_live() && controller.inventory_open && keys.just_pressed(KeyCode::Escape) && !taken;
    // The game's back button, clicked: pressed on it and let go over it
    // (`ButtonFeedback.OnPointerClick`).
    let back_now = back.iter().copied().max_by_key(|interaction| match interaction {
        Interaction::Pressed => 2,
        Interaction::Hovered => 1,
        Interaction::None => 0,
    });
    let back_clicked = *back_held && back_now == Some(Interaction::Hovered);
    *back_held = back_now == Some(Interaction::Pressed);
    let escape = escape || (back_clicked && controller.inventory_open && !taken);
    // Dead, the screen shuts if it is open, and nothing opens it (the
    // game lets the inventory's view go with the character's life,
    // `Player.OnDead`).
    let dead = controller.health.as_ref().is_some_and(|health| !health.alive);
    let tab = if dead { controller.inventory_open } else { tab || escape };
    let key = !dead && options.keys_live() && !controller.inventory_open && !controller.edit_build && bindings.fired("Interact") && features.world_interactions;
    // What the hands' work on the world has come to.
    let mut search = None;
    for acted in controller.movement.take_acted() {
        match acted {
            // The hand has reached a thing to examine: its examine begins
            // here, with Attention's buff (`InventoryController.Examine`,
            // the pick-up's action); refused, the state runs its 5 s
            // (the game's callback never comes).
            Acted::PickedUp if reaching.examine.is_some() => {
                let id = reaching.examine.take();
                let Some(item) = carried.ground.iter().find(|(item, _)| Some(item.id) == id).map(|(item, _)| item.clone()) else { continue };
                let attention = skills.as_deref().map_or(0.0, |skills| skills.buffs().attention_examine);
                match carried.examiner.begin(&item, attention, real.elapsed_secs_f64()) {
                    Ok(examination) => {
                        info!("examining where it lies {} for {:.3} s (Attention's buff {attention:.2})", item.kind.template, examination.time);
                        world_examine.0 = Some(item.id);
                    }
                    Err(trp_sim::items::examine::Refused::Busy) => {
                        feedback.warning(carried.texts.say("You can't examine two items at the same time").unwrap_or_else(|| "You can't examine two items at the same time".to_owned()));
                    }
                    Err(trp_sim::items::examine::Refused::Known) => {}
                }
            }
            Acted::PickedUp => {
                let carried = &mut *carried;
                let lies = reaching.take.take().and_then(|id| carried.ground.iter().position(|(item, _)| item.id == id));
                if let Some(at) = lies {
                    let (item, lay) = carried.ground.remove(at);
                    let name = carried.name(&item);
                    let picked = item.id;
                    let group = item.kind.sound.clone();
                    // (As it was, for the experience it gives.)
                    let whole = item.clone();
                    let (done, left) = pick_up(&mut carried.equipment.item, item, &carried.rules);
                    if done.is_ok() {
                        carried.known_on_arrival(picked);
                    }
                    // Gone into the inventory as itself, it is counted
                    // looted and found (`statistics`). PROVISIONAL: a
                    // stack only merged into another, or topped up from,
                    // counts for nothing.
                    if let (Ok((_, Applied::Put(_))), true) = (&done, features.inventory) {
                        statistics.taken(&whole, &game.db, crate::player::statistics::elite(skills.as_deref()));
                    }
                    // Gone from the ground whole, it is heard taken up
                    // (`LootItem.RemoveLootItem`); a stack only topped up
                    // from it stays, silent.
                    if done.is_ok() && left.is_none() && features.world_sounds && (lay - camera.single().map_or(lay, |eye| eye.translation())).length() <= TAKEN_HEARD_WITHIN {
                        sounds.write(Sound::Taken(group));
                    }
                    carried.note = match done {
                        Ok((_, Applied::Topped(count))) => format!("{name}: {count} picked up"),
                        Ok(_) => format!("{name}: picked up"),
                        Err(refusal) => format!("{name}: {}", why(&refusal, &carried.texts)),
                    };
                    // What did not go stays where it lay: the thing
                    // itself, or the rest of a stack.
                    if let Some(item) = left {
                        carried.ground.insert(at, (item, lay));
                    }
                }
            }
            // The container's wait is over and the character is still at
            // it: its screen opens now (its own opening began on the same
            // frame, `Thing`).
            Acted::Searched => search = reaching.search.take(),
            Acted::LeftUnsearched => {
                if let Some(index) = reaching.search.take() {
                    let range = &mut *range;
                    if let Some(settings) = range.thing_settings.as_ref() {
                        range.things[index].thing.tell(false, settings);
                    }
                    carried.note = format!("the {}: left before it was open", range.things[index].name);
                }
            }
            // The key has been in the lock long enough: the door is
            // unlocked, and stands shut.
            Acted::Worked => {
                if let Some(index) = reaching.unlocked.take() {
                    range.things[index].thing.unlock();
                    carried.note = format!("the {}: unlocked", range.things[index].name);
                    // A locked door opened with its key (`LockedDoorOpened(false)`).
                    statistics.heard.push(crate::player::experience::Happened::Door { breached: false });
                }
            }
            // The kick lands: the door is thrown open, or holds.
            Acted::Kicked { opens } => {
                if let Some(index) = reaching.kicked {
                    let thing = &mut range.things[index].thing;
                    if opens { thing.kick_open() } else { thing.fail_breach() }
                    // A locked door breached (`LockedDoorOpened(true)`).
                    if opens {
                        statistics.heard.push(crate::player::experience::Happened::Door { breached: true });
                    }
                }
            }
            Acted::Done => reaching.kicked = None,
            // The pack is off the shoulders (`OnBackpackDropEvent`): what
            // is in hand idles again, and the pack lies on the ground a
            // step ahead, as a thing put down does (the game throws it).
            // The bipod's gesture has passed its mark: the weapon's toggle is
            // over (`ToggleBipodOperation.OnBipodToggleEvent`).
            Acted::BipodToggled => {
                if let Some(firearm) = hands.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) {
                    firearm.bipod_toggled();
                }
            }
            Acted::BackpackDropped => {
                if let Some(firearm) = hands.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) {
                    firearm.backpack_dropped();
                }
                if let (Some(id), Ok(eye)) = (reaching.pack.take(), camera.single()) {
                    let ahead = eye.forward().as_vec3().with_y(0.0).normalize_or_zero();
                    let at = Vec3::new(eye.translation().x, controller.movement.position[1], eye.translation().z) + ahead * DROP_AHEAD;
                    let carried = &mut *carried;
                    let name = carried.find(id).map(|item| carried.name(item)).unwrap_or_default();
                    match take_out(&mut [&mut carried.equipment.item, &mut carried.loot], id, &carried.rules) {
                        Ok(item) => {
                            carried.ground.push((item, at));
                            carried.thrown.push((id, crate::world::loose::How::Thrown));
                            carried.note = format!("{name}: dropped");
                        }
                        Err(refusal) => carried.note = format!("{name}: {}", why(&refusal, &carried.texts)),
                    }
                    carried.tidy();
                }
            }
        }
    }
    // The backpack's own key (`Player.DropBackpack`), heard while the
    // character plays; under the screen the key that puts a thing down
    // asks for the same. With a pack on the back that may leave it, the
    // movement letting it (no sprint, no kick, no gesture playing) and
    // what is in hand at rest, the hands' operation for it begins and the
    // body makes the gesture.
    if options.keys_live() && !controller.inventory_open && !controller.edit_build && bindings.fired("DropBackpack") {
        carried.pack_asked = true;
    }
    // (Looked at before it is written: a resource written every frame is
    // one that has changed every frame, and the screen is drawn afresh
    // whenever this one has.)
    let pack_asked = carried.pack_asked;
    if pack_asked {
        carried.pack_asked = false;
    }
    if pack_asked && reaching.pack.is_none() {
        let pack = carried.equipment.worn("Backpack").map(|pack| pack.id);
        let may_leave = pack.is_some_and(|id| {
            let (mut worn, mut loot) = (carried.equipment.item.clone(), carried.loot.clone());
            take_out(&mut [&mut worn, &mut loot], id, &carried.rules).is_ok()
        });
        if let (Some(id), true, true) = (pack, may_leave, controller.movement.can_drop_backpack()) {
            let hands_take = match hands.as_deref_mut() {
                Some(HandsRig { firearm: Some(firearm), .. }) => firearm.drop_backpack(),
                Some(HandsRig { empty: Some(empty), .. }) => empty.idling(),
                _ => true,
            };
            if hands_take {
                if controller.movement.drop_backpack() {
                    reaching.pack = Some(id);
                } else if let Some(firearm) = hands.as_deref_mut().and_then(|rig| rig.firearm.as_mut()) {
                    firearm.backpack_dropped();
                }
            }
        }
    }
    // The interact key does the menu's chosen line; with nothing to
    // choose it does nothing.
    let chosen = reach.menu.as_ref().and_then(Menu::chosen).filter(|_| key);
    let bench_ask = match chosen {
        Some(Action::ModifyWeapon) => Some(crate::world::workbench::BuildFor::Weapon),
        Some(Action::ModifyHelmet) => Some(crate::world::workbench::BuildFor::Helmet),
        Some(Action::ModifyArmor) => Some(crate::world::workbench::BuildFor::Armor),
        _ => None,
    };
    if let (Some(ask), true) = (bench_ask, reach.bench.is_some()) {
        asked.0 = Some(ask);
        return;
    }
    // REPAIR GEAR: the screen opened for repairing (`Carried::repairing`).
    let repair_open = chosen == Some(Action::RepairGear) && reach.bench.is_some() && !controller.inventory_open;
    if repair_open {
        carried.repairing = true;
        carried.note = "a repair kit let go on what it mends repairs it".to_owned();
    }
    if let (Some(Action::Take), Some(id)) = (chosen, reach.loose) {
        if controller.movement.pick_up() {
            reaching.take = Some(id);
        }
        return;
    }
    // EXAMINE on a loose thing: the same reach as for a take, held until
    // the examine is over (`Movement::pick_up_to_examine`; its end is
    // `examine_view::finish_world`'s).
    if let (Some(Action::Examine), Some(id)) = (chosen, reach.loose) {
        if controller.movement.pick_up_to_examine() {
            reaching.examine = Some(id);
        }
        return;
    }
    // SEARCH on a loose container: the screen opens with it beside the
    // equipment, and it goes back where it lay when the screen shuts.
    // PROVISIONAL: at once, with no gesture and no wait (the game's goes
    // through the item's owner's interaction, not read further).
    let mut ground_search = false;
    if let (Some(Action::Search), Some(id)) = (chosen, reach.loose) {
        ground_search = carried.search_ground(id);
        if !ground_search {
            return;
        }
    }
    if let (Some(Action::Breach), Some(index)) = (chosen, reach.thing) {
        // A kick (`MovementContext.StartDoorBreak`): the door says where
        // it is made from, whether it puts the character there first, and
        // whether it will open; it is held from now, and the body's
        // animator takes the character through its approach and the kick.
        if !controller.movement.kicking() && !controller.movement.acting() {
            let feet = controller.movement.position;
            if let Some(breach) = range.things[index].thing.start_breach(feet) {
                controller.movement.kick(breach.opens, breach.snaps.then_some((breach.position, breach.view_target)));
                reaching.kicked = Some(index);
            }
        }
        return;
    }
    if let (Some(Action::Unlock), Some(index)) = (chosen, reach.thing) {
        // The key in the lock (`Player.StartInteraction`): the game's
        // other way to a thing, which holds the character; the door is
        // held for it and unlocked when the hand has been at it.
        let lock = WorldInteraction { interaction: Gesture::DoorKeyOpen, snap: false, height: controller.movement.position[1] + 1.0 };
        if controller.movement.unlock(lock) {
            range.things[index].thing.hold();
            reaching.unlocked = Some(index);
        }
        return;
    }
    if let (Some(action @ (Action::Open | Action::Close | Action::Search)), Some(index), None) = (chosen, reach.thing, reaching.search) {
        let range = &mut *range;
        let Some(settings) = range.thing_settings.as_ref() else { return };
        let feet = controller.movement.position;
        let placed = &mut range.things[index];
        // The gesture may be refused (in a sprint, with the weapon busy):
        // the thing is worked all the same.
        let gesture = placed.thing.work(action != Action::Close, feet, settings);
        if action == Action::Search {
            controller.movement.search(gesture, placed.place);
            reaching.search = Some(index);
        } else {
            controller.movement.work(gesture);
        }
        return;
    }
    // SEARCH on a dead dummy's body: the screen opens with its gear beside
    // the equipment, as the game's `ShowInventoryScreenLoot` does at once
    // (no gesture: `Interact` succeeds for a body, inventory spec,
    // "Searching a body"), and the body keeps it again as the screen shuts.
    let mut body_search = false;
    if let (Some(Action::Search), Some(which), None, None) = (chosen, reach.body, reach.loose, reach.thing) {
        body_search = carried.search_body(which);
        if !body_search {
            return;
        }
    }
    if !(first || tab || search.is_some() || ground_search || body_search || repair_open) {
        return;
    }
    controller.inventory_open = first || search.is_some() || ground_search || body_search || repair_open || !controller.inventory_open;
    let open = controller.inventory_open;
    // The screen opens on its gear tab every time: the game makes the
    // screen afresh at each opening, with its tabs beginning at the
    // gear's (`InventoryScreen.Show`, `TabGroup`), and a container asks
    // for that tab outright (`ShowInventoryScreenLoot`). The tab last
    // looked at is not kept from one opening to the next. With the
    // options' `INVENTORY SYSTEM` off it opens on the options alone.
    let opens_on = if features.inventory { GEAR } else { OPTIONS };
    if open && selected.0 != opens_on {
        selected.0 = opens_on;
    }
    // The container searched is the one the screen shows beside the
    // equipment; at the start, with `--loot`, the range's first container
    // (the supply crate's).
    let first_container = range.things.iter().position(|placed| placed.holds.is_some());
    let shows = search.or((first && options.loot).then_some(first_container).flatten()).filter(|_| open);
    if let Some(index) = shows {
        if let Some(holds) = range.things[index].holds {
            carried.bring(holds);
        }
    }
    // A container whose screen shuts is told to shut, with no hand; a loose
    // one searched goes back where it lay.
    if !open {
        carried.put_back_ground();
        carried.put_back_body();
        carried.repairing = false;
        if let Some(index) = reaching.shown.take() {
            let range = &mut *range;
            if let Some(settings) = range.thing_settings.as_ref() {
                range.things[index].thing.tell(false, settings);
            }
        }
    }
    reaching.shown = shows.filter(|&index| range.things[index].holds.is_some()).or(reaching.shown.filter(|_| open));
    let with_crate = shows.is_some() || (open && (carried.searched_ground.is_some() || carried.searched_body.is_some()));
    if carried.loot_open != with_crate {
        carried.loot_open = with_crate;
    }
    // Whether the screen has the mouse goes by its being open
    // (`cursor::hold`).
    for mut shown in &mut screen {
        *shown = if open { Visibility::Inherited } else { Visibility::Hidden };
    }
}
