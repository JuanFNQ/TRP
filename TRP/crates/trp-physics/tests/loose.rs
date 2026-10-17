//! Things of the install's let fall as the game lets them fall: each one's
//! colliders from its model as it lies loose (`loose::colliders`, a convex
//! mesh's points read with it), let go from a metre and a half with the
//! quick reload's start, and stepped as the game steps until its watch
//! says it is done. Each comes to rest on the ground within the game's 15
//! seconds, its centre no higher than its longest side. Skipped where the
//! install is not there.

use std::sync::Arc;

use trp_physics::loose::{self, Dice, Watch};
use trp_physics::*;

/// The AK-74's 30-round magazine (a box), and a backpack whose only
/// collider is a convex mesh.
const THINGS: [&str; 2] = ["55d480c04bdc2d1d4e8b456a", "5ab8ebf186f7742d8b372e80"];

#[test]
fn things_let_fall_come_to_rest() {
    let Ok(install) = trp_data::Install::locate() else { return };
    let db = trp_data::Database::load(&install).expect("the database");
    let physics = trp_data::engine::PhysicsSettings::load(&install).expect("the physics settings");
    let mut assets = trp_data::unity::Assets::new(install).expect("the install's assets");
    let mut dice = Dice::new(3);
    for template in THINGS {
        let item = db.item(template).expect(template);
        let key = item.prefab_key().expect("a model").to_owned();
        let (hierarchy, _) = trp_data::icon::model_of(&mut assets, &key).expect("the model");
        let colliders = {
            let assets = &assets;
            let mut points = |mesh| trp_data::unity::extract::read_mesh(assets, mesh).ok().map(|m| Arc::from(m.positions));
            loose::colliders(&hierarchy, &mut points)
        };
        assert!(!colliders.is_empty(), "{template}: no collider");
        // At rest its centre is no higher than its longest side.
        let (low, high) = loose::bounds(&colliders).expect("its box");
        let longest = (0..3).map(|axis| high[axis] - low[axis]).fold(0.0f32, f32::max);
        let mut world = World::new(Settings::of(&physics));
        world.add_ground(0.0, Material::default(), physics.layer("Terrain").unwrap());
        let start = loose::let_fall([0.0, 1.5, 0.0], [0.0, 0.0, 0.0, 1.0], &mut dice);
        let weight = item.props.get("Weight").and_then(|w| w.as_f64()).unwrap_or(0.5) as f32;
        let (body, shift) = loose::add(&mut world, colliders, weight, &start, loose::SWEPT_UNDER, physics.layer("Loot").unwrap());
        let mut watch = Watch::default();
        let mut frames = 0;
        while !watch.frame(&world, body, true, 1.0 / 120.0) {
            world.step(1.0 / 120.0);
            frames += 1;
        }
        let (at, turn) = world.pose(body);
        let centre = rotate(turn, shift);
        let height = at[1] + centre[1];
        println!("{template}: done after {:.2} s, its centre {height:.3} m up, asleep {}", frames as f32 / 120.0, world.asleep(body));
        assert!(frames < 15 * 120 + 2, "{template}: never done");
        assert!(height > 0.0 && height < longest, "{template}: its centre {height} m up, its longest side {longest} m");
    }
}
