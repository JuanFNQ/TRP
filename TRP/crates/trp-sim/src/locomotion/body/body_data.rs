//! The body's animator data loaded from the install, and its ids.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Ids {
    pub(super) speed: Option<usize>,
    pub(super) direct_x: Option<usize>,
    pub(super) direct_y: Option<usize>,
    pub(super) direct: Option<usize>,
    pub(super) level: Option<usize>,
    pub(super) tilt: Option<usize>,
    pub(super) inert: Option<usize>,
    pub(super) inert_float: Option<usize>,
    pub(super) sprint: Option<usize>,
    pub(super) sprint_speed: Option<usize>,
    pub(super) sprint_inertia: Option<usize>,
    pub(super) jumping: Option<usize>,
    pub(super) landing: Option<usize>,
    pub(super) falling: Option<usize>,
    pub(super) grounded: Option<usize>,
    pub(super) prone: Option<usize>,
    pub(super) transition_speed: Option<usize>,
    pub(super) aim_angle: Option<usize>,
    pub(super) delta_rotation: Option<usize>,
    pub(super) is_aiming: Option<usize>,
    pub(super) prone_reload: Option<usize>,
    pub(super) reload_float: Option<usize>,
    pub(super) step: Option<usize>,
    pub(super) sidestep: Option<usize>,
    pub(super) sideback_speed: Option<usize>,
    pub(super) weapon_size: Option<usize>,
    pub(super) weapon_type: Option<usize>,
    pub(super) weapon_type_float: Option<usize>,
    pub(super) foot_step: Option<usize>,
    pub(super) weapon_3rd: Option<usize>,
    pub(super) position_cache: Option<usize>,
    pub(super) first_person: Option<usize>,
    pub(super) hand_right: Option<usize>,
    pub(super) hand_left: Option<usize>,
    pub(super) shoulder_right: Option<usize>,
    pub(super) shoulder_left: Option<usize>,
    pub(super) elbow_right: Option<usize>,
    pub(super) elbow_left: Option<usize>,
    pub(super) aiming_layer: Option<usize>,
    pub(super) vaulting_height: Option<usize>,
    pub(super) vaulting_length: Option<usize>,
    pub(super) over_obstacle: Option<usize>,
    pub(super) forward_velocity: Option<usize>,
    pub(super) do_vault: Option<usize>,
    pub(super) do_climb: Option<usize>,
    pub(super) is_vaulting: Option<usize>,
    pub(super) vaulting_speed: Option<usize>,
    pub(super) mouse_look: Option<usize>,
    pub(super) is_interaction: Option<usize>,
    pub(super) interaction_index: Option<usize>,
    pub(super) inventory: Option<usize>,
    pub(super) inventory_operation: Option<usize>,
    pub(super) kick: Option<usize>,
    pub(super) kick_succeed: Option<usize>,
    pub(super) approached: Option<usize>,
    pub(super) third_person: Option<usize>,
    pub(super) third_person_float: Option<usize>,
    pub(super) first_action: Option<usize>,
    pub(super) speed_draw: Option<usize>,
    pub(super) swing_speed: Option<usize>,
    pub(super) deflected: Option<usize>,
    pub(super) blind_fire: Option<usize>,
    pub(super) blind_fire_float: Option<usize>,
    pub(super) prone_bipods: Option<usize>,
    pub(super) left_stance: Option<usize>,
    pub(super) force_left_stance: Option<usize>,
    pub(super) left_stance_curve: Option<usize>,
}

/// The two layers of the body's animator the game weighs by who is
/// looking, by the numbers its code has for them: the body's own part in
/// what the hands do with a weapon (`Weapon`), off in first person, and
/// the reach for the vest (`UtilityOperation`), weighed each frame for a
/// character seen from outside (`MovementContext.PlayerAnimatorPointOfView`,
/// `Player.AdjustUtilityLayerWeight`).
pub(super) const WEAPON_LAYER: usize = 1;

pub(super) const UTILITY_LAYER: usize = 2;

