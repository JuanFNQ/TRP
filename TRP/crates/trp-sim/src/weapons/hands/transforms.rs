//! Where the weapon is put: plain, shifted and carried.

use super::*;

impl Hands {
    /// Local position and rotation (`xyzw`) of the bone the weapon hangs
    /// from, relative to its rest of no offset and no rotation.
    pub fn weapon_transform(&self) -> ([f32; 3], [f32; 4]) {
        let d = &self.settings.data;
        let outside = self.outside();
        let sway = self.sway.position;
        let hands = self.hands_rotation.value();
        let displacement = self.displacement_strength * if self.aiming { d.aiming_displacement_str } else { 1.0 };
        // Seen from outside the weapon is turned by its rotation spring
        // alone, about the point the recoil turns it about, and the
        // recoil does not turn it (`ApplySimpleRotation`).
        let r = if outside {
            hands
        } else {
            [hands[0] + sway[0] + displacement * sway[0], hands[1] + sway[1], hands[2] + sway[2] + displacement * sway[2]]
        };
        // About y first, then about x and z together.
        let mut rotation = quat_mul(quat_mul(quat_axis(0, r[0]), quat_axis(2, r[2])), quat_axis(1, r[1]));
        let center = if let Some(mount) = self.mounted {
            // Mounted, every turn goes about the mount's pivot.
            mount.pivot
        } else if outside {
            self.settings.recoil_pivot
        } else if self.closer {
            self.settings.rotation_center_no_stock
        } else {
            self.settings.rotation_center
        };
        let swung = quat_rotate(rotation, center);
        let offset = self.hands_position.value();
        let kick = self.recoil.weapon_position();
        let mut position: [f32; 3] = std::array::from_fn(|i| offset[i] + kick[i] + center[i] - swung[i]);

        // Recoil turns the weapon about its own pivot, after everything else.
        let mut recoil = if outside { [0.0; 3] } else { self.recoil.weapon_rotation() };
        if recoil.iter().any(|a| a.abs() > f32::EPSILON) {
            let scale = self.fov_scale();
            if scale < 1.0 && self.shot_squeeze() {
                // A weapon squeezed along the view would otherwise show its
                // climb squeezed too; not once a scope's optic has been
                // turned to its zeroing point (`ShotNeedsFovAdjustments`).
                let adjust = |degrees: f32| (degrees.to_radians().tan() * scale).atan().to_degrees();
                recoil[0] = adjust(recoil[0]);
                recoil[2] = adjust(recoil[2]);
            }
            let turn = quat_euler(recoil);
            let pivot = self.mounted.map_or(self.settings.recoil_pivot, |m| m.pivot);
            let turned = quat_rotate(turn, pivot);
            let shift = quat_rotate(rotation, std::array::from_fn(|i| pivot[i] - turned[i]));
            position = std::array::from_fn(|i| position[i] + shift[i]);
            rotation = quat_mul(rotation, turn);
        }

        // The sights' line levelled: the weapon turned about its muzzle,
        // in its own axes (the game's form turns it by the same angles
        // taken about the world's axes, which differs in the second order).
        if let (true, Some(muzzle)) = (self.sight_angle.abs() > SIGHT_LINE_LEAST, self.muzzle) {
            let turn = quat_axis(0, self.sight_angle);
            let turned = quat_rotate(turn, muzzle);
            let shift = quat_rotate(rotation, std::array::from_fn(|i| muzzle[i] - turned[i]));
            position = std::array::from_fn(|i| position[i] + shift[i]);
            rotation = quat_mul(rotation, turn);
        }

        // A canted sight's turn about the barrel, after the line's, in the
        // bone's own axes about its own place (`SetPositionAndRotation(...,
        // _temporaryRotation * _scopeRotation)`): its place is kept.
        if self.scope_rotation != [0.0, 0.0, 0.0, 1.0] {
            rotation = quat_mul(rotation, self.scope_rotation);
        }

        // Last, the turn-away from an obstacle: drawn back, then turned
        // about a point a little way along the barrel. The game takes the
        // angles as those of a vector turned from the bone's axes into the
        // world's, and turns the bone by them about the world's axes.
        let away = &self.turn_away;
        if away.position != [0.0; 3] || away.rotation != [0.0; 3] {
            position = std::array::from_fn(|i| position[i] + away.position[i]);
            // Mounted, it draws back and does not turn (`AvoidObstacles`).
            if self.mounted.is_some() {
                return (position, rotation);
            }
            let to_world = quat_mul(quat_mul(quat_axis(1, self.view[0]), quat_axis(0, self.view[1])), self.weapon_root.1);
            let back = [-to_world[0], -to_world[1], -to_world[2], to_world[3]];
            let angles = quat_rotate(quat_mul(to_world, rotation), away.rotation);
            let turn = quat_mul(quat_mul(back, quat_euler(angles)), to_world);
            let pivot = quat_rotate(rotation, [0.0, -TURN_AWAY_PIVOT, 0.0]);
            let swung = quat_rotate(turn, pivot);
            position = std::array::from_fn(|i| position[i] + pivot[i] - swung[i]);
            rotation = quat_mul(turn, rotation);
        }
        (position, rotation)
    }

