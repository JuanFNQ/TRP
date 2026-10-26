//! The debug panel's commands (`F1`; the user, 2026-10-26: "redo the
//! debug menu, get rid of all the controls, and add more f1 commands like
//! regain hunger/thirst or regen it, drain stamina or regen, raise or lower
//! weapon durability and any others you might think of"; by the panel's
//! buttons, the mouse freed while it is open). The app's tools for trying
//! things out, none of them the game's: what each does is written on it.
//!
//! - **Character**: hunger and thirst, and the stamina (the legs', the
//!   arms' and the breath's pools together), each down or up by ten, empty
//!   or full; its figure beside it.
//! - **Health**: every part whole and what hurts it gone (bleedings,
//!   fractures, wounds, pain, tremor, the low health, exhaustion and
//!   dehydration, chronic fatigue, a contusion, a stun, a flash, the
//!   tunnel vision; what was taken on purpose stays: painkillers, medicine
//!   at work, stimulators and their buffs); a bullet's hurt to the next
//!   part, and death (`F7` and `F8` until 0.141.0: the user, 2026-10-26,
//!   "remove the f7.f8, and f9 binds just leave them as commands in the new
//!   debug meun").
//! - **The weapon in hand**: its durability down or up by ten (its
//!   greatest the cap), its heat up by 25 or cooled, its magazine filled
//!   (its own round, else the weapon's own), its malfunction cleared, or one
//!   of the five given to its next shot.
//! - **World**: the dummies dressed as the player and made whole (`F9`
//!   until 0.141.0).
//!
//! While the panel is open the mouse moves no view and fires nothing
//! (`keys::hear`); the keyboard still plays.

use bevy::prelude::*;
use trp_sim::health::Kind;
use trp_sim::malfunction::Malf;

use crate::controller::Controller;
use crate::inventory_view::{words, Carried, HandsHeld, FAINT, LINE, PANEL, WORDS};
use crate::weapon::HandsRig;

/// One of the panel's commands.
#[derive(Message, Component, Clone, Copy, Debug, PartialEq)]
pub enum DebugCommand {
    Hunger(Change),
    Thirst(Change),
    Stamina(Change),
    HealAll,
    HurtNext,
    Die,
    Durability(f32),
    HeatUp,
    Cool,
    FillMagazine,
    ClearMalfunction,
    MalfunctionNext(Malf),
    DressDummies,
}

/// How a figure is changed: by so much, or to its least or its most.
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
    Stamina,
    Durability,
    Heat,
    Malfunction,
    Done,
}

/// What was done last, for the panel's line.
#[derive(Resource, Default)]
struct LastDone(String);

/// How far a heat command raises it.
const HEAT_STEP: f32 = 25.0;

/// What the health's command takes away: what hurts.
const HURTS: [Kind; 14] = [
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
    row(popup, "Stamina", Some(Readout::Stamina), &changes(DebugCommand::Stamina));
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
        None,
        &[
            ("MISFIRE", DebugCommand::MalfunctionNext(Malf::Misfire)),
            ("JAM", DebugCommand::MalfunctionNext(Malf::Jam)),
            ("FEED", DebugCommand::MalfunctionNext(Malf::Feed)),
            ("SOFT SLIDE", DebugCommand::MalfunctionNext(Malf::SoftSlide)),
            ("HARD SLIDE", DebugCommand::MalfunctionNext(Malf::HardSlide)),
        ],
    );
    group(popup, "WORLD");
    row(popup, "Dummies", None, &[("DRESS AS YOU, MADE WHOLE", DebugCommand::DressDummies)]);
    popup.spawn((Node { margin: UiRect::top(Val::Px(6.0)), ..default() },)).with_children(|line| {
        line.spawn((Readout::Done, words("", 10.0, FAINT), TextLayout::new_with_no_wrap()));
    });
}

