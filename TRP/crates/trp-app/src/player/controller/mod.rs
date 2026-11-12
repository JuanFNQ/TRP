//! Drives the simulation from the keyboard and mouse and keeps what is to be drawn of it.
//! That is the player's place, the frame the view hangs in, the
//! weapon's moved bone and the camera.
//!
//! The simulation runs on frames of its own length (`trp_sim::Pacer`), so
//! nothing in it depends on the rate of drawing. What is drawn is the
//! simulation as it stood one physics step before the host's time: what
//! the springs move (the weapon, the camera) blended between the last two
//! of its frames that had a physics step, and what the movement and the
//! body's animation move (the feet's place, the ribcage's, the skeleton)
//! between the two of its frames either side of that moment. The view's
//! own turning is shown at once.

use std::collections::VecDeque;


use bevy::prelude::*;
use trp_sim::base::math::{delta_angle, quat_axis, quat_lerp, quat_mul, quat_rotate};
use trp_sim::{
    AimPoint, ArmPose, CameraPose, FireMode, FirearmEvent, GrenadeEvent, HandsInput, Input as SimInput, KnifeEvent, MedsEvent, Movement, Pacer, PointOfView, RangefinderEvent, State,
};

use crate::world::range::Range;
use crate::weapons::weapon::{HandsRig, Player, RigPose, ViewPivot, WeaponRig};
use crate::{platform::convert, Options, View};

mod pose_parts;
mod showing;
mod input;
mod stepping;

pub use pose_parts::*;
#[cfg(test)]
pub(crate) use showing::*;
pub use input::*;
pub use stepping::*;
use pose_parts::{AIM_KIT, MountDrawn, NO_TURN, Pending, Sprung, Stepped, blend_arm, look_into_chamber, rig_in_world};