    /// The weapon's moved bone in its parent, once the body has had its say.
    ///
    /// `parent` is where that parent sits in the frame the weapon's rig
    /// hangs in. Where the body's animation claims the weapon, the bone is
    /// drawn toward the body's own weapon bone, keeping the motion worked
    /// out here as an offset from it.
    ///
    /// The game does this before it squeezes the rig for a wide field of
    /// view, so the carried weapon is squeezed like the rest and nothing
    /// is allowed for here. (A recorded sprint shows the swing along the
    /// view at true size in the bone's own figures.)
    ///
    /// Where the body's animation also takes the right hand off the
    /// weapon (`held`), the weapon is then put in that hand: `marker` is
    /// the weapon's right-hand marker in the moved bone's own axes, as
    /// the weapon's animation has it now.
    /// `weapon_transform_carried` with the left shoulder's part
    /// (`PlayerBones.ShiftWeaponRoot`, in first person; left-shoulder spec,
    /// "The weapon"), which keeps its own state from frame to frame. At the
    /// stance's full curve, out of a sprint, the moved bone goes to the
    /// body's own weapon bone with the procedural motion's own place and turn
    /// (`Offset`, `DeltaRotation`) taken onto it: its place at once, or
    /// eased after it (`leftStanceRotationAndPositionChangeSpeed`, 10 a
    /// second) aimed and not walked forward since the stance came in; its
    /// turn eased from the last frame's toward the body's (10 a second, 20
    /// as the animation goes back), turned only about its own y. Aimed, its
    /// place is held at the ribcage's own place in its parent plus a fixed
    /// way (`_offsterDirrectionForLeftStanceInAim` (0.13, -0.305, 0.023)
    /// times 0.2) eased by the frame's time, the recoil's push added, and its
    /// turn eased from the last frame's toward the procedural motion's own.
    /// Below the full curve the weapon goes to the body's bone by the curve
    /// or the body's own authority, whichever is more, turned only about y
    /// while the curve is up; at no curve, as `weapon_transform_carried`.
    /// With a gesture playing it goes there by the body's authority alone and
    /// keeps the procedural motion's own turn (`localRotation =
    /// DeltaRotation`): the body's turn of the weapon is not taken
    /// (recorded: `courses8\headlamp`, the head light's gesture in the prone
    /// transitions at 21.30 s and 23.23 s, the weapon's 18-degree swing gone
    /// on the gesture's frame while its place still follows). With the
    /// body's `Position_cache` up it goes there by that share alone, turn
    /// and all, gesture or not (blind fire's 1: the same recording's gesture
    /// at 30.69 s, over the top, keeps the weapon's 64 degrees).
    pub fn weapon_transform_shifted(
        &mut self,
        parent: ([f32; 3], [f32; 4]),
        third: Carried,
        held: Option<Held>,
        marker: ([f32; 3], [f32; 4]),
        left: LeftShoulder,
    ) -> Shifted {
        self.left_shoulder_curve = left.curve;
        let mut untouched = false;
        let (offset, turn) = self.weapon_transform();
        let (parent_position, parent_rotation) = parent;
        let back = [-parent_rotation[0], -parent_rotation[1], -parent_rotation[2], parent_rotation[3]];
        let hanging = quat_rotate(parent_rotation, offset);
        let mut position: [f32; 3] = std::array::from_fn(|i| parent_position[i] + hanging[i]);
        let mut rotation = quat_mul(parent_rotation, turn);
        let swung = quat_rotate(third.rotation, offset);
        let wanted: [f32; 3] = std::array::from_fn(|i| third.position[i] + swung[i]);
        let own_euler = quat_to_euler(turn);
        // The turn about the moved bone's own y kept, the others as `euler`'s.
        let yaw_only = |rotation: [f32; 4], euler: [f32; 3]| {
            let local = quat_to_euler(quat_mul(back, rotation));
            quat_mul(parent_rotation, quat_euler([euler[0], local[1], euler[2]]))
        };
        let s = &mut self.left_shoulder;
        if left.curve >= 1.0 && !left.sprinting {
            if left.aiming && left.forward {
                s.was_moving_in_aim = true;
            }
            s.target = if left.aiming && !s.was_moving_in_aim { std::array::from_fn(|i| lerp(s.target[i], wanted[i], left.dt * LEFT_SHOULDER_SPEED)) } else { wanted };
            position = s.target;
            if left.aiming {
                let goal: [f32; 3] = std::array::from_fn(|i| left.ribcage_local[i] + LEFT_SHOULDER_AIM_WAY[i] * LEFT_SHOULDER_AIM_REACH);
                s.target_in_aim = std::array::from_fn(|i| lerp(s.target_in_aim[i], goal[i], left.dt));
                let kick = self.recoil.weapon_position();
                let local: [f32; 3] = std::array::from_fn(|i| s.target_in_aim[i] + kick[i]);
                let placed = quat_rotate(parent_rotation, local);
                position = std::array::from_fn(|i| parent_position[i] + placed[i]);
                rotation = yaw_only(quat_lerp(s.last_rotation, rotation, left.dt * LEFT_SHOULDER_SPEED), own_euler);
            } else {
                let goal = quat_mul(third.rotation, turn);
                s.last_rotation = if s.got_full_last_frame {
                    goal
                } else {
                    quat_lerp(s.last_rotation, goal, left.dt * if left.anim_value { LEFT_SHOULDER_SPEED } else { LEFT_SHOULDER_BACK_SPEED })
                };
                rotation = yaw_only(s.last_rotation, own_euler);
            }
            s.last_rotation = rotation;
            s.got_full_last_frame = false;
        } else {
            s.got_full_last_frame = true;
            s.was_moving_in_aim = false;
            // The body's `Position_cache` up takes the weapon by itself, its
            // turn too; else a gesture's takes the place alone.
            let (weight, turned) = if third.position_cache > 0.0 {
                (third.position_cache, true)
            } else if left.interaction {
                (third.authority, false)
            } else if left.curve > 0.0 {
                (left.curve.max(third.authority), true)
            } else {
                (third.authority, true)
            };
            if weight > 0.0 {
                position = std::array::from_fn(|i| lerp(position[i], wanted[i], weight));
                if turned {
                    rotation = quat_lerp(rotation, quat_mul(third.rotation, turn), weight);
                }
            }
            if left.curve > 0.0 && !left.sprinting {
                rotation = yaw_only(rotation, own_euler);
            }
            s.target = position;
            s.target_in_aim = quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i]));
            s.last_rotation = rotation;
            untouched = weight <= 0.0 && left.curve <= 0.0;
        }
        // Nothing moved it: the procedural motion's own, to the bit.
        let carried = if untouched { (offset, turn) } else { (quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i])), quat_mul(back, rotation)) };
        let held_too = if held.is_some_and(|h| h.weight < 1.0) { self.held_onto(parent, position, rotation, held, marker) } else { carried };
        self.shoulder_root = held_too;
        Shifted { carried, held: held_too }
    }

    /// Whether the weapon's motion is aiming (`ProceduralWeaponAnimation.IsAiming`).
    pub fn is_aiming(&self) -> bool {
        self.aiming
    }

    /// The left shoulder's curve as the weapon was last placed by it.
    pub fn left_shoulder_curve(&self) -> f32 {
        self.left_shoulder_curve
    }

    /// The right palm's hold of the weapon (`PlayerBones.Kinematics`), on a
    /// place and turn of the moved bone in the rig's frame, and the result
    /// in the parent's.
    fn held_onto(&self, parent: ([f32; 3], [f32; 4]), mut position: [f32; 3], mut rotation: [f32; 4], held: Option<Held>, marker: ([f32; 3], [f32; 4])) -> ([f32; 3], [f32; 4]) {
        let (parent_position, parent_rotation) = parent;
        if let Some(held) = held.filter(|h| h.weight < 1.0) {
            let (marker_position, marker_rotation) = marker;
            let reach = quat_rotate(rotation, marker_position);
            let marked: [f32; 3] = std::array::from_fn(|i| position[i] + reach[i]);
            let marked_facing = quat_mul(rotation, marker_rotation);
            let palm: [f32; 3] = std::array::from_fn(|i| lerp(held.position[i], marked[i], held.weight));
            let palm_facing = quat_slerp(held.rotation, marked_facing, held.weight);
            let at: [f32; 3] = std::array::from_fn(|i| lerp(palm[i], marked[i], held.weight));
            let facing = quat_lerp(palm_facing, marked_facing, held.weight);
            let unmarked = [-marker_rotation[0], -marker_rotation[1], -marker_rotation[2], marker_rotation[3]];
            rotation = quat_mul(facing, unmarked);
            let reach = quat_rotate(rotation, marker_position);
            position = std::array::from_fn(|i| at[i] - reach[i]);
        }
        let back = [-parent_rotation[0], -parent_rotation[1], -parent_rotation[2], parent_rotation[3]];
        let local = quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i]));
        (local, quat_mul(back, rotation))
    }

    pub fn weapon_transform_carried(
        &self,
        parent: ([f32; 3], [f32; 4]),
        carried: Option<Carried>,
        held: Option<Held>,
        marker: ([f32; 3], [f32; 4]),
    ) -> ([f32; 3], [f32; 4]) {
        let (offset, turn) = self.weapon_transform();
        let carried = carried.filter(|c| c.authority > 0.0);
        let held = held.filter(|h| h.weight < 1.0);
        if carried.is_none() && held.is_none() {
            return (offset, turn);
        }
        let (parent_position, parent_rotation) = parent;

        // Where the bone is now, in the rig's frame.
        let hanging = quat_rotate(parent_rotation, offset);
        let mut position: [f32; 3] = std::array::from_fn(|i| parent_position[i] + hanging[i]);
        let mut rotation = quat_mul(parent_rotation, turn);
        if let Some(carried) = carried {
            // Toward where the body would have it.
            let swung = quat_rotate(carried.rotation, offset);
            let wanted: [f32; 3] = std::array::from_fn(|i| carried.position[i] + swung[i]);
            position = std::array::from_fn(|i| lerp(position[i], wanted[i], carried.authority));
            rotation = quat_lerp(rotation, quat_mul(carried.rotation, turn), carried.authority);
        }
        if let Some(held) = held {
            // The marker goes toward the palm, and the bone to wherever
            // that puts it. By then the arm's own solver, on the same
            // weight, has drawn the palm toward the marker.
            let (marker_position, marker_rotation) = marker;
            let reach = quat_rotate(rotation, marker_position);
            let marked: [f32; 3] = std::array::from_fn(|i| position[i] + reach[i]);
            let marked_facing = quat_mul(rotation, marker_rotation);
            let palm: [f32; 3] = std::array::from_fn(|i| lerp(held.position[i], marked[i], held.weight));
            let palm_facing = quat_slerp(held.rotation, marked_facing, held.weight);
            let at: [f32; 3] = std::array::from_fn(|i| lerp(palm[i], marked[i], held.weight));
            let facing = quat_lerp(palm_facing, marked_facing, held.weight);
            let unmarked = [-marker_rotation[0], -marker_rotation[1], -marker_rotation[2], marker_rotation[3]];
            rotation = quat_mul(facing, unmarked);
            let reach = quat_rotate(rotation, marker_position);
            position = std::array::from_fn(|i| at[i] - reach[i]);
        }

        let back = [-parent_rotation[0], -parent_rotation[1], -parent_rotation[2], parent_rotation[3]];
        let local = quat_rotate(back, std::array::from_fn(|i| position[i] - parent_position[i]));
        (local, quat_mul(back, rotation))
    }

    /// A rendered frame has gone by for a character seen from outside,
    /// whose camera is left where it stands
    /// (`ThirdPersonStrategy.ApplyCameraTransformations`): a host calls
    /// this where it would have called `frame`, and the physics steps a
    /// frame counts are counted afresh from here.
    pub fn frame_outside(&mut self, frame_dt: f32) {
        self.steps_since_frame = 0;
        // Blind fire's blender moves on whoever is looking (the camera's
        // part of it is the character's own eyes').
        self.blind_fire_frame(frame_dt);
        self.feel_shots();
    }
}
