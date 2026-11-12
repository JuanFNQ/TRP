//! The figure's and its weapon's data loaded from the install.

use super::*;

/// The figure's body, read once: the body's controller on the menus'
/// copy's skeleton with the states the screens play (each layer's first,
/// and all of `GUI`'s), and the poser's arm solvers.
#[derive(Clone)]
pub struct FigureData {
    pub(super) controller: Arc<Controller>,
    pub(super) layer: usize,
    pub(super) skeleton: Arc<Skeleton>,
    pub(super) named: Arc<Named>,
    drawn: Vec<usize>,
    /// The poser's solvers (`Limbs`, `Twists`), right then left.
    pub(super) limbs: [LimbData; 2],
}

impl FigureData {
    /// Reads it, with further bones for a host to draw by, each named by its
    /// path below the animated object (the body's meshes' bones): the
    /// pose gives each, and `drawn` where.
    pub fn load(assets: &mut Assets, drawn: &[String]) -> Result<Self> {
        let table = trp_data::character::body::controller(assets)?;
        let mut skeleton = Skeleton::default();
        let named = Named::add(&mut skeleton);
        let drawn: Vec<usize> = drawn.iter().map(|path| skeleton.add(path)).collect();
        let paths: Vec<&str> = skeleton.paths.iter().map(String::as_str).collect();
        let bones = trp_data::character::body::menu_bones(assets, &paths)?
            .into_iter()
            .map(|b| BoneRest { path: b.path, position: b.position, rotation: b.rotation, scale: None })
            .collect();
        let firsts: Vec<&str> = table.layers.iter().filter_map(|layer| layer.default_state.as_deref()).collect();
        let prefix = format!("{LAYER}.");
        let played = |state: &trp_data::weapons::states::AnimState| state.full_name.starts_with(&prefix) || firsts.contains(&state.full_name.as_str());
        let mut controller = Controller::load(assets, &table, bones, &played)?;
        // The clips' travel over the ground is the root's own and is taken
        // off its bone, as the body's (`BodyData`).
        controller.root_bone = Some(named.root);
        let layer = controller.layer(LAYER).ok_or_else(|| anyhow!("the body's controller has no layer {LAYER}"))?;
        let limbs = LimbData::load_from(assets, trp_data::character::body::MENU_PLAYER_BUNDLE)?;
        for limb in &limbs {
            arms::check(limb)?;
        }
        Ok(Self { controller: Arc::new(controller), layer, skeleton: Arc::new(skeleton), named: Arc::new(named), drawn, limbs })
    }

    /// Every bone of the figure by its path below the animated object, each
    /// after its parent: the order of the pose.
    pub fn paths(&self) -> &[String] {
        &self.skeleton.paths
    }

    /// Each bone's parent in that list, none for the root's.
    pub fn parents(&self) -> &[Option<usize>] {
        &self.skeleton.parents
    }

    /// Where each bone the host asked for is in that list, in its order.
    pub fn drawn(&self) -> &[usize] {
        &self.drawn
    }

    /// The `GUI` layer's place among the controller's.
    pub fn layer(&self) -> usize {
        self.layer
    }

    /// When a clip of the figure's animator fires an event, in seconds into
    /// the clip: the first of that function's name (the idle's
    /// `OnRandomizeAnimation`, the weapon check's `OnWeaponOperationStart`
    /// and `OnWeaponOperationEnd`).
    pub fn event_time(&self, clip: &str, function: &str) -> Option<f32> {
        let clip = self.controller.clips.iter().find(|def| def.clip.name == clip)?;
        clip.clip.events.iter().find(|event| event.function == function).map(|event| event.time)
    }

    /// The controller's parameter's starting value, by name.
    pub fn parameter_default(&self, name: &str) -> Option<f32> {
        self.controller.parameters.iter().find(|p| p.name == name).map(|p| p.default)
    }

    /// The controller, for looking at its states.
    pub fn controller(&self) -> &Controller {
        &self.controller
    }
}

/// Which grip a hand takes (`MenuPlayerPoser.UpdatePersonGrip`).
#[derive(Debug, Clone)]
pub(super) enum GripChoice {
    /// A part's grip, by its node, and whether it is an alternative one.
    One { node: usize, alternative: bool },
    /// The left hand's blend between a special grip (an alternative or an
    /// underbarrel weapon's: the first) and the common one (the second),
    /// by `Patrol` (`GripBlender`), each one's finger pose as it was when
    /// the blend was made. The game counts it alternative.
    Blend { nodes: [usize; 2], fingers: [Vec<[f32; 4]>; 2] },
}

