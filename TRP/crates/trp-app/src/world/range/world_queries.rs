//! Solids and placed things moved, and what the range answers about its world.

use super::*;

impl Solid {
    pub(super) fn corners(&self) -> (Vec3, Vec3) {
        (self.centre - self.size / 2.0, self.centre + self.size / 2.0)
    }

    /// The box in the simulation's axes (z ahead): its corners in its own
    /// axes about its middle, its turn the other way about the upright.
    pub(super) fn block(&self) -> Block {
        let (low, high) = self.corners();
        Block { low: [low.x, low.y, -high.z], high: [high.x, high.y, -low.z], turn: -self.yaw }
    }

    /// How far along the line (Bevy's axes) the box is entered, as a share
    /// of the line. A line that starts inside it, or runs along a face,
    /// does not enter it.
    pub(super) fn entered(&self, from: Vec3, to: Vec3) -> Option<f32> {
        self.block().entered(in_sim(from), in_sim(to))
    }

    /// Its own axes in the simulation's (z ahead), and how far it reaches
    /// along each.
    pub(super) fn own_axes(&self) -> ([Vec3; 3], Vec3) {
        let (sin, cos) = self.yaw.sin_cos();
        ([Vec3::new(cos, 0.0, sin), Vec3::Y, Vec3::new(-sin, 0.0, cos)], self.size / 2.0)
    }
}

impl Placed {
    /// Where what moves of it is drawn, as it stands now, in Bevy's axes,
    /// and its size and colour; nothing for a thing with no moving part.
    pub fn part_shown(&self) -> Option<(Transform, Vec3, Color)> {
        match self.part {
            Part::None => None,
            Part::Leaf { size, color } => {
                let hinge = self.thing.hinge?;
                let along = in_bevy(turn(hinge.forward, hinge.axis, self.thing.angle()));
                let at = in_bevy(hinge.at) + along * size.x / 2.0 + Vec3::Y * size.y / 2.0;
                Some((Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::X, along)), size, color))
            }
            Part::Moving { centre, size, yaw, pivot, axis, slide, color } => {
                let turned = Quat::from_axis_angle(axis, self.thing.angle().to_radians());
                let at = pivot + turned * (centre - pivot) + slide * self.thing.out();
                Some((Transform::from_translation(at).with_rotation(turned * Quat::from_rotation_y(yaw)), size, color))
            }
        }
    }

    /// What a line of sight meets of it, in the simulation's axes.
    fn met(&self, site: &Site) -> Block {
        self.leaf.map_or(self.hit, |leaf| site.blocks[leaf])
    }
}

impl Range {
    /// Where a prop stands, by its name and the place it is made about.
    pub(super) fn stand_of(&self, name: &str, anchor: Vec3) -> crate::world::level::Stand {
        self.props.iter().find(|prop| prop.name == name).map_or(crate::world::level::Stand::made(anchor), |prop| prop.stand)
    }

    /// A start and its view, where the level file puts it, else as made
    /// with its prop where that stands (Bevy's axes, the view's turn in
    /// degrees right of the lane and below level).
    pub(super) fn start(&self, name: &str) -> Option<(Vec3, [f32; 2])> {
        let made = made_start(name);
        if let Some(found) = self.level.as_ref().and_then(|level| level.starts.get(name)) {
            return Some((found.feet, [found.yaw, made.map_or(0.0, |(_, view, _)| view[1])]));
        }
        let (feet, view, owner) = made?;
        Some(match owner {
            Some((prop, anchor)) => {
                let stand = self.stand_of(&prop, anchor);
                (stand.point(anchor, feet), [view[0] - stand.yaw.to_degrees(), view[1]])
            }
            None => (feet, view),
        })
    }

    /// The place in the range a capture can start the character at: the
    /// far edge of the platform, the shelf, the foot of the stairs; before
    /// the doors on the lane's side, or past them; or before one of the
    /// things to work or take up, within reach of it with the view turned
    /// as `start_view` says; or a level file's (`start <name>`). In the
    /// simulation's axes, feet on the floor.
    pub fn place(&self, name: &str) -> Option<[f32; 3]> {
        let (feet, _) = self.start(name)?;
        let at = [feet.x, -feet.z];
        Some([at[0], self.ground_height(at[0], at[1]), at[1]])
    }

