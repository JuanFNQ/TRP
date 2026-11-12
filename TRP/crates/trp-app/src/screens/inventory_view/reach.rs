//! The reach, the screen shut and the carried weight.

use super::*;

/// The screen shut on this frame (`toggle`, after the controller's step)
/// lets go at once of what its clicks were at, as `ask`, `windows` and
/// `drag` let go of it when they find the screen shut. They hear the
/// screen before the step (`ClicksHeard`, so that what they ask of the
/// hands is made before it), and would find it shut a frame on: the window
/// that asks how many and the windows over the screen shut (each that
/// shows a thing sounding its closing, `InfoWindow.Close`), a thing carried
/// let go and its marks gone, the menu of a right click shut. While the
/// screen stays shut there is nothing left for it to do.
#[allow(clippy::too_many_arguments)]
pub(super) fn screen_shut(
    controller: Res<Controller>,
    mut carried: ResMut<Carried>,
    mut clicks: ResMut<Clicks>,
    mut drag: ResMut<Drag>,
    mut menu: ResMut<crate::screens::item_menu_view::ItemMenu>,
    mut drop_shown: ResMut<DropShown>,
    mut ghost: Query<&mut Visibility, With<Ghost>>,
    mut sounds: MessageWriter<Sound>,
) {
    if controller.inventory_open {
        return;
    }
    // (`ask`)
    if carried.asking.is_some() {
        carried.asking = None;
    }
    (clicks.sliding, clicks.ask_press) = (false, None);
    let up = windows_up(&controller);
    if !up {
        // (`stretch_windows`, `windows`)
        clicks.stretching = None;
        clicks.moving = None;
        if !carried.observed.is_empty() {
            carried.observed.clear();
        }
        if !carried.windows.is_empty() || carried.priority.is_some() {
            for _ in carried.windows.iter().filter(|window| window.kind == WinKind::Info) {
                sounds.write(Sound::Ui(Ui::InspectorClose));
            }
            carried.windows.clear();
            carried.priority = None;
        }
    }
    // (`drag`)
    let Ok(mut shown) = ghost.single_mut() else { return };
    if *drop_shown != DropShown::default() {
        *drop_shown = DropShown::default();
    }
    (drag.0, clicks.pressed) = (None, None);
    if *shown != Visibility::Hidden {
        *shown = Visibility::Hidden;
    }
    if !up && menu.0.is_some() {
        menu.0 = None;
    }
}

/// The character's weight read again on an event of the inventory, as the
/// game's character does in its `LateUpdate` after the frame's movement
/// (`PhysicalBase.LateUpdate`; movement spec, "The weight, as the inventory
/// tells of it"): after every move of the frame has settled, so that the
/// next frame's step is the first to go by it. A change it is not told of
/// (`Carried::unheard`: a shot's rounds) waits for the next event, and one
/// of 0.01 kg or less since it was last worked out changes nothing
/// (`Movement::weigh`). The screens' figure is the sum as told
/// (`told_weight`: `HealthParametersPanel` on the same event). With the
/// weight switched off what is carried weighs nothing to the character; a
/// weight given for the run (`--weight`) stands.
pub(super) fn weigh_carried(mut carried: ResMut<Carried>, mut controller: ResMut<Controller>, options: Res<Options>, game: Res<crate::GameData>) {
    if carried.as_weighed() {
        return;
    }
    {
        let carried = carried.bypass_change_detection();
        carried.weighed.told(&carried.equipment.item);
    }
    let sum = carried.equipment.carried_weight(carried.strength_elite);
    if carried.told_weight != sum {
        carried.told_weight = sum;
    }
    if options.weight_given {
        return;
    }
    let weight = if controller.movement.systems.weight { sum } else { 0.0 };
    match controller.movement.weigh(&game.hard, &game.db.config, weight) {
        Ok(true) => info!("carried: {weight:.2} kg"),
        Ok(false) => {}
        Err(e) => warn!("the carried weight: {e:#}"),
    }
}

