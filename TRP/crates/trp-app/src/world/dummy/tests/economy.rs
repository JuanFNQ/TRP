//! Tests that the posing economy gives the same rays and blade touches.

use super::*;

/// What the posing economy's tests' dummies are made of, read once: the
/// dummies' AKM (its build with the light) and its animator split as the
/// app splits it, the motion's and the body's data (the body's drawn
/// bones those the hit colliders hang on, as `trp-sim`'s puppet tests
/// have them), the arms' solver, the player prefab's hit colliders.
pub(super) struct EconomyMakings {
    pub(super) db: &'static trp_data::Database,
    pub(super) weapon: trp_data::weapons::weapon::Weapon,
    hands_data: HandsData,
    hard: trp_data::setup::hard_settings::HardSettings,
    animator: trp_sim::weapons::weapon_animator::WeaponAnimatorData,
    firearm: trp_sim::FirearmSettings,
    pub(super) body: trp_sim::locomotion::body::BodyData,
    pub(super) movement: trp_sim::MovementSettings,
    limbs: [LimbData; 2],
    pub(super) engine: trp_data::setup::engine::EngineSettings,
    pub(super) colliders: Vec<HitColliderData>,
}

/// The body's bones the colliders hang on (`trp-sim`'s `collider_bones`)
/// and each arm's chain under its collarbone, as the dummy's skeleton has
/// them.
fn economy_paths() -> (Vec<String>, Vec<String>) {
    const SPINE3: &str = "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3";
    let mut drawn: Vec<String> = ["Root_Joint", "Root_Joint/Base HumanPelvis", "Root_Joint/Base HumanPelvis/Base HumanSpine1", "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2", SPINE3].iter().map(|path| (*path).to_owned()).collect();
    drawn.push(format!("{SPINE3}/Base HumanNeck"));
    drawn.push(format!("{SPINE3}/Base HumanNeck/Base HumanHead"));
    drawn.push(format!("{SPINE3}/Base HumanRibcage"));
    let mut arms = Vec::new();
    for side in ['L', 'R'] {
        drawn.push(format!("{SPINE3}/Base HumanRibcage/Base Human{side}Collarbone"));
        let mut leg = "Root_Joint/Base HumanPelvis".to_owned();
        for name in ["Thigh1", "Thigh2", "Calf", "Foot"] {
            leg.push_str(&format!("/Base Human{side}{name}"));
            drawn.push(leg.clone());
        }
        let mut arm = format!("{SPINE3}/Base HumanRibcage/Base Human{side}Collarbone");
        for name in ["Upperarm", "Forearm1", "Forearm2", "Forearm3", "Palm"] {
            arm.push_str(&format!("/Base Human{side}{name}"));
            arms.push(arm.clone());
        }
    }
    (drawn, arms)
}

pub(super) fn economy_makings() -> Option<&'static EconomyMakings> {
    static READ: std::sync::OnceLock<Option<EconomyMakings>> = std::sync::OnceLock::new();
    READ.get_or_init(|| {
        let (install, db) = installed()?;
        let mut assets = trp_data::unity::Assets::new(install.clone()).expect("manifest");
        let hard = trp_data::setup::hard_settings::HardSettings::load(install).expect("settings object");
        let engine = trp_data::setup::engine::EngineSettings::load(install).expect("engine settings");
        let weapon = trp_data::weapons::weapon::assemble_preset(&mut assets, db, &build(db).expect("the dummies' build")).expect("the AKM");
        let hands_data = HandsData::load(&mut assets).expect("player prefab");
        let hands = trp_sim::Hands::new(trp_sim::hands_settings(hands_data.clone(), db, &hard, &engine, &weapon, trp_sim::ViewSettings::default()).expect("hands settings"));
        let followed = trp_sim::characters::puppet::followed_rig_bones(&weapon.hierarchy, trp_sim::characters::puppet::sights_for(&hands, &weapon.hierarchy).as_ref());
        let animator = trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, &weapon).expect("an animator").following(&followed).expect("split");
        let firearm = trp_sim::firearm_settings(&assets, db, &hard, &weapon).expect("firearm settings");
        let (drawn, _) = economy_paths();
        let body = trp_sim::locomotion::body::BodyData::load_with(&mut assets, &drawn).expect("body animator");
        let read = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement");
        let movement = read.relimited(&hard, &db.config, 0.0, WeightLimits { unencumbered: true, ..WeightLimits::BASE }).expect("the load's settings");
        let limbs = LimbData::load(&mut assets).expect("the arms' solver");
        let colliders = trp_data::character::ragdoll::RagdollData::hit_colliders(&mut assets).expect("the hit colliders");
        Some(EconomyMakings { db, weapon, hands_data, hard, animator, firearm, body, movement, limbs, engine, colliders })
    })
    .as_ref()
}

