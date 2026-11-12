//! The debug panel's commands, worked by its buttons with the mouse freed while it is open.
//! The panel opens with `` ` ``: hunger and thirst, stamina, weapon durability and more. The app's tools for trying
//! things out, none of them the game's: what each does is written on it.
//!
//! - **Character**: hunger and thirst, and the stamina's three pools each
//!   on its own row (the legs', the arms' and the breath's, each with its
//!   own buttons to lower and raise it; the breath is the pool no bar
//!   shows, whose level sets the view's heave), each down or up by ten,
//!   empty or full; its figure beside it.
//! - **Health**: every part whole and what hurts it gone (bleedings,
//!   fractures, wounds, pain, tremor, the low health, exhaustion and
//!   dehydration, chronic fatigue, a contusion, a stun, a flash, the
//!   tunnel vision; what was taken on purpose stays: painkillers, medicine
//!   at work, stimulators and their buffs); a bullet's hurt to the next
//!   part, and death (commands of the panel, with no keys of their own).
//! - **The weapon in hand**: its durability down or up by ten (its
//!   greatest the cap), its heat up by 25 or cooled, its magazine filled
//!   (its own round, else the weapon's own), its malfunction cleared, or one
//!   of the five given to its next shot: only those its template allows
//!   (`WeaponFigures::allows`), the rest hidden.
//! - **World**: the dummies dressed as the player and made whole; their weapon lights on or off (to shine one
//!   at the camera: tactical-devices spec, "The light shaft and the flare").
//!
//! While the panel is open the mouse moves no view and fires nothing
//! (`keys::hear`); the keyboard still plays.

use bevy::prelude::*;
use trp_sim::condition::health::Kind;
use trp_sim::weapons::malfunction::Malf;

use crate::player::controller::Controller;
use crate::screens::inventory_view::{words, Carried, HandsHeld, FAINT, LINE, PANEL, WORDS};
use crate::weapons::weapon::HandsRig;

/// One of the panel's commands.
#[derive(Message, Component, Clone, Copy, Debug, PartialEq)]
pub enum DebugCommand {
    Hunger(Change),
    Thirst(Change),
    Legs(Change),
    Arms(Change),
    Breath(Change),
    HealAll,
    HurtNext,
    Die,
    Durability(f32),
    HeatUp,
    Cool,
    FillMagazine,
    ClearMalfunction,
    MalfunctionNext(Malf),
    /// Heals the dummies and stands them up, in their own gear as it is
    /// (`dummy::heal_dummies`).
    HealDummies,
    /// The dummies' weapon lights on or off (`dummy::DummyLights`: their
    /// light, and its flare where it faces the camera).
    DummyLights,
    /// Forgets the kinds examined.
    ForgetExamined,
    /// The free camera on or off, and play watched from outside on or off
    /// (`main::switch_view`; F4 and F5 are the gestures' binds).
    FreeCamera,
    Watched,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    By(f32),
    Empty,
    Full,
}

/// A figure written beside its commands.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Readout {
    Hunger,
    Thirst,
    Legs,
    Arms,
    Breath,
    Durability,
    Heat,
    Malfunction,
    /// Beside the next shot's kinds: a word where the weapon allows none.
    NextShot,
    Examined,
    /// Beside the dummies' lights' button: on or off.
    DummyLights,
    Done,
}

/// What was done last, for the panel's line.
#[derive(Resource, Default)]
struct LastDone(String);

/// How far a heat command raises it.
const HEAT_STEP: f32 = 25.0;

/// What the health's command takes away: what hurts.
const HURTS: [Kind; 15] = [
    Kind::LightBleeding,
    Kind::HeavyBleeding,
    Kind::Wound,
    Kind::Fracture,
    Kind::Pain,
    Kind::Tremor,
    Kind::LowHealth,
    Kind::Exhaustion,
    Kind::Dehydration,
    Kind::ChronicFatigue,
    Kind::Contusion,
    Kind::Stun,
    Kind::Flash,
    Kind::TunnelVision,
    Kind::Intoxication,
];

pub struct DebugMenuPlugin;

impl Plugin for DebugMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DebugCommand>().init_resource::<LastDone>().add_systems(Update, (press, carry_out, read_out).chain());
    }
}

