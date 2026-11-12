//! The posing economy: scenes, makings, poses asked for and what it saves.

use super::*;

// The puppet's economy (body-animator
// spec, "Layers", "TRP's own"; third-person spec, "Posing by distance and
// off screen"): a frame's pose made from its recipe, at once or when asked.

/// The body's bones that the tests place the hit colliders by, each by its
/// path below the animated object (`BodyData::load_with`): the spine to
/// the head, the ribcage and the collarbones, both legs to the foot. The
/// arms below the collarbone are the solver's (`PuppetPose::arms`).
pub(super) fn collider_bones() -> Vec<String> {
    const SPINE3: &str = "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2/Base HumanSpine3";
    let mut paths: Vec<String> = ["Root_Joint", "Root_Joint/Base HumanPelvis", "Root_Joint/Base HumanPelvis/Base HumanSpine1", "Root_Joint/Base HumanPelvis/Base HumanSpine1/Base HumanSpine2", SPINE3]
        .iter()
        .map(|path| (*path).to_owned())
        .collect();
    paths.push(format!("{SPINE3}/Base HumanNeck"));
    paths.push(format!("{SPINE3}/Base HumanNeck/Base HumanHead"));
    paths.push(format!("{SPINE3}/Base HumanRibcage"));
    for side in ['L', 'R'] {
        paths.push(format!("{SPINE3}/Base HumanRibcage/Base Human{side}Collarbone"));
        let mut leg = "Root_Joint/Base HumanPelvis".to_owned();
        for name in ["Thigh1", "Thigh2", "Calf", "Foot"] {
            leg.push_str(&format!("/Base Human{side}{name}"));
            paths.push(leg.clone());
        }
    }
    paths
}

/// What every character of the economy's tests is made from, read once:
/// the AKM and the empty hands, each animator split by the bones a puppet
/// reads of it (`puppet::followed_rig_bones`, by the sight it picks:
/// `puppet::sights_for`), the body with the colliders' bones drawn
/// (`collider_bones`), and the player prefab's hit colliders.
pub(super) struct Makings {
    pub(super) db: &'static Database,
    pub(super) hard: HardSettings,
    pub(super) engine: EngineSettings,
    pub(super) akm: trp_data::weapons::weapon::Weapon,
    pub(super) empty: trp_data::weapons::weapon::Weapon,
    hands_data: HandsData,
    akm_animator: trp_sim::weapons::weapon_animator::WeaponAnimatorData,
    empty_animator: trp_sim::weapons::weapon_animator::WeaponAnimatorData,
    pub(super) firearm: trp_sim::FirearmSettings,
    pub(super) body: trp_sim::locomotion::body::BodyData,
    pub(super) movement: trp_sim::MovementSettings,
    pub(super) limbs: [LimbData; 2],
    pub(super) colliders: Vec<trp_data::character::ragdoll::HitColliderData>,
    /// One health's settings for all (their maps written in one order).
    pub(super) health: Arc<HealthSettings>,
}

/// A PMC's health, as a dummy's, its settings the makings' own.
pub(super) fn economy_health(k: &Makings) -> Health {
    Health::new(k.health.clone(), 1)
}