    /// What `--export-level` writes (`level`): the boxes but the props',
    /// each prop where it stands with its parts, the starts, the lamps and
    /// the indoors.
    pub fn layout(&self) -> crate::world::level::Layout {
        use crate::world::level::{LevelBox, PropLayout, Start};
        let as_box = |name: String, solid: &Solid| LevelBox { name, centre: solid.centre, size: solid.size, yaw: solid.yaw, color: solid.color, surface: solid.surface };
        let of_prop: Vec<bool> = (0..self.solids.len()).map(|at| self.props.iter().any(|prop| prop.solids.contains(&at))).collect();
        let boxes = self.solids.iter().zip(&of_prop).filter(|(_, of)| !**of).map(|(solid, _)| as_box(solid.name.to_owned(), solid)).collect();
        let props = self
            .props
            .iter()
            .map(|prop| {
                let mut parts: Vec<LevelBox> = self.solids[prop.solids.clone()].iter().map(|solid| as_box(String::new(), solid)).collect();
                // What moves of its things, as it stands now.
                for placed in &self.things[prop.things.clone()] {
                    if let Some((place, size, color)) = placed.part_shown() {
                        let x = place.rotation * Vec3::X;
                        parts.push(LevelBox { name: String::new(), centre: place.translation, size, yaw: (-x.z).atan2(x.x), color, surface: trp_sim::sound::steps::CONCRETE });
                    }
                }
                // The props without solids, drawn by their looks.
                let flat = |at: Vec3, size: Vec3, yaw: f32, color: Color| LevelBox { name: String::new(), centre: at, size, yaw, color, surface: trp_sim::sound::steps::CONCRETE };
                match prop.name.as_str() {
                    WIRE_PROP => parts.extend(barbed_wire_look().into_iter().map(|(place, size)| flat(place.translation, size, prop.stand.yaw, Color::srgb(0.22, 0.20, 0.17)))),
                    // Each square named by its surface's number.
                    SAMPLER_PROP => parts.extend(surface_squares().iter().map(|square| LevelBox {
                        name: format!("{SAMPLER_PROP} part {}", square.surface + 1),
                        ..flat(square.centre + Vec3::Y * 0.005, Vec3::new(square.size.x - 0.04, 0.01, square.size.y - 0.04), square.yaw, square.color)
                    })),
                    ROUNDS_PROP => parts.push(flat(prop.stand.at + Vec3::Y * 0.02, Vec3::new(0.3, 0.04, 0.15), prop.stand.yaw, Color::srgb(0.6, 0.45, 0.2))),
                    _ => {}
                }
                PropLayout { name: prop.name.clone(), stand: prop.stand, parts }
            })
            .collect();
        let mut starts: Vec<(String, Start, Option<String>)> = Vec::new();
        let mut names = start_names();
        if let Some(level) = &self.level {
            let mut more: Vec<String> = level.starts.keys().filter(|name| !names.contains(name)).cloned().collect();
            more.sort();
            names.extend(more);
        }
        for name in names {
            let Some((feet, [yaw, _])) = self.start(&name) else { continue };
            let owner = made_start(&name).and_then(|(_, _, owner)| owner).map(|(prop, _)| prop);
            starts.push((name, Start { feet, yaw }, owner));
        }
        let indoors = indoors().map(|(centre, size, yaw)| LevelBox { name: "indoors".to_owned(), centre, size, yaw, color: Color::WHITE, surface: trp_sim::sound::steps::CONCRETE });
        crate::world::level::Layout { boxes, props, starts, lamps: hall_lamps(), indoors }
    }