/// The commands' section of the panel, under its own heading.
pub fn spawn_commands(popup: &mut ChildSpawnerCommands) {
    let group = |popup: &mut ChildSpawnerCommands, name: &str| {
        popup.spawn((Node { margin: UiRect::top(Val::Px(6.0)), ..default() }, words(name, 10.0, FAINT)));
    };
    let row = |popup: &mut ChildSpawnerCommands, name: &str, readout: Option<Readout>, buttons: &[(&str, DebugCommand)]| {
        popup.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|line| {
            line.spawn(Node { width: Val::Px(96.0), flex_shrink: 0.0, ..default() }).with_children(|cell| {
                cell.spawn(words(name, 11.0, WORDS));
            });
            // The figure's column kept where there is none, so that the
            // buttons stand in one column.
            line.spawn(Node { width: Val::Px(118.0), flex_shrink: 0.0, ..default() }).with_children(|cell| {
                if let Some(readout) = readout {
                    cell.spawn((readout, words("", 11.0, WORDS), TextLayout::new_with_no_wrap()));
                }
            });
            for (label, command) in buttons {
                let node = Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() };
                line.spawn((*command, Button, node, BackgroundColor(PANEL))).with_children(|inside| {
                    inside.spawn((words(*label, 11.0, WORDS), TextLayout::new_with_no_wrap()));
                });
            }
        });
    };
    let changes = |make: fn(Change) -> DebugCommand| [("-10", make(Change::By(-10.0))), ("+10", make(Change::By(10.0))), ("EMPTY", make(Change::Empty)), ("FULL", make(Change::Full))];
    group(popup, "CHARACTER");
    row(popup, "Hunger", Some(Readout::Hunger), &changes(DebugCommand::Hunger));
    row(popup, "Thirst", Some(Readout::Thirst), &changes(DebugCommand::Thirst));
    row(popup, "Legs", Some(Readout::Legs), &changes(DebugCommand::Legs));
    row(popup, "Arms", Some(Readout::Arms), &changes(DebugCommand::Arms));
    row(popup, "Breath", Some(Readout::Breath), &changes(DebugCommand::Breath));
    group(popup, "HEALTH");
    row(popup, "Health", None, &[("HEAL ALL", DebugCommand::HealAll), ("HURT THE NEXT PART", DebugCommand::HurtNext), ("DIE", DebugCommand::Die)]);
    group(popup, "WEAPON IN HAND");
    row(popup, "Durability", Some(Readout::Durability), &[("-10", DebugCommand::Durability(-10.0)), ("+10", DebugCommand::Durability(10.0))]);
    row(popup, "Heat", Some(Readout::Heat), &[("+25", DebugCommand::HeatUp), ("COOL", DebugCommand::Cool)]);
    row(popup, "Magazine", None, &[("FILL", DebugCommand::FillMagazine)]);
    row(popup, "Malfunction", Some(Readout::Malfunction), &[("CLEAR", DebugCommand::ClearMalfunction)]);
    row(
        popup,
        "Next shot",
        Some(Readout::NextShot),
        &[
            ("MISFIRE", DebugCommand::MalfunctionNext(Malf::Misfire)),
            ("JAM", DebugCommand::MalfunctionNext(Malf::Jam)),
            ("FEED", DebugCommand::MalfunctionNext(Malf::Feed)),
            ("SOFT SLIDE", DebugCommand::MalfunctionNext(Malf::SoftSlide)),
            ("HARD SLIDE", DebugCommand::MalfunctionNext(Malf::HardSlide)),
        ],
    );
    group(popup, "WORLD");
    row(popup, "Dummies", None, &[("HEAL AND STAND UP", DebugCommand::HealDummies)]);
    row(popup, "Dummies' lights", Some(Readout::DummyLights), &[("LIGHTS", DebugCommand::DummyLights)]);
    row(popup, "View", None, &[("FREE CAMERA", DebugCommand::FreeCamera), ("WATCHED", DebugCommand::Watched)]);
    group(popup, "INVENTORY");
    row(popup, "Examined", Some(Readout::Examined), &[("FORGET", DebugCommand::ForgetExamined)]);
    popup.spawn((Node { margin: UiRect::top(Val::Px(6.0)), ..default() },)).with_children(|line| {
        line.spawn((Readout::Done, words("", 10.0, FAINT), TextLayout::new_with_no_wrap()));
    });
}

/// A button pressed sends its command, once a press of the left button
/// (the engine's `Interaction` can come back to pressed within one press);
/// one under the mouse is lit, and the trigger is told to leave that press
/// alone (`Controller::on_debug_button`).
fn press(mouse: Res<ButtonInput<MouseButton>>, mut buttons: Query<(&Interaction, &DebugCommand, &mut BackgroundColor)>, mut sent: MessageWriter<DebugCommand>, mut controller: ResMut<Controller>) {
    let down = mouse.just_pressed(MouseButton::Left);
    let on = buttons.iter().any(|(interaction, ..)| *interaction != Interaction::None);
    if controller.on_debug_button != on {
        controller.on_debug_button = on;
    }
    for (interaction, command, mut colour) in &mut buttons {
        let look = match interaction {
            Interaction::Pressed => {
                if down {
                    sent.write(*command);
                }
                WORDS.with_alpha(0.35)
            }
            Interaction::Hovered => LINE,
            Interaction::None => PANEL,
        };
        if colour.0 != look {
            colour.0 = look;
        }
    }
}