pub(super) fn makings() -> Option<&'static Makings> {
    static MAKINGS: std::sync::OnceLock<Option<Makings>> = std::sync::OnceLock::new();
    MAKINGS
        .get_or_init(|| {
            let install = crate::fixture::install()?;
            let db = crate::fixture::database().expect("database");
            let hard = HardSettings::load(&install).expect("settings object");
            let engine = EngineSettings::load(&install).expect("engine settings");
            let mut assets = Assets::new(install).expect("manifest");
            let akm = trp_data::weapons::weapon::assemble(&mut assets, db, AKM).expect("weapon");
            let empty = trp_data::weapons::weapon::assemble_prefab(&mut assets, trp_data::weapons::weapon::EMPTY_HANDS).expect("the empty hands' rig");
            let hands_data = HandsData::load(&mut assets).expect("player prefab");
            let split = |weapon: &trp_data::weapons::weapon::Weapon, hands: &Hands| {
                let followed = trp_sim::characters::puppet::followed_rig_bones(&weapon.hierarchy, trp_sim::characters::puppet::sights_for(hands, &weapon.hierarchy).as_ref());
                trp_sim::weapons::weapon_animator::WeaponAnimatorData::load(&assets, weapon).expect("an animator").following(&followed).expect("split")
            };
            let akm_hands = Hands::new(trp_sim::hands_settings(hands_data.clone(), db, &hard, &engine, &akm, ViewSettings::default()).expect("hands settings"));
            let empty_hands = Hands::new(trp_sim::empty_hands_settings(hands_data.clone(), db, &hard, &engine, &empty, ViewSettings::default()).expect("the empty hands' motion"));
            let (akm_animator, empty_animator) = (split(&akm, &akm_hands), split(&empty, &empty_hands));
            let firearm = trp_sim::firearm_settings(&assets, db, &hard, &akm).expect("firearm settings");
            let body = trp_sim::locomotion::body::BodyData::load_with(&mut assets, &collider_bones()).expect("body animator");
            let read = trp_sim::load_movement_settings(&mut assets, db, &hard, &engine, 0.0).expect("movement");
            let movement = read.relimited(&hard, &db.config, 0.0, WeightLimits { unencumbered: true, ..WeightLimits::BASE }).expect("the load's settings");
            let limbs: [LimbData; 2] = LimbData::load(&mut assets).expect("the arms' solver");
            let colliders = trp_data::character::ragdoll::RagdollData::hit_colliders(&mut assets).expect("the hit colliders");
            let health = Arc::new(HealthSettings::new(&db.config).expect("health settings"));
            Some(Makings { db, hard, engine, akm, empty, hands_data, akm_animator, empty_animator, firearm, body, movement, limbs, colliders, health })
        })
        .as_ref()
}

/// A character made as the app makes a dummy's, its AKM in hand or (where
/// not `firearm`) the empty hands, its poses drawn smooth where `smooth`.
pub(super) fn made_for_economy(k: &Makings, firearm: bool, smooth: bool) -> Puppet {
    let pacer = Pacer::new(trp_sim::REFERENCE_FRAME_RATE, k.engine.fixed_timestep, k.engine.maximum_timestep);
    if firearm {
        let hands = Hands::new(trp_sim::hands_settings(k.hands_data.clone(), k.db, &k.hard, &k.engine, &k.akm, ViewSettings::default()).expect("hands settings"));
        let movement = Movement::new(k.movement.clone(), trp_sim::Body::new(&k.body, k.akm.cell_width as f32));
        Puppet::new(movement, hands, Firearm::new(k.firearm.clone(), &k.akm_animator), &k.akm.hierarchy, k.limbs, k.akm.cell_width as f32, pacer, smooth).expect("the puppet")
    } else {
        let mut hands = Hands::new(trp_sim::empty_hands_settings(k.hands_data.clone(), k.db, &k.hard, &k.engine, &k.empty, ViewSettings::default()).expect("the empty hands' motion"));
        hands.keep_from(hands.kept());
        let movement = Movement::new(k.movement.clone(), trp_sim::Body::new(&k.body, k.empty.cell_width as f32));
        Puppet::empty_handed(movement, hands, trp_sim::EmptyHands::new(&k.empty_animator), &k.empty.hierarchy, k.limbs, k.empty.cell_width as f32, pacer, smooth).expect("the puppet")
    }
}

/// The lockstep's characters: what each does, by the time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Scene {
    /// Its AKM in hand: standing (crouching at 1.5 s, leaning right from
    /// 2.5 to 3.5 s), walking, reloading (at 0.3 and 3.5 s), hit in the
    /// chest walking (at 0.5 and 2.2 s), a leg broken walking (at 0.5 s),
    /// killed by a shot to the head (at 1.2 s).
    Standing,
    Walking,
    Reloading,
    Hit,
    BrokenLeg,
    Killed,
    /// Its hands empty: lying down at 0.8 s and crawling from 2 s; walking
    /// and sprinting from 1.5 to 2.5 s.
    EmptyLyingDown,
    EmptyWalking,
}

pub(super) const SCENES: [Scene; 8] = [Scene::Standing, Scene::Walking, Scene::Reloading, Scene::Hit, Scene::BrokenLeg, Scene::Killed, Scene::EmptyLyingDown, Scene::EmptyWalking];