    /// Every box of the range as it stands, in Bevy's axes (middle, size,
    /// turn about the upright): the character's collision's (the solids
    /// and the doors' leaves) and the things' own (the containers, the
    /// tables). What a corpse falls against (`death`).
    pub fn boxes(&self) -> Vec<(Vec3, Vec3, Quat)> {
        let things = self.things.iter().filter(|placed| placed.leaf.is_none()).map(|placed| &placed.hit);
        self.site
            .blocks
            .iter()
            .chain(things)
            .map(|block| {
                let middle = [(block.low[0] + block.high[0]) / 2.0, (block.low[1] + block.high[1]) / 2.0, (block.low[2] + block.high[2]) / 2.0];
                let size = Vec3::new(block.high[0] - block.low[0], block.high[1] - block.low[1], block.high[2] - block.low[2]);
                // The game's turn about straight up, in Bevy's mirrored
                // axes: the other way about.
                (in_bevy(middle), size, Quat::from_rotation_y(-block.turn))
            })
            .collect()
    }

    /// Which way to look from a place of `place` (degrees right of the
    /// lane, degrees below level) for what is there to be in reach; at a
    /// level file's start its arrow's way, the pitch the range's own start
    /// of that name has (level, for one of the file's own).
    pub fn start_view(&self, name: &str) -> Option<[f32; 2]> {
        self.start(name).map(|(_, view)| view)
    }

    /// Locks one of the range's doors, the first or the second, with the
    /// key of that item template.
    pub fn lock_door(&mut self, which: usize, key: &str) {
        if let Some(door) = self.things.iter_mut().filter(|placed| placed.name == "door").nth(which) {
            door.thing.lock(Some(key.to_owned()));
        }
    }

    /// Moves every thing on by a frame of the simulation's clock: the game
    /// runs their routines after the character's update, so this comes
    /// after the character's frame. A door's leaf is put where it now stands.
    pub fn step_things(&mut self, dt: f32) {
        let Some(settings) = self.thing_settings.as_ref() else { return };
        for placed in &mut self.things {
            placed.thing.step(dt, settings);
            if let Some(leaf) = placed.leaf {
                self.site.blocks[leaf] = leaf_block(&placed.thing);
            }
        }
    }