/// Changes a figure kept as current, least and most.
fn change(figure: &mut [f32; 3], how: Change) {
    let [_, least, most] = *figure;
    figure[0] = match how {
        Change::By(by) => (figure[0] + by).clamp(least, most),
        Change::Empty => least,
        Change::Full => most,
    };
}

/// Each command sent this frame, carried out. (The hurt, the death and the
/// dummies' are their own systems', which read the same messages:
/// `health_view::damage_keys`, `dummy::dress_dummies`.)
#[allow(clippy::too_many_arguments)]
fn carry_out(
    mut commands: MessageReader<DebugCommand>,
    mut controller: ResMut<Controller>,
    mut carried: ResMut<Carried>,
    held: Res<HandsHeld>,
    mut weapon: Option<ResMut<HandsRig>>,
    game: Res<crate::GameData>,
    mut done: ResMut<LastDone>,
) {
    for command in commands.read().copied() {
        let firearm = weapon.as_deref_mut().and_then(|rig| rig.firearm.as_mut());
        let said = match command {
            DebugCommand::Hunger(how) | DebugCommand::Thirst(how) => {
                let Some(health) = controller.health.as_mut() else { continue };
                let figure = if matches!(command, DebugCommand::Hunger(_)) { &mut health.energy } else { &mut health.hydration };
                change(figure, how);
                format!("{command:?}")
            }
            DebugCommand::Legs(how) | DebugCommand::Arms(how) | DebugCommand::Breath(how) => {
                let stamina = &mut controller.movement.stamina;
                let pool = match command {
                    DebugCommand::Legs(_) => &mut stamina.legs,
                    DebugCommand::Arms(_) => &mut stamina.arms,
                    _ => &mut stamina.oxygen,
                };
                let mut figure = [pool.current, 0.0, pool.capacity];
                change(&mut figure, how);
                pool.current = figure[0];
                format!("{command:?}")
            }
            DebugCommand::HealAll => {
                let Some(health) = controller.health.as_mut().filter(|health| health.alive) else { continue };
                for part in &mut health.parts {
                    (part.current, part.destroyed) = (part.maximum, false);
                }
                let gone: Vec<u32> = health.effects.iter().filter(|effect| HURTS.contains(&effect.kind)).map(|effect| effect.id).collect();
                for id in &gone {
                    health.force_remove(*id);
                }
                format!("HealAll: every part whole, {} effects taken off", gone.len())
            }
            DebugCommand::Durability(by) => {
                let Some(item) = held.item.and_then(|id| carried.equipment.item.find_mut(id)) else { continue };
                let Some(kept) = item.repairable.as_mut() else { continue };
                kept.durability = (kept.durability + by).clamp(0.0, kept.max_durability);
                format!("Durability: {:.1} of {:.1}", kept.durability, kept.max_durability)
            }
            DebugCommand::HeatUp | DebugCommand::Cool => {
                let Some(firearm) = firearm else { continue };
                let now = firearm.wear().map_or(0.0, |wear| wear.heat());
                firearm.set_heat(if command == DebugCommand::Cool { 0.0 } else { now + HEAT_STEP });
                format!("{command:?}")
            }
            DebugCommand::FillMagazine => {
                let Some(weapon_id) = held.item else { continue };
                let own_round = carried
                    .equipment
                    .item
                    .find(weapon_id)
                    .and_then(|weapon| game.db.items.get(&weapon.kind.template)?.props.get("defAmmo")?.as_str().map(str::to_owned))
                    .and_then(|round| trp_sim::items::inventory::Kind::of(&game.db, &round).ok())
                    .map(std::sync::Arc::new);
                let Some(weapon_item) = carried.equipment.item.find_mut(weapon_id) else { continue };
                let Some(slot) = weapon_item.slot_index(trp_sim::items::inventory::MAGAZINE_SLOT) else { continue };
                let Some(magazine) = weapon_item.slots[slot].as_mut() else { continue };
                let capacity = magazine.kind.capacity;
                // More of the rounds it holds; empty, the weapon's own.
                if trp_sim::items::inventory::magazine_rounds(magazine) > 0 {
                    trp_sim::items::inventory::set_magazine_rounds(magazine, capacity);
                } else if let Some(round) = own_round {
                    trp_sim::items::inventory::fill_magazine(magazine, round, capacity);
                } else {
                    continue;
                }
                let top = trp_sim::items::inventory::magazine_round_at(magazine, 0);
                let mut chambered = false;
                if let Some(firearm) = firearm.filter(|firearm| firearm.single_chamber()) {
                    firearm.magazine = Some(capacity);
                    // And an empty chamber its top round, where the chamber
                    // takes it. The item
                    // is told at once; the hands keep it so after.
                    let takes = top.as_ref().is_some_and(|round| {
                        carried.equipment.item.find(weapon_id).and_then(|weapon| trp_sim::items::inventory::chamber_takes(weapon, round)) == Some(true)
                    });
                    if takes && firearm.chamber_outright() {
                        trp_sim::items::inventory::set_chambered(&mut carried.equipment.item, weapon_id, top);
                        chambered = true;
                    }
                }
                format!("FillMagazine: {capacity} rounds{}", if chambered { ", one chambered" } else { "" })
            }
            DebugCommand::ClearMalfunction => {
                let Some(firearm) = firearm else { continue };
                firearm.clear_malfunction();
                format!("{command:?}")
            }
            DebugCommand::MalfunctionNext(malf) => {
                let Some(firearm) = firearm else { continue };
                if firearm.force_malfunction(malf) {
                    format!("{malf:?} on the next shot")
                } else {
                    format!("{malf:?}: this weapon cannot have it in the game")
                }
            }
            DebugCommand::HurtNext | DebugCommand::Die | DebugCommand::HealDummies | DebugCommand::DummyLights | DebugCommand::FreeCamera | DebugCommand::Watched => format!("{command:?}"),
            // Back to what the character began knowing, kept so, and every
            // view drawn anew.
            DebugCommand::ForgetExamined => {
                let forgotten = trp_sim::items::examine::forget();
                crate::screens::examine_view::keep(&crate::screens::options_view::options_file());
                carried.set_changed();
                format!("{forgotten} kinds examined forgotten")
            }
        };
        info!("debug panel: {said}");
        done.0 = format!("Done: {said}");
    }
}