impl Scene {
    pub(super) fn firearm(self) -> bool {
        !matches!(self, Scene::EmptyLyingDown | Scene::EmptyWalking)
    }

    /// What it does at the draw step `step` of `dt` seconds: the keys, and
    /// what befalls it (a reload asked, a hit, a break, a death).
    pub(super) fn act(self, step: usize, dt: f32, p: &mut Puppet, h: &mut Health) -> Input {
        let t = step as f32 * dt;
        let at = |seconds: f32| step == (seconds / dt).round() as usize;
        let walk = Input { move_dir: [if (t / 1.25) as u32 % 2 == 0 { 1.0 } else { -1.0 }, 0.0], ..Input::default() };
        match self {
            Scene::Standing => {
                if at(1.5) {
                    Input { crouch: true, ..Input::default() }
                } else if (2.5..3.5).contains(&t) {
                    Input { lean: 1.0, ..Input::default() }
                } else {
                    Input::default()
                }
            }
            Scene::Walking => walk,
            Scene::Reloading => {
                if at(0.3) || at(3.5) {
                    p.firearm_mut().expect("its AKM").reload_with(30);
                }
                Input::default()
            }
            Scene::Hit => {
                if at(0.5) || at(2.2) {
                    p.movement.hit(32.5, Part::Chest, DamageKind::Bullet);
                    h.apply_damage(Part::Chest, 30.0, DamageKind::Bullet);
                }
                walk
            }
            Scene::BrokenLeg => {
                if at(0.5) {
                    h.do_fracture(Part::LeftLeg);
                }
                walk
            }
            Scene::Killed => {
                if at(1.2) {
                    h.apply_damage(Part::Head, 100.0, DamageKind::Bullet);
                }
                if h.alive { walk } else { Input::default() }
            }
            Scene::EmptyLyingDown => {
                if at(0.8) {
                    Input { prone: true, ..Input::default() }
                } else if t >= 2.0 {
                    Input { move_dir: [0.0, 1.0], ..Input::default() }
                } else {
                    Input::default()
                }
            }
            Scene::EmptyWalking => Input { sprint: (1.5..2.5).contains(&t), move_dir: [walk.move_dir[0], if (1.5..2.5).contains(&t) { 1.0 } else { 0.0 }], ..Input::default() },
        }
    }
}

/// Everything of a character's simulation a test can read, written out:
/// its movement, its hands' motion, its health and the thing in its hands,
/// and its newest recipe's figures. Two the same are the same to the bit
/// (Rust writes each float as the shortest that reads back the same).
pub(super) fn simulation_of(p: &Puppet, h: &Health) -> String {
    let m = &p.movement;
    let hands = &p.hands;
    let recipe = p.recipe().map(|r| (r.frame, r.position, r.yaw, r.pitch, r.body_at, r.body_turn, r.ribcage, r.bound));
    let held = p.firearm().map(|f| (f.animator().state().0.to_owned(), f.animator().state().1, f.reloading(), f.aiming(), f.hands_busy()));
    format!(
        "{:?} {:?} {:?} {} {} {} {} {} {} {} {} {} | {:?} {:?} {:?} {} {:?} | {:?} | {:?} | {h:?}",
        m.position,
        m.velocity,
        m.state,
        m.yaw,
        m.pitch,
        m.body_yaw,
        m.pose_level,
        m.smoothed_pose_level,
        m.prone,
        m.stamina.legs.current,
        m.stamina.arms.current,
        m.speed_limit(),
        hands.weapon_root,
        hands.elbow_shift(true),
        hands.elbow_shift(false),
        hands.turn_away_value(),
        hands.weapon_line(),
        held,
        recipe,
    )
}

/// Every float of a pose, as bits.
pub(super) fn pose_bits(p: &PuppetPose) -> Vec<u32> {
    let placed = |(at, turn): &([f32; 3], [f32; 4])| at.iter().chain(turn.iter()).map(|v| v.to_bits()).collect::<Vec<_>>();
    let mut out: Vec<u32> = p.position.iter().chain([p.yaw, p.pitch].iter()).chain(p.body_at.iter()).chain(p.body_turn.iter()).chain(p.ribcage.iter()).map(|v| v.to_bits()).collect();
    for bone in p.bones.iter().chain(&p.rig).chain(p.arms.iter().flatten()).chain(p.straight_arms.iter().flatten()) {
        out.extend(placed(bone));
    }
    out
}