/// How near the left hand's marker has to come to the weapon's marker for
/// the vest for that reach to begin, and how near for all of it, in
/// metres; and the share of its weight the layer loses a second once no
/// reload is in hand (`Player.UtilityLayerRange`, `UtilityLayerLerpSpeed`:
/// figures of the game's code, not of the install).
pub(super) const UTILITY_RANGE: [f32; 2] = [0.5, 0.2];

pub(super) const UTILITY_FADE: f32 = 3.0;

/// The body's controller, read once and shared by every body made from it.
#[derive(Clone)]
pub struct BodyData {
    pub(super) controller: Arc<Controller>,
    pub(super) ids: Ids,
    /// The layer the game fades by the `Aiming_Layer` curve.
    pub(super) faded_layer: usize,
    /// The length written on the sidestep's way in.
    pub(super) sidestep_length: f32,
    /// How many further bones a host asked to have posed, and which of
    /// them, if any, is the root the clips move along the ground.
    pub(super) drawn: usize,
    pub(super) drawn_root: Option<usize>,
    /// When in its state the gesture of a backpack's drop has the mark
    /// for the pack leaving the shoulders, as a share of the state.
    pub(super) backpack_drop: Option<f32>,
    /// When in each of the bipod's states its toggle's mark comes.
    pub(super) bipod_marks: [Option<f32>; 4],
    /// When in its state each move over an obstacle has each of its four
    /// sound marks, as a share of the state (`VAULT_STATES`, `VAULT_MARKS`).
    pub(super) vault_marks: [[Option<f32>; 4]; 3],
    /// Which of the further bones asked for are the neck and the head,
    /// and the share of the free look's turn each takes
    /// (`PlayerBones.NeckChain`, `HeadChain`).
    pub(super) head_bones: [Option<usize>; 2],
    pub(super) head_shares: [f32; 2],
    /// Blind fire's curves by the state's time (`SmoothLerpAnimCurve`), by
    /// part (the way in, the way out) and side (over the top, to the right).
    pub(super) blind_curves: [[Option<Curve>; 2]; 2],
    /// How far the limbs reach at rest (`limbs_reach`).
    pub(super) reach: [f32; 3],
}

/// The neck and the head, by the ends of their paths on the skeleton.
const HEAD_BONES: [&str; 2] = ["/Base HumanNeck", "/Base HumanNeck/Base HumanHead"];

/// The turn under which the game leaves the head as the animation has it:
/// a tenth of a degree, as the length of the turn's three angles.
pub(super) const HEAD_TURN_LEAST: f32 = 0.01;

/// The layer of the body's animator the game's own screens stand the
/// character by, and the parameters it is set by: the kind of thing in the
/// hands, as a whole number and twice as a figure, -1 for none
/// (`PlayerModelLoader.CreateWeapon`: the layer at index 11 given a weight
/// of 1). Its one state, `GUI.GUI_IDLE`, blends by the last of them, and
/// at -1 plays `inv_idle_hands_down`.
const STANDING_LAYER: &str = "GUI";

const STANDING_KIND: [&str; 3] = ["WeaponType", "WeaponTypeFloat", "UI_WeaponType"];

const NOTHING_IN_HANDS: f32 = -1.0;

/// A pose of the bones a host named, each one's place and turning in its
/// parent, in the order named.
pub type Pose = Vec<([f32; 3], [f32; 4])>;

/// The state of the interactions' layer for a backpack's drop, and the
/// mark in it the game's logic waits for.
pub(super) const BACKPACK_DROP: &str = "InteractionsLayer.10 Backpack Drop";

const BACKPACK_DROP_MARK: &str = "OnBackpackDrop";

/// The interactions layer's four states for the bipod, and the mark in each
/// the bipod's toggle ends on (`probe bodystates bipod`: at 0.5 putting it
/// out, 0.6 folding it; mounting spec, "The bipod").
pub(super) const BIPOD_STATES: [&str; 4] = [
    "InteractionsLayer.31 Bipod Forward On",
    "InteractionsLayer.32 Bipod Forward Off",
    "InteractionsLayer.33 Bipod Backward On ",
    "InteractionsLayer.34 Bipod Backward Off",
];