/// The figures beside the commands, written while the panel is open.
fn read_out(
    shown: Option<Res<crate::panels::hud::GuideShown>>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    weapon: Option<Res<HandsRig>>,
    done: Res<LastDone>,
    dummy_lights: Option<Res<crate::world::dummy::DummyLights>>,
    mut readouts: Query<(&Readout, &mut Text)>,
    mut buttons: Query<(&DebugCommand, &mut Node)>,
) {
    if !shown.is_some_and(|shown| shown.0) {
        return;
    }
    let health = controller.health.as_ref();
    let firearm = weapon.as_deref().and_then(|rig| rig.firearm.as_ref());
    // The next shot's kinds the weapon in hand can have, the rest hidden.
    const KINDS: [Malf; 5] = [Malf::Misfire, Malf::Jam, Malf::Feed, Malf::SoftSlide, Malf::HardSlide];
    let any = firearm.map(|firearm| KINDS.iter().any(|malf| firearm.malfunction_allowed(*malf)));
    for (command, mut node) in &mut buttons {
        let DebugCommand::MalfunctionNext(malf) = command else { continue };
        let display = if firearm.is_some_and(|firearm| firearm.malfunction_allowed(*malf)) { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    let item = held.item.and_then(|id| carried.equipment.item.find(id));
    let stamina = &controller.movement.stamina;
    for (readout, mut text) in &mut readouts {
        let said = match readout {
            Readout::Hunger => health.map_or_else(String::new, |h| format!("{:.1} / {:.0}", h.energy[0], h.energy[2])),
            Readout::Thirst => health.map_or_else(String::new, |h| format!("{:.1} / {:.0}", h.hydration[0], h.hydration[2])),
            Readout::Legs => format!("{:.0} / {:.0}", stamina.legs.current, stamina.legs.capacity),
            Readout::Arms => format!("{:.0} / {:.0}", stamina.arms.current, stamina.arms.capacity),
            Readout::Breath => format!("{:.0} / {:.0}", stamina.oxygen.current, stamina.oxygen.capacity),
            Readout::Durability => item.and_then(|item| item.repairable).map_or_else(|| "none in hand".to_owned(), |kept| format!("{:.1} / {:.1}", kept.durability, kept.max_durability)),
            Readout::Heat => firearm.and_then(|firearm| firearm.wear()).map_or_else(String::new, |wear| format!("{:.1}", wear.heat())),
            Readout::Malfunction => firearm.and_then(|firearm| firearm.wear()).map_or_else(String::new, |wear| format!("{:?}", wear.state.malf)),
            Readout::NextShot => match any {
                None => "none in hand".to_owned(),
                Some(false) => "none in the game".to_owned(),
                Some(true) => String::new(),
            },
            Readout::Examined if !trp_sim::items::examine::on() => "all known (EXAMINING off)".to_owned(),
            Readout::Examined => format!("{} learned", trp_sim::items::examine::learned().len()),
            Readout::DummyLights => match dummy_lights.as_deref() {
                Some(lights) if lights.on => "on".to_owned(),
                Some(_) => "off".to_owned(),
                None => "no dummies".to_owned(),
            },
            Readout::Done => done.0.clone(),
        };
        if **text != said {
            **text = said;
        }
    }
}
