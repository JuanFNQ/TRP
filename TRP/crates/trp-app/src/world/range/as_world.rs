//! The range as the simulation's World.

use super::*;

impl World for Range {
    fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.site.height(x, z)
    }

    /// A foot that has come down onto the edge of a top it overhangs is
    /// held by it: the game's controller pushes a capsule out of what it
    /// has sunk into, and counts one pushed upward as on the ground
    /// (movement spec, "The character's own collision"). This is how a
    /// climb's end stays up, the axis still short of the top. A character
    /// that walks off a top is not held: it is level with the top when its
    /// axis passes the edge and falls clear of the corner (recorded).
    fn held(&self, at: [f32; 3], capsule: Capsule) -> bool {
        self.site.held(at, capsule, self.radius)
    }

    /// The solids, and the doors' leaves as they stand.
    fn line(&self, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered(from, to));
        let (from, to) = (in_bevy(from), in_bevy(to));
        // The ground, from above only.
        let ground = (from.y >= 0.0 && to.y < 0.0).then(|| from.y / (from.y - to.y));
        let nearest = self.solids.iter().filter_map(|solid| solid.entered(from, to)).chain(ground).chain(leaves).reduce(f32::min)?;
        Some(nearest * length)
    }

    /// The weapon's lines meet the other characters' capsules as well as the
    /// range (`WEAPON_OCCLUSION_LAYERS` holds `Player`), never one a line
    /// starts in (the character's own).
    fn line_meets(&self, from: [f32; 3], to: [f32; 3]) -> Option<(f32, bool)> {
        let length = (0..3).map(|k| (to[k] - from[k]).powi(2)).sum::<f32>().sqrt();
        let world = self.line(from, to).map(|along| (along, false));
        let character = self.site.others.iter().filter_map(|other| other.entered(from, to)).reduce(f32::min).map(|share| (share * length, true));
        match (world, character) {
            (Some(w), Some(c)) => Some(if c.0 < w.0 { c } else { w }),
            (w, c) => w.or(c),
        }
    }

    /// What a step's line down meets first: a box's surface, a door's leaf
    /// (wood), or the ground's where it is hit (a square of the sampler's,
    /// or concrete).
    fn surface(&self, from: [f32; 3], to: [f32; 3]) -> Option<u8> {
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered(from, to)).map(|t| (t, WOOD));
        let (from, to) = (in_bevy(from), in_bevy(to));
        let ground = (from.y >= 0.0 && to.y < 0.0).then(|| from.y / (from.y - to.y)).map(|t| {
            let at = from + (to - from) * t;
            (t, ground_surface(at.x, at.z))
        });
        let solids = self.solids.iter().filter_map(|solid| solid.entered(from, to).map(|t| (t, solid.surface)));
        solids.chain(ground).chain(leaves).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, surface)| surface)
    }

    /// Whatever a line hits above the ground is something standing on it.
    fn terrain(&self, at: [f32; 3]) -> bool {
        at[1] <= 1e-3
    }

    /// The game's own controller's move: out of each box the capsule is
    /// in by the shortest way, straight up where that is mostly upward,
    /// never upward in a vault or a climb, and 5 cm a move out of what it
    /// is deep in (`trp_sim::collision`). The doors' leaves are among them.
    fn obstructed(&self, from: [f32; 3], to: [f32; 3], capsule: Capsule) -> [f32; 3] {
        self.site.settle(from, to, capsule, self.radius)
    }

    /// A door's leaf the capsule is in lifts the hard cap on speed, as the
    /// game's controller lets every door's: a door swung into the character
    /// pushes it out of its way.
    fn lifts_cap(&self, at: [f32; 3], capsule: Capsule) -> bool {
        self.things.iter().filter_map(|placed| placed.leaf).any(|leaf| self.site.blocks[leaf].overlaps(at, capsule, self.radius))
    }

    /// That move puts the feet on the ground and on the boxes' tops too.
    fn sets_feet(&self) -> bool {
        true
    }

    /// A mount's rays (`trp_sim::mounting`): the solids and the doors' leaves
    /// as they stand, each face's normal, and the ground from above. Every
    /// layer the game names is the same here.
    fn ray(&self, from: [f32; 3], direction: [f32; 3], length: f32, _layer: trp_sim::weapons::mounting::Layer) -> Option<trp_sim::weapons::mounting::Hit> {
        let to: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * length);
        let solids = self.solids.iter().filter_map(|solid| solid.block().entered_face(from, to));
        let leaves = self.things.iter().filter_map(|placed| placed.leaf).filter_map(|leaf| self.site.blocks[leaf].entered_face(from, to));
        let ground = (from[1] >= self.site.ground && to[1] < self.site.ground).then(|| ((from[1] - self.site.ground) / (from[1] - to[1]), [0.0, 1.0, 0.0]));
        let (share, normal) = solids.chain(leaves).chain(ground).min_by(|a, b| a.0.total_cmp(&b.0))?;
        let distance = share * length;
        Some(trp_sim::weapons::mounting::Hit { distance, point: std::array::from_fn(|k| from[k] + direction[k] * distance), normal })
    }

    /// The prone mount's room ahead: clear of the solids and above the ground.
    fn sphere_clear(&self, centre: [f32; 3], radius: f32, _layer: trp_sim::weapons::mounting::Layer) -> bool {
        centre[1] - radius >= self.site.ground && !self.solids.iter().any(|solid| solid.block().sphere_overlaps(centre, radius))
    }

    /// The room checks' boxes (movement spec, "Room to move"): against the
    /// site's boxes (the solids and the doors' leaves as they stand) and
    /// under the ground. The box is taken upright, turned by its yaw alone:
    /// the range is level, so a lying body's lay on it is nothing.
    fn box_overlaps(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4]) -> bool {
        let forward = trp_sim::base::math::quat_rotate(turn, [0.0, 0.0, 1.0]);
        let yaw = forward[0].atan2(forward[2]).to_degrees();
        centre[1] - half[1] < self.site.ground || self.site.blocks.iter().any(|block| block.upright_box_overlaps(centre, half, yaw))
    }

    /// A lying body's room ahead and behind: the sphere tried every
    /// centimetre against the site's boxes and the ground (PROVISIONAL: the
    /// game sweeps it whole, which something thinner than a centimetre
    /// between two tries would meet and these not); what it starts in is
    /// not met, as the engine's sweep does not.
    fn sphere_cast(&self, from: [f32; 3], radius: f32, direction: [f32; 3], length: f32) -> Option<f32> {
        const TRY_EVERY: f32 = 0.01;
        let touches = |along: f32| {
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * along);
            at[1] - radius < self.site.ground || self.site.blocks.iter().any(|block| block.sphere_overlaps(at, radius))
        };
        if touches(0.0) {
            return None;
        }
        let tries = (length / TRY_EVERY).ceil().max(1.0) as usize;
        (1..=tries).map(|i| length * i as f32 / tries as f32).find(|along| touches(*along))
    }

    /// The head's capsule (movement spec, "The head's push out of walls"):
    /// out of each of the site's boxes it reaches into (the solids and the
    /// doors' leaves as they stand) by the shortest way, and out of the
    /// ground, each lengthened by a millimetre, added up.
    fn capsule_push(&self, ends: [[f32; 3]; 2], radius: f32) -> [f32; 3] {
        const MARGIN: f32 = 0.001;
        let mut push = [0.0; 3];
        for (way, depth) in self.site.blocks.iter().filter_map(|block| block.capsule_push(ends, radius)) {
            for k in 0..3 {
                push[k] += way[k] * (depth + MARGIN);
            }
        }
        let under = self.site.ground - (ends[0][1].min(ends[1][1]) - radius);
        if under > 0.0 {
            push[1] += under + MARGIN;
        }
        push
    }

    /// A mount's way to its place: the box tried every 5 cm along the way
    /// against the solids (PROVISIONAL: the game sweeps it whole, which a
    /// thin wall between two tries would meet and these not).
    fn box_sweep(&self, from: [f32; 3], half: [f32; 3], yaw: f32, direction: [f32; 3], length: f32, _layer: trp_sim::weapons::mounting::Layer) -> bool {
        const TRY_EVERY: f32 = 0.05;
        let tries = (length / TRY_EVERY).ceil().max(1.0) as usize;
        (0..=tries).any(|i| {
            let along = length * i as f32 / tries as f32;
            let at: [f32; 3] = std::array::from_fn(|k| from[k] + direction[k] * along);
            self.solids.iter().any(|solid| solid.block().upright_box_overlaps(at, half, yaw))
        })
    }

    /// A mounted turn outward: the capsule moved aside against the site's
    /// boxes (PROVISIONAL: the game's prediction also turns the capsule about
    /// its pivot by twice the turn, not felt here: an upright capsule turns into
    /// itself about its own axis).
    fn turn_blocked(&self, at: [f32; 3], shift: [f32; 3], _turn: f32, capsule: Capsule) -> bool {
        let moved: [f32; 3] = std::array::from_fn(|k| at[k] + shift[k]);
        self.site.blocks.iter().any(|block| block.overlaps(moved, capsule, self.radius))
    }

    /// The camera's box is kept out of the solids; the doors' leaves are
    /// not among them.
    fn push_out(&self, centre: [f32; 3], half: [f32; 3], turn: [f32; 4], margin: f32) -> [f32; 3] {
        // The box's own axes, and how far it reaches along a direction.
        let turn = Quat::from_xyzw(turn[0], turn[1], turn[2], turn[3]);
        let (centre, half) = (Vec3::from(centre), Vec3::from(half));
        let axes = [turn * Vec3::X, turn * Vec3::Y, turn * Vec3::Z];
        let reach = |along: Vec3| (0..3).map(|k| along.dot(axes[k]).abs() * half[k]).sum::<f32>();

        // The ground: up by as far as the lowest corner is under it.
        let mut push = Vec3::ZERO;
        let under = reach(Vec3::Y) - centre.y;
        if under > 0.0 {
            push.y += under + margin;
        }
        // Each solid: out along whichever direction it takes the least to
        // part the two boxes (their faces' and their edges' crossings).
        for solid in &self.solids {
            // The solid in the simulation's axes, z ahead, and its own axes.
            let (middle, (faces, size)) = (Vec3::new(solid.centre.x, solid.centre.y, -solid.centre.z), solid.own_axes());
            let between = centre - middle;
            let mut least: Option<(f32, Vec3)> = None;
            let edges = faces.iter().flat_map(|face| axes.iter().map(move |axis| face.cross(*axis)));
            let mut apart = false;
            for along in faces.into_iter().chain(axes).chain(edges) {
                // Two edges that run alike cross in nothing.
                let Some(along) = along.try_normalize() else { continue };
                let gap = between.dot(along);
                let solid_reach = (0..3).map(|k| along.dot(faces[k]).abs() * size[k]).sum::<f32>();
                let overlap = reach(along) + solid_reach - gap.abs();
                if overlap <= 0.0 {
                    apart = true;
                    break;
                }
                if least.is_none_or(|(most, _)| overlap < most) {
                    least = Some((overlap, if gap < 0.0 { -along } else { along }));
                }
            }
            if let (false, Some((overlap, out))) = (apart, least) {
                push += out * (overlap + margin);
            }
        }
        push.into()
    }
}