/// A button pressed sends its command, once a press of the left button
/// (the engine's `Interaction` can come back to pressed within one press);
/// one under the mouse is lit.
fn press(mouse: Res<ButtonInput<MouseButton>>, mut buttons: Query<(&Interaction, &DebugCommand, &mut BackgroundColor)>, mut sent: MessageWriter<DebugCommand>) {
    let down = mouse.just_pressed(MouseButton::Left);
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
            DebugCommand::Stamina(how) => {
                let stamina = &mut controller.movement.stamina;
                for pool in [&mut stamina.legs, &mut stamina.arms, &mut stamina.oxygen] {
                    let mut figure = [pool.current, 0.0, pool.capacity];
                    change(&mut figure, how);
                    pool.current = figure[0];
                }
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
                    .and_then(|round| trp_sim::inventory::Kind::of(&game.db, &round).ok())
                    .map(std::sync::Arc::new);
                let Some(weapon_item) = carried.equipment.item.find_mut(weapon_id) else { continue };
                let Some(slot) = weapon_item.slot_index(trp_sim::inventory::MAGAZINE_SLOT) else { continue };
                let Some(magazine) = weapon_item.slots[slot].as_mut() else { continue };
                let capacity = magazine.kind.capacity;
                // More of the rounds it holds; empty, the weapon's own.
                if trp_sim::inventory::magazine_rounds(magazine) > 0 {
                    trp_sim::inventory::set_magazine_rounds(magazine, capacity);
                } else if let Some(round) = own_round {
                    trp_sim::inventory::fill_magazine(magazine, round, capacity);
                } else {
                    continue;
                }
                if let Some(firearm) = firearm.filter(|firearm| firearm.single_chamber()) {
                    firearm.magazine = Some(capacity);
                }
                format!("FillMagazine: {capacity} rounds")
            }
            DebugCommand::ClearMalfunction => {
                let Some(firearm) = firearm else { continue };
                firearm.clear_malfunction();
                format!("{command:?}")
            }
            DebugCommand::MalfunctionNext(malf) => {
                let Some(firearm) = firearm else { continue };
                firearm.force_malfunction(malf);
                format!("{malf:?} on the next shot")
            }
            DebugCommand::HurtNext | DebugCommand::Die | DebugCommand::DressDummies => format!("{command:?}"),
        };
        info!("debug panel: {said}");
        done.0 = format!("Done: {said}");
    }
}

/// The figures beside the commands, written while the panel is open.
fn read_out(
    shown: Option<Res<crate::hud::GuideShown>>,
    controller: Res<Controller>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    weapon: Option<Res<HandsRig>>,
    done: Res<LastDone>,
    mut readouts: Query<(&Readout, &mut Text)>,
) {
    if !shown.is_some_and(|shown| shown.0) {
        return;
    }
    let health = controller.health.as_ref();
    let firearm = weapon.as_deref().and_then(|rig| rig.firearm.as_ref());
    let item = held.item.and_then(|id| carried.equipment.item.find(id));
    let stamina = &controller.movement.stamina;
    for (readout, mut text) in &mut readouts {
        let said = match readout {
            Readout::Hunger => health.map_or_else(String::new, |h| format!("{:.1} / {:.0}", h.energy[0], h.energy[2])),
            Readout::Thirst => health.map_or_else(String::new, |h| format!("{:.1} / {:.0}", h.hydration[0], h.hydration[2])),
            Readout::Stamina => format!("legs {:.0}  arms {:.0}", stamina.legs.current, stamina.arms.current),
            Readout::Durability => item.and_then(|item| item.repairable).map_or_else(|| "none in hand".to_owned(), |kept| format!("{:.1} / {:.1}", kept.durability, kept.max_durability)),
            Readout::Heat => firearm.and_then(|firearm| firearm.wear()).map_or_else(String::new, |wear| format!("{:.1}", wear.heat())),
            Readout::Malfunction => firearm.and_then(|firearm| firearm.wear()).map_or_else(String::new, |wear| format!("{:?}", wear.state.malf)),
            Readout::Done => done.0.clone(),
        };
        if **text != said {
            **text = said;
        }
    }
}