/// What is in the hands (a weapon, or an item being used) and how many
/// changes of the hands have begun (`Player.HandsChangingEvent`), for the
/// battle screen's quick bar (`battle_view`).
#[derive(Resource, Default)]
pub struct HandsHeld {
    /// The one stack of rounds a reload the magazine picker chose takes
    /// from (its `AmmoPack`: `ammo_picker`), while the hands' reload lasts.
    pub pack: Option<Id>,
    pub item: Option<Id>,
    pub changes: u32,
    /// How many quick draws of the holster's weapon have begun (`Player.
    /// OnStartQuickdrawPistol`, from `Remove.Start` where `CheckForFastWeaponSwitch`
    /// holds), which end the battle bar's pistol glow.
    pub quickdraws: u32,
    /// With the hands empty, the slot whose weapon they take as soon as one
    /// is put in it (the slot they last drew from: `Player.TrySetInHands`).
    pub takes_from: Option<&'static str>,
}

/// What is looked at from within the game's reach for what can be opened
/// or picked up (`Player.InteractionRaycast`): a thing on the ground, or a
/// door, container or trunk of the range (a door within
/// `DOOR_RAYCAST_DISTANCE`, the rest within `LOOT_RAYCAST_DISTANCE`), and
/// the prompt for what the game offers to do to it. Nothing is looked at
/// unless the movement state lets the player (`CanInteract` of its
/// animator state) and the thing in hand does (a rifle only while its
/// logic rests or fires; an item being used never).
pub(crate) fn reach(
    mut commands: Commands,
    view: Res<View>,
    range: Res<Range>,
    game: Res<GameData>,
    mut controller: ResMut<Controller>,
    carried: Res<Carried>,
    bindings: Res<Bindings>,
    texts: Res<MenuTexts>,
    hands: Option<Res<HandsRig>>,
    features: Res<crate::screens::options_view::Features>,
    camera: Query<&GlobalTransform, With<crate::weapons::weapon::Eye>>,
    mut reach: ResMut<Reach>,
    mut prompt: Query<(Entity, &mut Visibility), With<Prompt>>,
    mut shown: Local<Option<(Menu, String)>>,
    (loose, skills, look, dummies): (Res<crate::world::loose::Loose>, Res<crate::player::skills::Skills>, Res<crate::screens::health_view::HealthLook>, Option<Res<crate::world::dummy::Dummies>>),
) {
    // The game's line starts a little behind the point the player looks
    // from (`BEHIND_CAST`) and ends the reach ahead of it.
    let float = |name: &str, or: f32| game.hard.float(name).unwrap_or(or);
    let (within, behind, door) = (float("LOOT_RAYCAST_DISTANCE", 1.3), float("BEHIND_CAST", 0.15), float("DOOR_RAYCAST_DISTANCE", 1.0));
    // Nothing is offered while a thing is being taken up, a door kicked,
    // or a container searched is still to be shown.
    let busy = controller.movement.acting() || controller.movement.searching() || controller.movement.kicking();
    // An item being used lets nothing be done (`MedsController.CanInteract`).
    let hands_let = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).is_none_or(|firearm| firearm.can_interact())
        && hands.as_deref().and_then(|hands| hands.meds.as_ref()).is_none_or(|meds| !meds.in_hand());
    let allowed = controller.movement.can_interact() && hands_let;
    // With the options' `WORLD INTERACTIONS` off nothing is looked at to
    // be worked; with `INVENTORY SYSTEM` off nothing that would go into the
    // inventory is (a thing on the ground, a container to search).
    let casting = view.played() && !controller.inventory_open && !controller.edit_build && features.world_interactions;
    let playing = casting && !busy && allowed;
    let mut seen = Reach::default();
    let mut loose_takes = false;
    // (The sense below is cast whatever the state and the hands: only what
    // is looked at asks them, `Player.InteractionRaycast`; `courses10`'s
    // `examine-world-loot`, the dot whole through the pick-up's state.)
    if let (true, Ok(eye)) = (casting, camera.single()) {
        let (from, to) = (eye.translation() - eye.forward() * behind, eye.translation() + eye.forward() * within);
        // (A thing the physics has, by its centre of mass; one that lies
        // as a box, by the box's middle.)
        let centre = |item: &Item, at: Vec3| loose.places.get(&item.id).map_or(at + Vec3::Y * 0.06, |place| place.centre);
        if playing {
            seen.thing = range.looked_at(from, to, door + behind).filter(|index| features.inventory || range.things[*index].holds.is_none());
            // A thing on the ground is looked at when the line passes within
            // a hand's breadth of it; the nearest along the line is meant.
            let along = to - from;
            let near = |at: Vec3| {
                let share = ((at - from).dot(along) / along.length_squared()).clamp(0.0, 1.0);
                ((from + along * share).distance(at) < LOOSE_REACH).then_some(share)
            };
            let looked_at = carried.ground.iter().filter_map(|(item, at)| near(centre(item, *at)).map(|share| (share, item)));
            // Every loose thing is looked at; the game offers to take one only
            // if a pick-up would find it a place (`GetAvailableInteractionState`).
            // It asks that as the thing comes to be looked at, and again only
            // as the inventory's screen shuts (`Player.InteractionRaycast`, whose
            // `PossibleInteractionsChanged` comes with a new thing or a forced
            // cast, `UpdateInteractionCast`): while the same thing stays looked
            // at, its answer stands, TAKE or "No Space". (The screen open, no
            // thing is looked at here, so the answer is asked afresh as it shuts.)
            let nearest = looked_at.min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, item)| item);
            let answered = |id: Id| (reach.loose == Some(id) && reach.thing == seen.thing).then(|| reach.menu.as_ref().is_some_and(|menu| menu.lines.iter().any(|(action, _)| *action == Action::Take)));
            seen.loose = nearest.filter(|_| features.inventory).map(|item| item.id);
            // With neither, a dead dummy's body (`GameWorld.FindInteractable`
            // finds a corpse on the dead bodies' layer within the same reach),
            // where it has gear to search. `PROVISIONAL`: no line of sight is
            // asked past the range's things (the game's `Linecast` against the
            // loot's obstruction mask).
            if seen.thing.is_none() && seen.loose.is_none() && features.inventory {
                seen.body = dummies.as_deref().and_then(|dummies| dummies.body_looked_at(from, to)).map(|(which, _)| which).filter(|&which| carried.body(which).is_some());
            }
            loose_takes = nearest.is_some_and(|item| answered(item.id).unwrap_or_else(|| can_pick_up(&carried.equipment.item, item, &carried.rules)));
        }
        // Nothing looked at: the sense (`Player.InteractionRaycast`,
        // `GameWorld.InteractionSense`): a sphere cast 1.5 m along the same
        // line, 0.1 m wide times one and Perception's `PerceptionLootDot`
        // (elite: from 3 m over the character straight down, 1.1 m wide,
        // 2.35 m long), finding a loose thing. PROVISIONAL: a loose thing
        // is found where the sphere comes within the look's own hand's
        // breadth of its middle (`LOOSE_REACH`), the thing's colliders not
        // being had; and the not aiming is the weapon's (`Player.Sense`).
        if seen.thing.is_none() && seen.loose.is_none() && seen.body.is_none() && features.inventory {
            let perception = skills.level(trp_sim::condition::skills::Skill::Perception);
            let buff = |id: &str| trp_sim::condition::skills::buffs_of(trp_sim::condition::skills::Skill::Perception).iter().find(|buff| buff.id == id).map(|buff| buff.value(perception));
            let elite = matches!(buff("PerceptionmEliteNoIdea"), Some(trp_sim::condition::skills::BuffValue::Flag(true)));
            let dot = match buff("PerceptionLootDot") {
                Some(trp_sim::condition::skills::BuffValue::Figure(figure)) => figure,
                _ => 0.0,
            };
            let (start, way, length, radius) = if elite {
                let feet = controller.movement.position;
                (Vec3::new(feet[0], feet[1] + 3.0, -feet[2]), Vec3::NEG_Y, 2.35, 1.1)
            } else {
                (from, eye.forward().as_vec3(), 1.5, 0.1 * (1.0 + dot))
            };
            let aimed = hands.as_deref().and_then(|hands| hands.firearm.as_ref()).is_some_and(|firearm| firearm.aiming());
            seen.sense = !aimed
                && carried.ground.iter().any(|(item, at)| {
                    let middle = centre(item, *at);
                    let along = (middle - start).dot(way).clamp(0.0, length);
                    (start + way * along).distance(middle) < radius + LOOSE_REACH
                });
        }
    }
    // The game's menu for it (`InteractionContextHelper.GetAvailableActions`):
    // a thing on the ground is taken; a door, a container or a trunk
    // offers what its kind and its state have, a door's kick only from
    // where the door lets it be made and not lying down, a lock's key
    // only to a character that carries it.
    let feet = controller.movement.position;
    let lying = matches!(controller.movement.state, State::ProneIdle | State::ProneMove | State::FromProne);
    let threshold = float("DOOR_BREACH_THRESHOLD", 0.8);
    // With nothing else looked at, a bench when its zone has the character
    // (`range::Workbench::in_sight`): its lines, the gun bench's MODIFY
    // WEAPON chosen only with a weapon in hand (it opens the edit build
    // screen for the weapon in hand), the armor bench's MODIFY
    // HELMET and MODIFY ARMOR only with one worn.
    seen.bench = if playing && seen.thing.is_none() && seen.loose.is_none() && seen.body.is_none() {
        range.benches.iter().find(|bench| bench.in_sight(Vec3::new(feet[0], feet[1], -feet[2]), range.radius)).map(|bench| bench.kind)
    } else {
        None
    };
    let armed = hands.as_deref().is_some_and(|hands| hands.firearm.is_some()) && controller.armed;
    // A loose thing (`InteractionContextHelper.GetAvailableInteractionState`):
    // TAKE where a pick-up finds it a place; EXAMINE where its kind is not
    // known (a body's gear is no loose thing here); SEARCH for a container
    // that is no quest's (`IsContainer`); with none, no lines and the
    // panel's error "No Space ({0})" with its short name. (MODDING, an
    // armour's slots opened as a container, is not built.) Its name is its
    // short name, a stack's count in brackets after it where it can be
    // taken; not known, "Unknown item" (`LootItem.Name`, every line's
    // `TargetName`; `courses10`'s `examine-world-loot`, "UNKNOWN ITEM" over
    // TAKE and EXAMINE, the short name over TAKE alone once examined).
    let (mut menu, name) = match (seen.loose.and_then(|id| carried.ground.iter().find(|(item, _)| item.id == id)), seen.thing) {
        (Some((item, _)), _) => {
            let known = trp_sim::items::examine::known(item);
            let mut lines = Vec::new();
            if loose_takes {
                lines.push((Action::Take, false));
            }
            if !known {
                lines.push((Action::Examine, false));
            }
            if Carried::opens(item) && !item.kind.quest {
                lines.push((Action::Search, false));
            }
            let short = carried.name(item);
            let name = match (known, loose_takes && item.count > 1) {
                (false, _) => carried.texts.say("Unknown item").unwrap_or_else(|| "Unknown item".to_owned()),
                (true, true) => format!("{short} ({})", item.count),
                (true, false) => short.clone(),
            };
            if lines.is_empty() {
                seen.error = Some(texts.0.get(&NO_SPACE.to_lowercase()).cloned().unwrap_or_else(|| NO_SPACE.to_owned()).replace("{0}", &short));
            }
            seen.name = Some(name.clone());
            ((!lines.is_empty()).then(|| Menu::of(lines)), name)
        }
        (None, Some(index)) => {
            let placed = &range.things[index];
            let has_key = placed.thing.key.as_deref().is_some_and(|key| carries(&carried.equipment.item, key));
            // Less a door's three lines the game shows but never lets be
            // chosen (`NEVER_SHOWN`); the first line that can be is chosen
            // as before.
            let menu = placed.thing.menu(feet, has_key, lying, threshold).map(|menu| Menu::of(menu.lines.into_iter().filter(|(action, _)| !NEVER_SHOWN.contains(action)).collect()));
            (menu, placed.name.to_owned())
        }
        // The user's REPAIR GEAR on either, had only with a repair kit
        // carried, as an unlocking only with the key.
        // A body: its one line, SEARCH (`GetAvailableInteractionState` for
        // a loot item whose root is an equipment: no TAKE, no EXAMINE),
        // its name the corpse's (`Corpse.Init`: "Corpse").
        (None, None) if seen.body.is_some() => {
            let name = carried.texts.say("Corpse").unwrap_or_else(|| "Corpse".to_owned());
            seen.name = Some(name.clone());
            (Some(Menu::of(vec![(Action::Search, false)])), name)
        }
        (None, None) if seen.bench.is_some() => {
            use crate::world::range::BenchKind;
            use crate::world::workbench::BuildFor;
            let kind = seen.bench.unwrap_or(BenchKind::Gun);
            let has_kit = carries_class(&carried.equipment.item, REPAIR_KITS_CLASS);
            let worn = |ask| crate::screens::edit_build::worn_gear(&carried.equipment, ask).is_none();
            let lines = match kind {
                BenchKind::Gun => vec![(Action::ModifyWeapon, !armed), (Action::RepairGear, !has_kit)],
                BenchKind::Armor => vec![(Action::ModifyHelmet, worn(BuildFor::Helmet)), (Action::ModifyArmor, worn(BuildFor::Armor)), (Action::RepairGear, !has_kit)],
            };
            (Some(Menu::of(lines)), kind.name().to_owned())
        }
        (None, None) => (None, String::new()),
    };
    // The lines as they stood keep the choice the wheel made; when they
    // change (another thing, another state of it, the kick's angle
    // crossed) the menu is made afresh, with its first line chosen.
    let same = reach.thing == seen.thing && reach.loose == seen.loose && reach.bench == seen.bench && reach.body == seen.body;
    if let (Some(menu), Some(before)) = (menu.as_mut(), reach.menu.as_ref().filter(|_| same)) {
        if menu.lines == before.lines {
            menu.selected = before.selected;
        }
    }
    // The wheel: a notch up is a line up, a notch down a line down.
    let notches = std::mem::take(&mut controller.menu_notches);
    if let Some(menu) = menu.as_mut() {
        for _ in 0..notches.unsigned_abs() {
            menu.scroll(notches < 0);
        }
    }
    if controller.menu_shown != menu.is_some() {
        controller.menu_shown = menu.is_some();
    }
    seen.menu = menu;
    if *reach != seen {
        *reach = seen;
    }
    // With `TARKOV UI` the game's own panel is drawn (`action_view`); the
    // app's own look otherwise: every line, the chosen one light with dark
    // words, one that cannot be chosen dim, the error as a line of its own.
    let games = look.tarkov_ui && look.battle.as_ref().is_some_and(|views| views.action.is_some());
    let now = if games {
        None
    } else {
        reach.menu.clone().map(|menu| (menu, name.clone())).or_else(|| reach.error.clone().map(|error| (Menu::of(Vec::new()), error)))
    };
    if *shown == now {
        return;
    }
    let key = bindings.caption("Interact");
    for (panel, mut visible) in &mut prompt {
        let wanted = if now.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *visible != wanted {
            *visible = wanted;
        }
        commands.entity(panel).despawn_related::<Children>();
        let Some((menu, name)) = &now else { continue };
        commands.entity(panel).with_children(|lines| {
            lines.spawn((PromptLine, words(format!("{key}   {}", name.to_uppercase()), 12.0, FAINT)));
            for (index, (action, disabled)) in menu.lines.iter().enumerate() {
                let chosen = menu.selected == Some(index);
                let (back, ink) = match (chosen, *disabled) {
                    (true, _) => (MENU_LIGHT, MENU_DARK),
                    (false, true) => (MENU_DARK, MENU_DIM),
                    (false, false) => (MENU_DARK, MENU_LIGHT),
                };
                lines.spawn((PromptLine, Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)), ..default() }, BackgroundColor(back))).with_children(|line| {
                    line.spawn(words(texts.of(*action), 15.0, ink));
                });
            }
        });
    }
    *shown = now;
}