#[derive(Resource, Clone)]
pub struct Controller {
    pub movement: Movement,
    /// Where the character stood as this frame began, before its move: what
    /// a blast of this frame is measured from, as the game measures it
    /// (`blast_view`; recorded, `courses8`'s `zarya-behind-player`).
    pub feet_before_move: [f32; 3],
    /// The character's health (`docs/spec/health.md`), with the options'
    /// `HEALTH SYSTEM` on: stepped after the movement each frame, which it
    /// tells what it allows.
    pub health: Option<trp_sim::condition::health::Health>,
    /// What the last drawn frame ran of the simulation.
    pub paced: Paced,
    pending: Pending,
    pacer: Pacer,
    /// What the springs left of the last two frames of the simulation
    /// that had a physics step.
    before: Sprung,
    now: Sprung,
    /// What the movement and the body left of its last few frames, the
    /// newest last: enough to reach back past the moment drawn.
    stepped: VecDeque<Stepped>,
    /// Dead: what is drawn is the newest frame alone, its rig on the
    /// animator's straight pose, nothing between frames and nothing smooth,
    /// so that the corpse starts from the simulation's own pose whatever the
    /// drawing (`death::die`: a drawing option moves no result).
    cut: bool,
    /// The view as shown on the last drawn frames, each with the host's
    /// time: yaw and pitch. And where the ribcage is drawn for the newest
    /// (`Shown::ribcage`).
    looks: VecDeque<(f32, [f32; 2])>,
    ribcage_shown: Vec3,
    /// The same ribcage from the feet in the world's frame and the game's
    /// axes, the view it is shown for, and the view shown one of the
    /// simulation's frames before.
    ribcage_drawn: ([f32; 3], [f32; 2], [f32; 2]),
    /// Whether the aim key was down when last looked at, in a run that
    /// plays a script.
    aim_key: bool,
    /// How the trigger stands: the fire binding given turns it over and
    /// taken back lets it go (`FirearmHandsInputTranslator`).
    trigger: bool,
    /// How the vault key stands: its binding given or taken back turns
    /// it over, which is all the game makes of either (`Player.Vaulting`).
    vault_key: bool,
    /// What the keys push the two movement axes to, right and forward.
    move_keys: [f32; 2],
    /// The slow lean's axis as the keys push it, kept as the walk's are.
    slow_lean: f32,
    /// How deep the weapon would stick into something, in metres, as the
    /// last frame measured it, and whether that is another character.
    pub overlap: f32,
    pub overlap_with_character: bool,
    /// The box that keeps the camera out of the world, from the player
    /// prefab, if it could be read.
    pub camera_box: Option<trp_data::character::player::CameraBoxData>,
    /// The gear's share of the weapon's ergonomics (the armour's and the
    /// backpack's `WeaponErgonomicPenalty` over 100; `health_view::armour_slows`
    /// works it out as what is worn changes), handed to the weapon's motion
    /// with the load every frame (`Hands::set_load`).
    pub gear_ergonomics: f32,
    /// The inventory screen is open: the mouse is the screen's, so the
    /// view does not turn and the mouse's buttons do nothing.
    pub inventory_open: bool,
    /// The edit build screen is open (`edit_build`): the same, and the
    /// character is handed nothing, as under any screen.
    pub edit_build: bool,
    /// The mouse is on one of the debug panel's buttons (`debug_menu`): a
    /// press of the left button there is the button's, not the trigger's
    /// (FILL, which chambers a round, would fire it at once).
    pub on_debug_button: bool,
    /// The gestures' menu is open (`gestures`): the cursor freed and the
    /// world blurred; what the character hears is filtered by the bindings
    /// themselves (`Bindings::only`).
    pub gestures_open: bool,
    /// The gestures' held list is up: the action panel hides.
    pub gestures_list: bool,
    /// The situations the hands raised this frame for the quick prompt
    /// (`Player.PhraseSituation`, 5 s each): a reload begun, a grenade's
    /// pin pulled, a reload with no magazine found, a known malfunction.
    pub phrase_situations: Vec<i32>,
    /// Whether there is a weapon in the hands: the one on the sling slot
    /// of the equipment (inventory spec, "The weapon in hand"). With none
    /// the weapon's keys do nothing and its rig is not drawn.
    pub armed: bool,
    /// What the simulation takes from the rig in the hands, as its
    /// animator left it on the last frame.
    rig_pose: RigPose,
    /// Whether the weapon's motion and the arms go by an underbarrel
    /// launcher's bones (`trp_sim::Firearm::launcher_bones_in_use`): the left
    /// elbow then bends toward its goal (`crate::arms`).
    pub launcher_bones: bool,
    /// What the thing in the hands said of itself since it was last asked:
    /// that it has appeared, that it has been put away.
    hands_events: Vec<FirearmEvent>,
    /// What the weapon in hand said of its malfunctions since they were
    /// last taken (`crate::wear`): each malfunction, reminder, look,
    /// repair, the round a repair threw out, the heat's stray shot.
    pub wear_events: Vec<FirearmEvent>,
    /// The `Sound` marks the animator of what is in the hands passed since
    /// they were last played (`hands_sound`), each the mark's string.
    pub hands_sounds: Vec<String>,
    /// What the weapon asked of its sound player since it was last heard
    /// (`shot_sound`): its shots and its trigger let go, in order.
    pub shot_calls: Vec<crate::audio::shot_sound::ShotCall>,
    /// What a spent case is to do (`cases`): a round to the shell port at a
    /// shot, thrown at the `ShellEject` mark, for a weapon of one chamber;
    /// a case thrown from a chamber of its own's port.
    pub case_calls: Vec<crate::weapons::cases::CaseCall>,
    /// The rounds a reload of a weapon with chambers of its own has shown
    /// since they were last kept (`magazine_rounds::ChamberKinds`): each
    /// chamber, and which of the reload's rounds it took.
    pub rounds_shown: Vec<(usize, usize)>,
    /// The chambers of its own whose fired round has gone to the port as
    /// its case since they were last kept (`FirearmEvent::CaseLeft`): the
    /// case there is of that round's kind from now.
    pub cases_left: Vec<usize>,
    /// Each shot's call for its bullets (`bullets`): whether it was fired
    /// by automatic fire (the game's firing operation in the player's
    /// update, before the frame's bullets move on) or by the animation's
    /// event (after it), and whether a cylinder's hammer was down.
    pub bullet_calls: Vec<crate::weapons::bullets::BulletCall>,
    /// How often the aim's call has brought the sights up since the
    /// weapon's aiming sound was last played (`hands_sound`).
    pub aims_up: u32,
    /// The weapon's mount sound: how many are due to play (`hands_sound`),
    /// and the time left until the next (`WeaponSoundPlayer.
    /// EnterMountedStateHandler`'s timer, the approach's length, stopped by
    /// an exit; by the engine's clock).
    pub mount_sounds: u32,
    mount_sound_in: Option<f32>,
    /// The bipod's own sounds due (`hands_sound`, `BipodAudioController`):
    /// on the ground, the same timer's end after a forward mount (`PlayMountSound`,
    /// none at a side); opened (true) or folded by the hands' marks.
    pub bipod_grounds: u32,
    pub bipod_marks: Vec<bool>,
    mount_forward: bool,
    /// How many mount searches have found nothing or been refused
    /// (`OnNothingFound`: the stance panel's mount icon flashes red,
    /// `battle_view`).
    pub mount_refusals: u32,
    /// What health said of the items it was given to use since the
    /// inventory last asked: what one holds now, one used up.
    pub items_said: Vec<trp_sim::condition::health::Event>,
    /// A change of the hands just made, for a run told to write each
    /// (`hands_log`), until the simulation first steps the new hands.
    pub hands_change: Option<HandsChange>,
    /// The bleedings' damage since the screen last asked: the part, and
    /// whether heavy (`ApplyDamageEvent` of a bleeding kind: blood on the
    /// screen, `blood_view`).
    pub bled: Vec<(trp_sim::condition::health::Part, bool)>,
    /// A heavy bleeding's start (true) and its end or the death (false)
    /// since the drops on the ground last asked (`Player`'s
    /// `OnHealthEffectVisualAdded`, `…Removed`, `OnPlayerVisualDied`;
    /// `decal_view`).
    pub bleeding_drops: Vec<bool>,
    /// What health did since the voice last asked (`voice_sound`), kept
    /// once the voice runs.
    pub voice_heard: Option<Vec<trp_sim::condition::health::Event>>,
    /// How many rounds the character has within reach for the weapon in
    /// hand, as the inventory last counted them: what a tube is filled
    /// from. And how many the weapon has taken since the inventory last
    /// took them out of the pockets and the vest.
    pub rounds_in_reach: u32,
    pub rounds_taken: u32,
    /// For an underbarrel launcher: the rounds its chamber takes within
    /// reach, as the inventory last counted them; and since the inventory
    /// last looked, that its reload has taken one into its chamber, and that
    /// its round has been fired (`crate::inventory_view`).
    pub launcher_rounds_in_reach: u32,
    pub launcher_round_taken: bool,
    pub launcher_fired: bool,
    /// The strength of the launcher's last shot's recoil (1 and its round's
    /// `ammoRec` a hundredth: `FirearmController.LauncherShot`), as the
    /// inventory read it before the round left its item; and the launcher's
    /// shot sounds and its own hands' sound marks, for its own sound player
    /// (`crate::hands_sound`, `crate::shot_sound`).
    pub launcher_recoil: f32,
    pub launcher_shot_calls: Vec<crate::audio::shot_sound::ShotCall>,
    /// The rounds the launcher's shots fired this frame, by their templates,
    /// read off its item as each goes (`inventory_view::settle`), for their
    /// grenades (`bullets`).
    pub launcher_bullets: Vec<String>,
    pub launcher_sounds: Vec<String>,
    /// The weapon in hand's own magazine is not known, and a reload was
    /// refused for it this frame ("Attached magazine is not examined.":
    /// `Player.CanStartReload`; inventory spec, "Examining").
    pub magazine_unknown: bool,
    pub reload_refused_unknown: bool,
    /// How many chambered rounds the weapon has thrown out, unfired, since
    /// the inventory last put them on the ground.
    pub rounds_ejected: u32,
    /// How many chambered rounds the chamber's key has had the action throw
    /// out (the rack: firing spec, "The magazine's and the chamber's keys")
    /// since the inventory last took them out of the chamber.
    pub rounds_racked: u32,
    /// How many live rounds a single barrel's reload has taken out since
    /// the inventory last put them away or down.
    pub rounds_unloaded: u32,
    /// The chambers whose live rounds a cylinder's quick reload has thrown
    /// out since the inventory last put them down.
    pub rounds_dropped: Vec<usize>,
    /// The magazine within reach that a reload would take for the weapon
    /// in hand, with its rounds, its clips' index, whether it comes out
    /// and its rounds the weapon's chamber refuses (`inventory::
    /// unfit_rounds`), as the inventory last found it; the one
    /// the last reload's key chose; and what the weapon has said of its
    /// magazines since the inventory last heard: each exchange begun (the
    /// rounds left in the old one, and whether it was the quick way), and
    /// how many times an old one has left the hand (firing spec, "The
    /// quick reload").
    pub next_magazine: Option<(trp_sim::items::inventory::Id, u32, i32, bool, u32)>,
    /// What the magazine picker's choice asks of the hands this frame
    /// (`ammo_picker`), and whether the picker took the reload's key, which
    /// the hands then do not hear.
    pub picked: Option<crate::screens::ammo_picker::Picked>,
    pub picker_took_reload: bool,
    pub magazine_asked: Option<trp_sim::items::inventory::Id>,
    pub swaps: Vec<(Option<u32>, bool)>,
    pub magazines_hidden: u32,
    /// A magazine's move from the inventory the weapon's hands have done
    /// (`FirearmEvent::MagazineMoved`), counted.
    pub magazines_moved: u32,
    /// The reload key found no magazine to put in, so its last resort is
    /// asked of the inventory this frame (`LoadAmmoToChamber`: the
    /// inventory's `chamber_by_hand`).
    pub chamber_by_hand: bool,
    /// Folds the key has begun in the weapon's hands, counted: the item in
    /// hand folds with each as it begins (`FoldOperation`), which the
    /// inventory sees to.
    pub folds_begun: u32,
    /// Shots the weapon in hand has made, counted: the inventory sets its
    /// magazine beside them (`UncheckOnShot`).
    pub shots_made: u32,
    /// Every shot since the run began, never taken (the muzzle's flash
    /// counts its own from it: `muzzle_view`).
    pub shots_fired: u32,
    /// Every underbarrel launcher's shot likewise, which plays the
    /// launcher's own muzzle (`InitiateShot`: `_underbarrelContainer.
    /// FirearmsEffects`; `muzzle_view::fired_muzzles`).
    pub launcher_shots_fired: u32,
    /// The hands gave up putting a round into the chamber (put away
    /// meanwhile): the inventory undoes the move.
    pub chamber_given_up: bool,
    /// The switches on the head's sounds to play, by their places in
    /// `trp_data::sound::TOGGLE_SOUNDS` (`step_sound`).
    pub toggle_sounds: Vec<usize>,
    /// What the character last heard of the face shield, night vision and
    /// thermal vision on the head (`step_sound` hears each change).
    pub switch_sounds: trp_sim::effects::visor::SwitchSounds,
    /// Whether the weapon in hand draws its magazine, as its reload last
    /// said, if it has said so since this was last taken: not from the
    /// old one's leaving the hand (`OnMagPuttedToRig`, where the game
    /// takes it off the weapon) until the next appears (`OnMagAppeared`).
    pub magazine_drawn: Option<bool>,
    /// What the character did that earns its skills or its experience
    /// since the experience last read it (`crate::experience`): the
    /// weapon's reloads, chamberings, sights and malfunctions put right, a
    /// throw, and the health's events.
    pub happened: Vec<crate::player::experience::Happened>,
    /// A menu is shown for what is looked at: the wheel is the menu's and
    /// does not set the walking speed (`ActionPanel.TranslateCommand`).
    /// And the notches it has turned since the menu last took them, up
    /// the list counted as positive.
    pub menu_shown: bool,
    pub menu_notches: i32,
    /// Notches of the wheel a run nobody is at the keys of turned with the
    /// scope zoom's keys held (`--zoom`), up counted as positive, not yet
    /// taken by the simulation.
    pub zoom_notches: i32,
    /// The weapon's magazine has been checked in the hands, seen with the
    /// character's own eyes (`FirearmController.CheckAmmo`:
    /// `StrictCheckMagazine` in first person), until the inventory takes it.
    pub magazine_checked: bool,
    /// The weapon in hand's chamber has come to be known, until the
    /// inventory takes it (`InventoryController.CheckChamber(weapon, true)`):
    /// a look into the chamber begun with no malfunction (`CheckChamber`),
    /// a look at the magazine begun (`CheckAmmo`: the inventory asks whether
    /// the weapon is a revolver or has more than one chamber), a cylinder's
    /// reload shut (`ReloadCylinderMagazine`'s commit). In either view.
    pub chamber_checked: bool,
    pub magazine_looked: bool,
    /// What the hands tell the battle screen's panels at the lower right
    /// since they last heard (`sight_panels`).
    pub panel_calls: Vec<crate::panels::sight_panels::PanelCall>,
    /// A grenade in hand has been let go of in a throw since the inventory
    /// last heard: it is gone from what is carried. And the throws to be
    /// made in the world (`grenade`): from below or not.
    pub grenade_thrown: bool,
    pub throws: Vec<bool>,
    /// Where the last grenade thrown left the hand, worked out on the frame
    /// of the simulation that threw it (`GrenadeRelease`), for the throw to
    /// take (`loose::set_going`).
    pub grenade_release: Option<GrenadeRelease>,
    /// The knife's blade where the last frame of the simulation left it, in
    /// the world and the game's axes, the first person's squeeze off: what
    /// the next frame's sweep starts from (`trp_sim::BladeSweep`). And the
    /// world the stroke under way is swept through (`knife::StrokeWorld`:
    /// the range as the physics engine has it, made for the stroke, and the
    /// living dummies' hit colliders, put in on each live frame).
    blade_left: Option<([f32; 3], [f32; 4])>,
    stroke_world: crate::weapons::knife::StrokeSlot,
    /// The knife's blows on the dummies since they last took them
    /// (`dummy::blow_dummies`).
    pub blows: Vec<crate::weapons::knife::Blow>,
    /// The blade's meetings since their effects were last queued: what each
    /// struck plays its material's hit (`bullets::fly_bullets`), and
    /// whether the blade is one shown on the character's model, whose hit
    /// has a sound of its own.
    pub knife_hits: Vec<(crate::weapons::knife::Meeting, bool)>,
    /// Medical items that have started their work (`MedEffect` started),
    /// counted: what stops a magazine's loading (inventory spec).
    pub meds_started: u32,
    /// What of health raises the game's health panel in a raid
    /// (`BattleUIScreen`), counted: damage taken (`ApplyDamageEvent`) or an
    /// effect started that calls for the panel (`IEffectTriggersUIPanel`:
    /// chronic fatigue, encumbered, overencumbered and Immunity's notice
    /// of the app's), and an
    /// effect taken off (`EffectRemovedEvent`).
    pub health_pulls: [u32; 2],
    /// The player's two flags that bring a drawn weapon up aimed
    /// (`SpawnOperation.WeaponAppeared`: `ToggleAim`): the last slot taken
    /// by its key was taken by the holster's key pressed twice
    /// (`FastSlotSelection`, set by every slot's key: `inventory_view`), and
    /// the last weapon put away from rest went with its sights up
    /// (`PreviousWeaponAimState`: `FirearmEvent::AimKept`). Both last the
    /// whole run, whatever is in hand between.
    pub fast_slot: bool,
    pub aim_kept: bool,
}

#[cfg(test)]
mod rig_blend_tests;

/// Places the player and the frame the view hangs in.
pub fn apply(
    controller: Res<Controller>,
    rig: Option<Res<WeaponRig>>,
    mut player: Query<&mut Transform, (With<Player>, Without<ViewPivot>)>,
    mut pivot: Query<&mut Transform, (With<ViewPivot>, Without<Player>)>,
) {
    if rig.is_none() {
        return;
    }
    let shown = controller.shown();
    let (yaw, pitch) = controller.look();
    if let Ok(mut transform) = player.single_mut() {
        transform.translation = shown.position;
        // The simulation's yaw turns right when positive; Bevy's turns left.
        transform.rotation = Quat::from_rotation_y(-yaw.to_radians());
    }
    if let Ok(mut transform) = pivot.single_mut() {
        transform.translation = shown.ribcage;
        transform.rotation = Quat::from_rotation_x(-pitch.to_radians());
    }
}