const BIPOD_MARK: &str = "OnBipodToggle";

/// The three moves over an obstacle, by the states that play them, in the
/// order of the game's `EVaultingSoundType` less its none: a vault, a
/// sprint's vault, a climb (footsteps spec, "The vault's sounds").
pub const VAULT_STATES: [&str; 3] = ["Base Layer.Vaulting.Vault", "Base Layer.Vaulting.Sprint_vault", "Base Layer.Vaulting.Climb_on"];

/// Each move's four marks, events of its state (`IVaultingSoundsEvents`):
/// start, grab, slide, end, in the order of `EVaultingSoundState`.
pub const VAULT_MARKS: [[&str; 4]; 3] = [
    ["OnVaultStart", "OnVaultGrab", "OnVaultSlide", "OnVaultEnd"],
    ["OnSprintVaultStart", "OnSprintVaultGrab", "OnSprintVaultSlide", "OnSprintVaultEnd"],
    ["OnClimbStart", "OnClimbGrab", "OnClimbSlide", "OnClimbEnd"],
];

impl BodyData {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        Self::load_with(assets, &[])
    }

    /// As `load`, with further bones for the animator to pose, each named
    /// by its path below the animated object: a host that draws the body
    /// names the bones its meshes hang on, and `Body::drawn_bone` gives
    /// each as the animation has it.
    pub fn load_with(assets: &mut Assets, drawn: &[String]) -> Result<Self> {
        Ok(Self::load_standing(assets, drawn, &[])?.0)
    }

    /// As `load_with`, and with it the character as the game's own screens
    /// stand it with nothing in its hands (`standing`): the pose of every
    /// bone of `figure`, each named by its path below the animated object,
    /// or why it could not be had. Nothing more is read when `figure` is
    /// empty, and the body's own animator is the same either way.
    pub fn load_standing(assets: &mut Assets, drawn: &[String], figure: &[String]) -> Result<(Self, Option<Result<Pose>>)> {
        let table = trp_data::character::body::controller(assets)?;
        let standing = (!figure.is_empty()).then(|| standing(assets, &table, figure));
        Ok((Self::load_from(assets, &table, drawn, true)?, standing))
    }

    /// As `load_with`, its controller's clips left whole: every bone worked
    /// out at each update, as before the bones the body follows were told
    /// apart from those only drawn (`Controller::follow`). The same poses;
    /// for setting the two side by side.
    pub fn load_unsplit(assets: &mut Assets, drawn: &[String]) -> Result<Self> {
        let table = trp_data::character::body::controller(assets)?;
        Self::load_from(assets, &table, drawn, false)
    }

    /// The body's controller.
    pub fn controller(&self) -> &Arc<Controller> {
        &self.controller
    }

    fn load_from(assets: &mut Assets, table: &trp_data::weapons::states::StateTable, drawn: &[String], split: bool) -> Result<Self> {
        let mut paths: Vec<String> = BONES.iter().map(|path| (*path).to_owned()).collect();
        paths.extend(finger_paths(BONES[ARMS[1][5]], 'L'));
        paths.extend(BEND_GOAL_NAMES.iter().map(|name| format!("{}/{name}", BONES[4])));
        paths.push(format!("{}/{CAMERA_CONTAINER_NAME}", BONES[4]));
        paths.extend(drawn.iter().cloned());
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let bones = trp_data::character::body::bones(assets, &paths)?
            .into_iter()
            .map(|b| BoneRest { path: b.path, position: b.position, rotation: b.rotation, scale: None })
            .collect();
        let backpack_drop = table.states.iter().find(|state| state.full_name == BACKPACK_DROP).and_then(|state| state.event_time(BACKPACK_DROP_MARK));
        let bipod_marks = BIPOD_STATES.map(|name| table.states.iter().find(|state| state.full_name == name).and_then(|state| state.event_time(BIPOD_MARK)));
        let vault_marks = std::array::from_fn(|kind| {
            let state = table.states.iter().find(|state| state.full_name == VAULT_STATES[kind]);
            std::array::from_fn(|mark| state.and_then(|state| state.event_time(VAULT_MARKS[kind][mark])))
        });
        let mut controller = Controller::load(assets, &table, bones, &wanted)?;
        // The first bone followed is the one the clips move along the ground.
        controller.root_bone = Some(0);
        // What the body goes by is the bones before those a host asked to
        // have drawn (`DRAWN_FROM`): every update works those out, and the
        // rest are made from its mix (`Controller::follow`, `drawn_bones`).
        if split {
            let followed: Vec<bool> = (0..controller.bones.len()).map(|bone| bone < DRAWN_FROM).collect();
            controller.follow(&followed)?;
        }
        let id = |name: &str| controller.parameter(name);
        let ids = Ids {
            speed: id("Speed"),
            direct_x: id("Direct_X"),
            direct_y: id("Direct_Y"),
            direct: id("Direct"),
            level: id("Level"),
            tilt: id("Tilt"),
            inert: id("Inert"),
            inert_float: id("InertFloat"),
            sprint: id("Sprint"),
            sprint_speed: id("SprintSpeed"),
            sprint_inertia: id("SprintInertia"),
            jumping: id("IsJumping"),
            landing: id("Landing"),
            falling: id("FallingDown"),
            grounded: id("IsGrounded"),
            prone: id("Prone"),
            transition_speed: id("TransitionMultiplier"),
            aim_angle: id("Aim_angle"),
            delta_rotation: id("DeltaRotation"),
            is_aiming: id("isAiming"),
            prone_reload: id("ProneReload"),
            reload_float: id("ReloadFloat"),
            step: id("Step"),
            sidestep: id("SidestepFloat"),
            sideback_speed: id("SidebackSpeed"),
            weapon_size: id("WeapSizeModifier"),
            weapon_type: id("WeaponType"),
            weapon_type_float: id("WeaponTypeFloat"),
            foot_step: id("FootStep"),
            weapon_3rd: id("Weapon_3rd"),
            position_cache: id("Position_cache"),
            first_person: id("First_Person_Curve_Weight"),
            hand_right: id("Hand_Right"),
            hand_left: id("Hand_Left"),
            shoulder_right: id("Shoulder_Right"),
            shoulder_left: id("Shoulder_Left"),
            elbow_right: id("Elbow_Right"),
            elbow_left: id("Elbow_Left"),
            aiming_layer: id("Aiming_Layer"),
            vaulting_height: id("VaultingHeight"),
            vaulting_length: id("VaultingLength"),
            over_obstacle: id("OverObstacleHeight"),
            forward_velocity: id("AbsoluteForwardVelocity"),
            do_vault: id("DoVault"),
            do_climb: id("DoClimb"),
            is_vaulting: id("IsVaulting"),
            vaulting_speed: id("Vaulting_speed"),
            mouse_look: id("Mouse_look"),
            is_interaction: id("IsInteraction"),
            interaction_index: id("InteractionIndex"),
            inventory: id("Inventory"),
            inventory_operation: id("InventoryOperation"),
            kick: id("Kick"),
            kick_succeed: id("KickSucceed"),
            approached: id("Approached"),
            third_person: id("ThirdPerson"),
            third_person_float: id("ThirdPersonFloat"),
            first_action: id("FirstAction"),
            speed_draw: id("SpeedDraw"),
            swing_speed: id("SwingSpeed"),
            deflected: id("Deflected"),
            blind_fire: id("BlindFire"),
            blind_fire_float: id("BlindFireFloat"),
            prone_bipods: id("ProneBipodsOn"),
            left_stance: id("LeftStance"),
            force_left_stance: id("ForceLeftStanceInOneFrame"),
            left_stance_curve: id("Leftstance_curve"),
        };
        let base = controller.layers.first().map_or(0, |layer| layer.machine);
        let sidestep_length = controller
            .machines
            .get(base)
            .into_iter()
            .flat_map(|machine| &machine.states)
            .filter_map(|state| state.movement.as_ref())
            .find(|script| script.state == movement_state::SIDESTEP && script.kind == part::IN)
            .map_or(0.0, |script| script.length);
        // Blind fire's curves, from the scripts of its four states.
        let mut blind_curves: [[Option<Curve>; 2]; 2] = Default::default();
        for script in controller.machines.get(base).into_iter().flat_map(|machine| &machine.states).filter_map(|state| state.movement.as_ref()) {
            if script.state != movement_state::BLIND_FIRE {
                continue;
            }
            let part = usize::from(script.kind == part::OUT);
            let side = usize::from(script.direction == 1);
            blind_curves[part][side] = script.curve.clone();
        }
        let drawn_root = drawn.iter().position(|path| path == BONES[0]);
        let head_bones = HEAD_BONES.map(|end| drawn.iter().position(|path| path.ends_with(end)));
        // The prefab's own shares; a half each where they cannot be read
        // (what this install's prefab has).
        let head_shares = trp_data::character::player::head_chain(assets).unwrap_or([0.5, 0.5]);
        let reach = limbs_reach(assets, &controller)?;
        Ok(Self {
            controller: Arc::new(controller),
            ids,
            faded_layer: 6,
            sidestep_length,
            drawn: drawn.len(),
            drawn_root,
            backpack_drop,
            bipod_marks,
            vault_marks,
            head_bones,
            head_shares,
            blind_curves,
            reach,
        })
    }

    /// How many clips were read for it.
    pub fn clip_count(&self) -> usize {
        self.controller.clips.len()
    }

    /// How long one of the animator's states lasts at its own speed, in
    /// seconds, by the first clip it plays (times that clip's length factor
    /// in its blend tree), with that clip's name: for looking at.
    pub fn state_length(&self, full_name: &str) -> Option<(f32, String)> {
        let state = self.controller.machines.iter().flat_map(|machine| &machine.states).find(|state| state.name == full_name)?;
        let (clip, factor) = state.trees.first()?.as_ref()?.iter().find_map(|node| node.clip.map(|clip| (clip, node.length_factor)))?;
        let clip = &self.controller.clips.get(clip)?.clip;
        Some((clip.duration() * factor / state.speed.abs().max(1e-6), clip.name.clone()))
    }
}