/// The weapon in the figure's hands, read once: its animator, its rig at
/// rest, and the bones the poser reads.
#[derive(Clone)]
pub struct FigureWeaponData {
    pub(super) animator: WeaponAnimatorData,
    pub(super) rig: WeaponRig,
    /// Its state `PATROL` by its full name; a controller without one (the
    /// rangefinder's) is asked for it all the same and stays where it is,
    /// as the engine's `Play` of a state it lacks leaves it.
    pub(super) patrol: String,
    pub(super) moved: usize,
    /// Right then left: the marker each hand is solved onto, the fixed
    /// point beside it that counts as on the weapon, and the weapon's goal
    /// for the elbow (`TransformLinks.GatherIK`).
    pub(super) markers: [Option<usize>; 2],
    pub(super) references: [Option<usize>; 2],
    pub(super) goals: [Option<usize>; 2],
    /// The weapon's own hands' palms and every bone below each, in order
    /// (the hand posers' `_firstPersonPalm`).
    pub(super) palms: [Vec<usize>; 2],
    pub(super) grips: [Option<GripChoice>; 2],
    /// Each grip node's finger pose, palm first: stored, or (where the
    /// grip keeps none, `live`) the node and the bones below it, in order,
    /// read as they stand (`GripPose`'s indexer).
    pub(super) grip_fingers: Vec<GripFingers>,
}

#[derive(Debug, Clone)]
pub(super) struct GripFingers {
    pub(super) node: usize,
    pub(super) stored: Vec<[f32; 4]>,
    pub(super) live: Option<Vec<usize>>,
}

impl FigureWeaponData {
    /// Reads it for an assembled weapon.
    pub fn load(assets: &Assets, weapon: &Weapon) -> Result<Self> {
        let animator = WeaponAnimatorData::load(assets, weapon)?;
        let tree = &weapon.hierarchy;
        let rig = WeaponRig::new(tree);
        let patrol = animator.state_names().into_iter().find(|name| name.rsplit('.').next() == Some(PATROL)).unwrap_or_else(|| PATROL.to_owned());
        let moved = rig.moved_bone().context("the weapon has no Weapon_root_anim")?;
        // `TransformLinks.GetTransform`: a bone found by its name anywhere
        // in the weapon.
        let named = |name: &str| (0..tree.nodes.len()).find(|&i| tree.nodes[i].name == name);
        let markers = [named("weapon_R_IK_marker"), named("weapon_L_IK_marker")];
        let references = [named("weapon_R_hand_marker"), named("weapon_L_hand_marker")];
        let goals = [named("Bend_Goal_Right"), named("Bend_Goal_Left")];
        let below = |index: usize, ancestor: usize| {
            let mut at = index;
            while let Some(p) = tree.nodes[at].parent {
                if p == ancestor {
                    return true;
                }
                at = p;
            }
            false
        };
        let hand = |name: &str| -> Vec<usize> {
            named(name).map(|palm| std::iter::once(palm).chain((palm + 1..tree.nodes.len()).filter(|&i| below(i, palm))).collect()).unwrap_or_default()
        };
        let palms = [hand("Base HumanRPalm"), hand("Base HumanLPalm")];
        // Every grip in the weapon on an object that is on
        // (`GetComponentsInChildren<GripPose>()`), in the tree's order.
        let on = |mut index: usize| loop {
            if !tree.nodes[index].active {
                return false;
            }
            match tree.nodes[index].parent {
                Some(parent) => index = parent,
                None => return true,
            }
        };
        let depth = |mut index: usize| {
            let mut depth = 0;
            while let Some(parent) = tree.nodes[index].parent {
                depth += 1;
                index = parent;
            }
            depth
        };
        let grips: Vec<usize> = (0..tree.nodes.len()).filter(|&i| tree.nodes[i].grip.is_some() && on(i)).collect();
        let grip = |i: usize| tree.nodes[i].grip.as_ref().expect("a grip");
        // Each grip's fingers, palm first: stored, or the bones below it as
        // the rig has them where it keeps none.
        let fingers_of = |i: usize| -> Vec<[f32; 4]> {
            let stored = &grip(i).fingers;
            if stored.is_empty() {
                std::iter::once(i).chain((i + 1..tree.nodes.len()).filter(|&j| below(j, i))).map(|j| tree.nodes[j].rotation).collect()
            } else {
                stored.clone()
            }
        };
        let grip_fingers = grips
            .iter()
            .map(|&i| GripFingers {
                node: i,
                stored: fingers_of(i),
                live: grip(i).live.then(|| std::iter::once(i).chain((i + 1..tree.nodes.len()).filter(|&j| below(j, i))).collect()),
            })
            .collect();
        // The left hand: an underbarrel weapon's grips first, then the
        // deepest; its first special grip and its first common one, blended
        // where it has both (an underbarrel weapon's grip then both).
        let mut left: Vec<usize> = grips.iter().copied().filter(|&i| grip(i).left_hand).collect();
        left.sort_by_key(|&i| (grip(i).kind != 2, std::cmp::Reverse(depth(i))));
        let special = left.iter().copied().find(|&i| grip(i).kind != 0);
        let common = left.iter().copied().find(|&i| grip(i).kind == 0);
        let left = match (special, common) {
            (Some(special), Some(common)) => {
                let common = if grip(special).kind == 2 { special } else { common };
                Some(GripChoice::Blend { nodes: [special, common], fingers: [fingers_of(special), fingers_of(common)] })
            }
            (Some(one), None) | (None, Some(one)) => Some(GripChoice::One { node: one, alternative: grip(one).kind == 1 }),
            (None, None) => None,
        };
        // The right: an alternative grip first, then the deepest.
        let mut right: Vec<usize> = grips.iter().copied().filter(|&i| !grip(i).left_hand).collect();
        right.sort_by_key(|&i| (grip(i).kind != 1, std::cmp::Reverse(depth(i))));
        let right = right.first().map(|&node| GripChoice::One { node, alternative: grip(node).kind == 1 });
        Ok(Self { animator, rig, patrol, moved, markers, references, goals, palms, grips: [right, left], grip_fingers })
    }
}