    /// What a spent case's cast meets (`BouncingObject.Raycast` on the
    /// shells' mask, the simulation's axes): the first of the solids, the
    /// doors' leaves (wood) and the ground (a square of the sampler's, or
    /// concrete), as the steps' line down finds them, with how far, where,
    /// the face's normal and the surface.
    pub fn cast(&self, from: [f32; 3], way: [f32; 3], reach: f32) -> Option<trp_sim::combat::shells::Hit> {
        let length = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2]).sqrt();
        if length < 1e-9 {
            return None;
        }
        let to: [f32; 3] = std::array::from_fn(|k| from[k] + way[k] / length * reach);
        let solids = self.solids.iter().filter_map(|solid| solid.block().entered_face(from, to).map(|(t, normal)| (t, normal, solid.surface)));
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered_face(from, to).map(|(t, normal)| (t, normal, WOOD)));
        let ground = (from[1] >= 0.0 && to[1] < 0.0).then(|| {
            let t = from[1] / (from[1] - to[1]);
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t);
            (t, [0.0, 1.0, 0.0], ground_surface(at[0], -at[2]))
        });
        let (t, normal, surface) = solids.chain(leaves).chain(ground).min_by(|a, b| a.0.total_cmp(&b.0))?;
        Some(trp_sim::combat::shells::Hit { distance: t * reach, point: std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t), normal, surface })
    }

    /// What a bullet's cast meets (`trp_sim::ballistics::BulletWorld`;
    /// ballistics spec, "Surfaces"): each box the ray enters through a face
    /// turned toward it (the roofs too; nothing it starts inside), each
    /// door's leaf, and the ground from above, each once, with the preset
    /// of the material its surface gives (the ground's by the sampler's
    /// square under the hit). Every box is a thing of its own; the ground
    /// is one collider whose thing is its material, as the game's terrain
    /// is (`TerrainBallistic`).
    pub fn bullet_hits(&self, from: [f32; 3], to: [f32; 3], presets: &[trp_data::setup::hard_settings::BallisticPreset], hits: &mut Vec<trp_sim::combat::ballistics::RayHit>) {
        use trp_sim::combat::ballistics::{material, preset_surface, RayHit};
        let way: [f32; 3] = std::array::from_fn(|k| to[k] - from[k]);
        let length = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2]).sqrt();
        if length < 1e-9 {
            return;
        }
        let at = |t: f32| -> [f32; 3] { std::array::from_fn(|k| from[k] + way[k] * t) };
        for (index, solid) in self.solids.iter().enumerate() {
            if let Some((t, normal)) = solid.block().entered_face(from, to) {
                let id = index as u64 + 1;
                hits.push(RayHit { distance: t * length, point: at(t), normal, collider: id, thing: id, mesh: false, surface: preset_surface(presets, material::of_surface(solid.surface)), body: None });
            }
        }
        for (index, leaf) in self.things.iter().enumerate().filter_map(|(index, placed)| placed.leaf.map(|leaf| (index, leaf))) {
            if let Some((t, normal)) = self.site.blocks[leaf].entered_face(from, to) {
                let id = BULLET_LEAVES + index as u64;
                hits.push(RayHit { distance: t * length, point: at(t), normal, collider: id, thing: id, mesh: false, surface: preset_surface(presets, material::of_surface(WOOD)), body: None });
            }
        }
        let ground = self.site.ground;
        if from[1] > ground && to[1] <= ground {
            let t = (from[1] - ground) / (from[1] - to[1]);
            let point = at(t);
            let made_of = material::of_surface(ground_surface(point[0], -point[2]));
            hits.push(RayHit { distance: t * length, point, normal: [0.0, 1.0, 0.0], collider: BULLET_GROUND, thing: BULLET_GROUND + 1 + made_of as u64, mesh: false, surface: preset_surface(presets, made_of), body: None });
        }
    }

    /// The other characters standing on the range, whose capsules the
    /// character's collision meets (`trp_sim::collision::Upright`): the
    /// living dummies, as they stood after their last move.
    pub fn set_others(&mut self, others: Vec<trp_sim::locomotion::collision::Upright>) {
        self.site.others = others;
    }

    /// How far along the line from `from` to `to` (the simulation's axes)
    /// the first of the range's boxes or the ground is met, as a share of
    /// it: what a blast's line meets (`HighPolyWithTerrainMask`: the world's
    /// solids and its terrain). The range's boxes are the app's own, and no
    /// fragment pierces them (`PROVISIONAL`: they have no ballistic
    /// materials).
    pub fn entered(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let ground = self.site.ground;
        let under = (from[1] >= ground && to[1] < ground).then(|| (from[1] - ground) / (from[1] - to[1]));
        let things = self.things.iter().filter(|placed| placed.leaf.is_none()).map(|placed| &placed.hit);
        self.site.blocks.iter().chain(things).filter_map(|block| block.entered(from, to)).chain(under).min_by(f32::total_cmp)
    }

    /// Which thing a line of sight from `from` to `to` (Bevy's axes) meets
    /// first, before anything else solid, within `door_reach` of `from`
    /// for a door and anywhere along the line for the rest
    /// (`Player.InteractionRaycast`: a door within `DOOR_RAYCAST_DISTANCE`
    /// and the rest within `LOOT_RAYCAST_DISTANCE`, each with `BEHIND_CAST`).
    pub fn looked_at(&self, from: Vec3, to: Vec3, door_reach: f32) -> Option<usize> {
        let ground = (from.y >= 0.0 && to.y < 0.0).then(|| from.y / (from.y - to.y)).unwrap_or(f32::INFINITY);
        let solid = self.solids.iter().filter_map(|solid| solid.entered(from, to)).fold(ground, f32::min);
        let (sim_from, sim_to) = (in_sim(from), in_sim(to));
        let (index, share) = self
            .things
            .iter()
            .enumerate()
            .filter_map(|(index, placed)| placed.met(&self.site).entered(sim_from, sim_to).map(|share| (index, share)))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let length = from.distance(to);
        let near_enough = self.things[index].leaf.is_none() || share * length <= door_reach;
        (share <= solid + 1e-5 && near_enough).then_some(index)
    }
}