/// A small generator of the asks' frames (xorshift), seeded.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn chance(&mut self, share: f64) -> bool {
        (self.next() % 1_000_000) as f64 / 1_000_000.0 < share
    }
}

/// The puppet's economy in lockstep: eight characters (the AKM in hand
/// standing, crouching and leaning, walking, reloading, hit, a leg broken,
/// killed; the empty hands lying down and crawling, walking and
/// sprinting), each made twice and stepped with the same keys and the same
/// blows, A posed in full and B posed only when asked, B asked at seeded
/// random frames as a bullet would (a kept frame's pose, the pose drawn)
/// and now and then posed in full for a while. On every step the two
/// simulations are the same to the bit (movement, the hands' motion,
/// health, the firearm, the recipe and its bound); every pose B is asked
/// for is A's of the same frame to the bit, and so is what B draws; on the
/// first frame posed in full after asked, B's pose and what it draws are
/// A's. At draws of 60 and 144 a second (two frames a draw, and none or
/// one), with the poses drawn smooth and not. B left most frames unposed.
#[test]
fn a_pose_asked_for_is_the_pose_made_at_once() {
    let Some(k) = makings() else { return };
    let mut asked_total = 0;
    for (dt, smooth) in [(1.0f32 / 60.0, true), (1.0 / 144.0, true), (1.0 / 60.0, false), (1.0 / 144.0, false)] {
        let steps = (5.0 / dt).round() as usize;
        let (mut asked, mut drawn, mut switched, mut unposed, mut frames) = (0, 0, 0, 0, 0);
        for (number, scene) in SCENES.into_iter().enumerate() {
            let mut a = made_for_economy(k, scene.firearm(), smooth);
            let mut b = made_for_economy(k, scene.firearm(), smooth);
            let (mut ha, mut hb) = (economy_health(k), economy_health(k));
            b.posing(Posing::Asked);
            let mut draws = Draws(0x9e37_79b9_7f4a_7c15 ^ (number as u64 + 1) * 7919 ^ (dt.to_bits() as u64) << 20 ^ u64::from(smooth));
            for step in 0..steps {
                let full_before = b.how_posed() == Posing::Full;
                // Now and then posed in full for a while, and back.
                if draws.chance(if full_before { 0.2 } else { 0.02 }) {
                    b.posing(if full_before { Posing::Asked } else { Posing::Full });
                }
                let (input_a, input_b) = (scene.act(step, dt, &mut a, &mut ha), scene.act(step, dt, &mut b, &mut hb));
                assert_eq!(format!("{input_a:?}"), format!("{input_b:?}"));
                let before = b.newest_frame();
                a.step_with(dt, &input_a, &Flat, Some(&mut ha));
                b.step_with(dt, &input_b, &Flat, Some(&mut hb));
                assert_eq!(simulation_of(&a, &ha), simulation_of(&b, &hb), "{scene:?} at {dt} s, smooth {smooth}, step {step}");
                let Some(newest) = b.newest_frame() else { continue };
                let new_frames = newest.wrapping_sub(before.unwrap_or(u64::MAX));
                frames += new_frames;
                let first_full = !full_before && b.how_posed() == Posing::Full && new_frames > 0;
                if first_full {
                    switched += 1;
                    assert!(b.is_posed(newest), "posed at once on its first frame in full");
                    assert_eq!(pose_bits(b.pose().unwrap()), pose_bits(a.pose().unwrap()), "{scene:?}: the first frame in full, step {step}");
                    assert_eq!(pose_bits(&b.drawn().unwrap()), pose_bits(&a.drawn().unwrap()), "{scene:?}: drawn on the first frame in full, step {step}");
                    drawn += 1;
                }
                if b.how_posed() == Posing::Asked {
                    unposed += (newest + 1 - new_frames..=newest).filter(|n| !b.is_posed(*n)).count() as u64;
                }
                // Asked as a bullet would: a kept frame's pose.
                if draws.chance(0.3) {
                    let oldest = (0..=newest).rev().take_while(|n| b.recipe_of_frame(*n).is_some()).last().unwrap_or(newest);
                    let frame = oldest + draws.next() % (newest - oldest + 1);
                    let (pa, pb) = (a.pose_of_frame(frame).expect("A keeps it"), b.pose_of_frame(frame).expect("B keeps it"));
                    assert_eq!(pose_bits(pb), pose_bits(pa), "{scene:?} at {dt} s, smooth {smooth}: frame {frame} asked at step {step}");
                    asked += 1;
                }
                // And drawn.
                if draws.chance(0.3) {
                    assert_eq!(pose_bits(&b.drawn().unwrap()), pose_bits(&a.drawn().unwrap()), "{scene:?} at {dt} s, smooth {smooth}: drawn at step {step}");
                    drawn += 1;
                }
            }
            if scene == Scene::Killed {
                assert!(!ha.alive && !hb.alive, "killed");
            }
        }
        println!("{:.0} draws a second, smooth {smooth}: {frames} frames of 8 characters; {asked} kept frames asked for, {drawn} drawn, {switched} first frames in full, the same to the bit; {unposed} frames left unposed as made", 1.0 / dt);
        assert!(asked > 200 && drawn > 200 && switched > 4, "{asked} {drawn} {switched}");
        assert!(unposed * 2 > frames, "most frames asked for nothing were left unposed: {unposed} of {frames}");
        asked_total += asked;
    }
    assert!(asked_total > 1000);
}