/// What the item in hand tells the weapon's animator as the figure is
/// made (`PlayerModelLoader.CreateWeapon`, `MenuPlayerPoser.ChangeWeapon`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loaded {
    /// A firearm (the game's `Weapon`: no knife, thrown thing or
    /// rangefinder), which alone is told the rest below.
    pub firearm: bool,
    /// The stock folded (its animator's `Stock` layer on).
    pub folded: bool,
    /// The fitted magazine's capacity (`GetMaxMagazineCount`), none
    /// without a magazine.
    pub magazine: Option<u32>,
    /// The magazine's `magAnimationIndex`.
    pub magazine_animation: i32,
    /// Rounds in the chamber.
    pub chamber: u32,
    /// The weapon is reloaded by its magazine (a magazine, a reload mode
    /// neither `OnlyBarrel` nor `InternalMagazine`, no rocket launcher);
    /// its chamber can be checked (no rocket launcher).
    pub can_reload: bool,
    pub can_check_chamber: bool,
}

impl Loaded {
    /// An assembled weapon as its build has it, `chamber` rounds in its
    /// chamber, its stock as `folded`. A thing that is no firearm (a knife,
    /// a thrown thing, a rangefinder: the game's `Knife`, `ThrowWeap` and
    /// the rest are no `Weapon`, and neither is a rig read from a prefab
    /// alone, which has no template) can neither reload nor have its
    /// chamber checked, and tells its animator nothing of either.
    pub fn of(db: &Database, weapon: &Weapon, chamber: u32, folded: bool) -> Self {
        let template = db.item(&weapon.template).ok();
        let firearm = template.is_some_and(|t| db.is_a(t, "Weapon"));
        let mode = template.and_then(|t| t.props.get("ReloadMode")).and_then(serde_json::Value::as_str);
        let launcher = template.is_some_and(|t| db.is_a(t, "RocketLauncher"));
        Self {
            firearm,
            folded: folded && firearm,
            magazine: weapon.magazine_capacity.filter(|_| firearm),
            magazine_animation: weapon.magazine_animation.unwrap_or(0),
            chamber: if firearm { chamber } else { 0 },
            can_reload: firearm && weapon.magazine_capacity.is_some() && !matches!(mode, Some("OnlyBarrel" | "InternalMagazine")) && !launcher,
            can_check_chamber: firearm && !launcher,
        }
    }

    /// The weapon's moves the figure picks among, by their triggers, in the
    /// order of the pick (`ChangeWeapon`: `_weaponAnimations`).
    pub fn moves(&self) -> Vec<&'static str> {
        let mut moves = vec!["Look"];
        if self.can_check_chamber {
            moves.push("CheckChamber");
        }
        if self.can_reload {
            moves.extend(["CheckAmmo", "Reload"]);
        }
        moves
    }
}