/// Three dummies as the app makes them (a puppet with the AKM, smooth,
/// its skeleton the colliders' bones and the arms', the player prefab's
/// hit colliders on it), at their places on the range and one more 2.5 m
/// to the walker's right, the economy as said.
pub(super) fn economy_dummies(k: &EconomyMakings, economy: bool) -> Dummies {
    let (drawn_paths, arm_paths) = economy_paths();
    let paths: Vec<String> = drawn_paths.iter().chain(&arm_paths).cloned().collect();
    let parents: Vec<Option<usize>> = paths.iter().map(|path| path.rsplit_once('/').and_then(|(above, _)| paths.iter().position(|p| p == above))).collect();
    let bone = |name: &str| paths.iter().position(|path| path.rsplit('/').next() == Some(name));
    let mut list = Vec::new();
    for (which, at) in [PLACES[0].at, PLACES[1].at, PLACES[1].at + Vec3::new(2.5, 0.0, -1.0)].into_iter().enumerate() {
        let pacer = trp_sim::Pacer::new(trp_sim::REFERENCE_FRAME_RATE, k.engine.fixed_timestep, k.engine.maximum_timestep);
        let mut movement = trp_sim::Movement::new(k.movement.clone(), trp_sim::Body::new(&k.body, k.weapon.cell_width as f32));
        movement.position = [at.x, at.y, -at.z];
        movement.yaw = FACING;
        movement.body_yaw = FACING;
        movement.note_rotation();
        let hands = trp_sim::Hands::new(trp_sim::hands_settings(k.hands_data.clone(), k.db, &k.hard, &k.engine, &k.weapon, trp_sim::ViewSettings::default()).expect("hands settings"));
        let puppet = Puppet::new(movement, hands, trp_sim::Firearm::new(k.firearm.clone(), &k.animator), &k.weapon.hierarchy, k.limbs, k.weapon.cell_width as f32, pacer, true).expect("the puppet");
        let colliders = k
            .colliders
            .iter()
            .filter_map(|c| {
                let local = c.local.map_or(Affine3A::IDENTITY, |(place, turn)| convert::transform(place, turn, [1.0; 3]).compute_affine());
                Some(Collider { data: c.clone(), bone: bone(&c.bone)?, local, body: BodyRef { character: which as u32 + 1, part: c.part, collider: c.collider_type, plate: c.plate, damage_mod: c.damage_mod }, figures: c.figures })
            })
            .collect::<Vec<_>>();
        assert_eq!(colliders.len(), k.colliders.len(), "every collider on a bone");
        let root = &k.weapon.hierarchy.nodes[0];
        let mut root_local = convert::transform(root.translation, root.rotation, root.scale);
        root_local.translation = Vec3::ZERO;
        let model = Model { player: Entity::PLACEHOLDER, pivot: Entity::PLACEHOLDER, tree: Vec::new(), weapon_root: None, grip: Grip { root_local, fingers: Vec::new() } };
        list.push(Dummy {
            which,
            colliders,
            placed: Vec::new(),
            objects: Vec::new(),
            wearer: Wearer::dress(k.db, Arc::new(ArmorRules::new(&k.db.config).expect("the armour's rules")), &[], VestSkills::default()),
            worn: false,
            health: Health::new(Arc::new(HealthSettings::new(&k.db.config).expect("the health's settings")), 1),
            followed: false,
            load: (0.0, Vec::new()),
            dressed: true,
            taken: Vec::new(),
            killing: None,
            fall: None,
            hits: 0,
            puppet: Some(puppet),
            heading: 1.0,
            model: Some(model),
            empty_grip: None,
            own: None,
            holds: None,
            drawn: drawn_paths.iter().map(|path| paths.iter().position(|p| p == path)).collect(),
            arm_bones: ['R', 'L'].map(|side| ["Collarbone", "Upperarm", "Forearm1", "Forearm2", "Forearm3", "Palm"].map(|name| bone(&format!("Base Human{side}{name}")))),
            locals: vec![Transform::IDENTITY; paths.len()],
            parents: parents.clone().into(),
            colliders_frame: None,
            before_frame: None,
            placements: Mutex::new(Vec::new()),
            drawn_by: PosingDrawn::EveryFrame,
            held_frame: None,
            heard: Vec::new(),
        });
    }
    Dummies { list, bleeding_drops: Vec::new(), experience: Vec::new(), marks: Vec::new(), listened: false, economy, posing_log: PosingLog::default() }
}