/// The character as the game's own screens stand it with nothing in its
/// hands (`PlayerModelLoader.CreateWeapon`, which its screens call with no
/// time to play): the body's controller on the bones named, its standing
/// layer at a weight of 1 and nothing in the hands, updated once by
/// nothing. A controller of its own, for these bones and of the states
/// such an update plays: each layer's first and the standing layer's.
fn standing(assets: &mut Assets, table: &trp_data::weapons::states::StateTable, paths: &[String]) -> Result<Pose> {
    let named: Vec<&str> = paths.iter().map(String::as_str).collect();
    let bones = trp_data::character::body::bones(assets, &named)?
        .into_iter()
        .map(|b| BoneRest { path: b.path, position: b.position, rotation: b.rotation, scale: None })
        .collect();
    let firsts: Vec<&str> = table.layers.iter().filter_map(|layer| layer.default_state.as_deref()).collect();
    let prefix = format!("{STANDING_LAYER}.");
    let played = |state: &AnimState| state.full_name.starts_with(&prefix) || firsts.contains(&state.full_name.as_str());
    let mut controller = Controller::load(assets, table, bones, &played)?;
    // The root as the body's own: the clips' travel over the ground taken
    // off it.
    controller.root_bone = paths.iter().position(|path| path == BONES[0]);
    let layer = controller.layer(STANDING_LAYER).ok_or_else(|| anyhow::anyhow!("the body's controller has no layer {STANDING_LAYER}"))?;
    let mut animator = Animator::new(Arc::new(controller));
    animator.set_layer_weight(layer, 1.0);
    for name in STANDING_KIND {
        animator.set_float(name, NOTHING_IN_HANDS);
    }
    animator.update(0.0);
    Ok(animator.pose().to_vec())
}