/// What the economy saves, by how many characters (this step's effect
/// alone: the frame's cost check stays paused): n characters (the eight scenes in turn) stepped at 60 draws
/// a second (two of their frames a draw), each way the app poses one by its
/// size on screen: in full and drawn between frames on every draw; asked,
/// its newest frame posed every 4th of its frames; asked, nothing posed.
/// The time of a draw's steps and poses printed for each; the simulations
/// the same every way. `TRP_PUPPETS=8,50,200` for more than the eight.
#[test]
fn what_the_economy_saves_by_how_many() {
    let Some(k) = makings() else { return };
    let counts: Vec<usize> = std::env::var("TRP_PUPPETS").ok().map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_else(|| vec![8]);
    let dt = 1.0 / 60.0;
    let steps = 30;
    for n in counts {
        let mut said = Vec::new();
        let mut ends: Option<Vec<String>> = None;
        for way in ["in full, drawn every draw", "asked, its newest posed every 4th frame", "asked, nothing posed"] {
            let scenes: Vec<Scene> = (0..n).map(|i| SCENES[i % SCENES.len()]).collect();
            let mut puppets: Vec<(Puppet, Health)> = scenes.iter().map(|scene| (made_for_economy(k, scene.firearm(), true), economy_health(k))).collect();
            let full = way.starts_with("in full");
            for (p, _) in &mut puppets {
                p.posing(if full { Posing::Full } else { Posing::Asked });
            }
            let began = std::time::Instant::now();
            for step in 0..steps {
                for ((p, h), scene) in puppets.iter_mut().zip(&scenes) {
                    let input = scene.act(step, dt, p, h);
                    let before = p.newest_frame();
                    p.step_with(dt, &input, &Flat, Some(h));
                    if full {
                        std::hint::black_box(p.drawn());
                    } else if way.contains("4th") {
                        let newest = p.newest_frame().unwrap_or(0);
                        if before.is_none_or(|before| newest / 4 != before / 4) {
                            std::hint::black_box(p.pose());
                        }
                    }
                }
            }
            let each = began.elapsed().as_secs_f64() * 1000.0 / steps as f64;
            said.push(format!("{way} {each:.2} ms"));
            let now: Vec<String> = puppets.iter().map(|(p, h)| simulation_of(p, h)).collect();
            if let Some(ends) = &ends {
                assert!(*ends == now, "{n} characters, {way}: the simulations differ");
            }
            ends = Some(now);
        }
        println!("{n} characters, a draw's steps and poses: {}", said.join("; "));
    }
}