/// The dummies stepped a draw of 60 a second (the walker's keys, the
/// third reloading at the half second), posed when asked where `asked`,
/// and their colliders said to be of the newest frame (`place_dummies`).
pub(super) fn step_economy(dummies: &mut Dummies, step: usize, asked: bool) {
    for dummy in &mut dummies.list {
        let which = dummy.which;
        let puppet = dummy.puppet.as_mut().expect("its character");
        puppet.posing(if asked { Posing::Asked } else { Posing::Full });
        puppet.keep_from(dummy.colliders_frame);
        if which == 2 && step == 30 {
            puppet.firearm_mut().expect("its AKM").reload_with(30);
        }
        let keys = if which == 1 { [if (step / 90) % 2 == 0 { 1.0 } else { -1.0 }, 0.0] } else { [0.0, 0.0] };
        puppet.step(1.0 / 60.0, keys, &trp_sim::Flat, Some(&mut dummy.health));
        dummy.before_frame = dummy.colliders_frame;
        dummy.colliders_frame = dummy.newest_standing();
    }
}

/// A small seeded generator (xorshift) of numbers in a span.
pub(super) struct Seeded(pub(super) u64);

impl Seeded {
    pub(super) fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % 1_000_000) as f32 / 1_000_000.0
    }

    pub(super) fn within(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

/// The posing economy's broad test of a bullet's path (`Dummies::ray`):
/// over seeded rays, from the firing line and from anywhere about the
/// dummies, aimed near one or anywhere, every hit the same to the bit
/// with each dummy's bound asked first and without, on every frame of
/// two seconds of three dummies (standing, walking, reloading), posed in
/// full and posed when asked; and the bound let most rays pass dummies
/// by unposed.
#[test]
fn a_rays_hits_are_the_same_with_the_bound_asked_first() {
    let Some(k) = economy_makings() else { return };
    let (mut hits_seen, mut rays, mut passed) = (0usize, 0usize, 0usize);
    for asked in [false, true] {
        let mut dummies = economy_dummies(k, true);
        let mut seeded = Seeded(0x5eed_0001 ^ u64::from(asked));
        for step in 0..120 {
            step_economy(&mut dummies, step, asked);
            for _ in 0..40 {
                let from = if seeded.unit() < 0.5 { [seeded.within(-4.0, 4.0), seeded.within(1.0, 1.8), seeded.within(0.0, 8.0)] } else { near(&dummies, &mut seeded, 4.0) };
                let aim = if seeded.unit() < 0.7 { near(&dummies, &mut seeded, 0.6) } else { near(&dummies, &mut seeded, 5.0) };
                let way: V3 = std::array::from_fn(|i| aim[i] - from[i]);
                let length = (way[0].powi(2) + way[1].powi(2) + way[2].powi(2)).sqrt().max(1e-3);
                let reach = length * seeded.within(0.6, 1.6);
                let to: V3 = std::array::from_fn(|i| from[i] + way[i] / length * reach);
                let (mut with, mut without) = (Vec::new(), Vec::new());
                dummies.ray_by(from, to, true, &mut with);
                dummies.ray_by(from, to, false, &mut without);
                assert_eq!(format!("{with:?}"), format!("{without:?}"), "step {step}, asked {asked}: {from:?} to {to:?}");
                rays += 1;
                hits_seen += with.len();
                passed += dummies.list.iter().filter(|dummy| dummy.colliders_frame.is_some_and(|frame| !dummy.bound_meets(frame, from, to, 0.0))).count();
            }
        }
    }
    println!("{rays} rays, {hits_seen} hits the same with the bound asked first; {passed} of {} dummies passed by unposed", rays * 3);
    assert!(hits_seen > 1000 && passed > rays, "{hits_seen} {passed}");
}

/// The same of a blade's sweep (`Dummies::blade_targets`): over seeded
/// sweeps about the dummies (a knife's box, 1.5 m along a seeded way),
/// the engine's touches (`trp_physics`, each target on a kinematic body
/// of its own as the stroke's world has it), in the engine's own order,
/// the same to the bit from three dummies posed when asked (their
/// colliders placed as the blade asks) as from the same three posed in
/// full, stepped alike. Leaving out the dummies whose bound a sweep
/// misses is not the same: the engine's order of touches goes by every
/// body in its world (counted, and said).
#[test]
fn a_blades_touches_are_the_same_with_the_dummies_posed_when_asked() {
    let Some(k) = economy_makings() else { return };
    let (mut full, mut asked) = (economy_dummies(k, false), economy_dummies(k, true));
    let mut seeded = Seeded(0x5eed_0002);
    let (mut touched, mut sweeps, mut reordered) = (0usize, 0usize, 0usize);
    let touches = |targets: Vec<(crate::weapons::knife::Target, V3, [f32; 4], PhysicsCollider)>, sweep: &trp_sim::BladeSweep| {
        let mut world = trp_physics::World::new(trp_physics::Settings::game());
        let bodies: Vec<(Body, crate::weapons::knife::Target)> = targets.into_iter().map(|(target, at, turn, collider)| (world.add_kinematic(at, turn, &[collider], 16), target)).collect();
        world.build_query_trees();
        world
            .sweep_box_touches(sweep.from, sweep.half, sweep.turn, sweep.way, sweep.reach)
            .into_iter()
            .map(|touch| {
                let target = match touch.met {
                    Some(trp_physics::Met::Body(body)) => bodies.iter().find(|(own, _)| *own == body).map(|(_, target)| (target.which, target.index)),
                    _ => None,
                };
                (touch.distance.to_bits(), touch.point.map(f32::to_bits), target)
            })
            .collect::<Vec<_>>()
    };
    for step in 0..90 {
        step_economy(&mut full, step, false);
        step_economy(&mut asked, step, true);
        for _ in 0..10 {
            let from = near(&asked, &mut seeded, 1.0);
            let way = {
                let w = [seeded.within(-1.0, 1.0), seeded.within(-0.4, 0.4), seeded.within(-1.0, 1.0)];
                let length = (w[0].powi(2) + w[1].powi(2) + w[2].powi(2)).sqrt().max(1e-3);
                w.map(|v| v / length)
            };
            let sweep = trp_sim::BladeSweep { from, half: [0.02, 0.03, 0.15], turn: [0.0, 0.0, 0.0, 1.0], way, reach: 1.5 };
            let (with, without) = (touches(asked.blade_targets(), &sweep), touches(full.blade_targets(), &sweep));
            assert_eq!(with, without, "step {step}: {sweep:?}");
            // The dummies whose bound the sweep misses left out.
            let to: V3 = std::array::from_fn(|i| from[i] + way[i] * 1.5);
            let met: Vec<usize> = asked.list.iter().filter(|dummy| dummy.colliders_frame.is_some_and(|frame| dummy.bound_meets(frame, from, to, 0.16))).map(|dummy| dummy.which).collect();
            let some: Vec<_> = asked.blade_targets().into_iter().filter(|(target, ..)| met.contains(&target.which)).collect();
            reordered += usize::from(touches(some, &sweep) != with);
            sweeps += 1;
            touched += usize::from(!with.is_empty());
        }
    }
    println!("{sweeps} sweeps, {touched} touching a dummy, the touches the same posed when asked; with the dummies whose bound a sweep misses left out, {reordered} sweeps' touches not the same");
    assert!(touched > 100, "{touched}");
}
