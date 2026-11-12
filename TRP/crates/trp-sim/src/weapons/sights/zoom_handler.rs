//! The zoom handler and the zoom input that drives it.

use super::*;

/// Which end of its range a variable scope is at (`ESmoothScopeState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    Widest,
    Narrowest,
    Between,
}

/// A variable scope's zoom (`ScopeZoomHandler`): its field of view, the
/// target it moves to, the velocity the wheel builds up and the place the
/// wheel has asked for, the last input, and whether it has arrived. Its
/// range and speed are the item's (`MinMaxFov`), not its prefab's.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomHandler {
    pub(super) data: AdjustableOptic,
    pub(super) curve: Curve,
    fov: f32,
    pub(super) target: f32,
    velocity: f32,
    wanted: f32,
    pub(super) input: f32,
    last_input: f32,
    arrived: bool,
    pub(super) edge: Edge,
}

impl ZoomHandler {
    /// Set up on the item's kept zoom (`Init`): none kept is the widest.
    pub(super) fn new(data: AdjustableOptic, curve: Curve, kept: &mut f32) -> Self {
        let [widest, narrowest] = data.fov;
        if *kept == 0.0 {
            *kept = widest;
        }
        let fov = kept.clamp(narrowest, widest);
        let mut handler = Self { data, curve, fov, target: fov, velocity: 0.0, wanted: fov, input: 0.0, last_input: 0.0, arrived: false, edge: Edge::Between };
        // (The game's `Init` tells the screen of an end the zoom starts
        // away from its widest at; the app's start at the widest.)
        handler.note_edge();
        handler
    }

    /// The scope's field of view now, degrees.
    pub fn field_of_view(&self) -> f32 {
        self.fov
    }

    /// How far toward the narrowest it is, 0 to 1 (`ZoomFactor`).
    pub(super) fn factor(&self) -> f32 {
        inverse_lerp(self.data.fov[0], self.data.fov[1], self.fov)
    }

    /// The aiming sensitivity it gives (`method_2`): from
    /// `AdjustableOpticSensitivity` at the widest to its `Max` at the
    /// narrowest.
    pub fn sensitivity(&self) -> f32 {
        lerp(self.data.sensitivity[0], self.data.sensitivity[1], self.factor())
    }

    /// One frame of the wheel (`method_4`): `input` is the wheel's turn
    /// times the options' `OpticSensitivity` (up positive), `dt` the
    /// frame's time. The zoom kept with the item follows. Whether the zoom
    /// came to an end of its range or left one.
    pub(super) fn update(&mut self, input: f32, dt: f32, kept: &mut f32) -> bool {
        let [widest, narrowest] = self.data.fov;
        self.input = input;
        if input != 0.0 {
            // The wheel turned back while the zoom was on its way stops it
            // where it is; else the velocity grows by the turn and the
            // wanted place moves by the velocity (`method_5`).
            let reversed = (input > 0.0 && self.last_input < 0.0) || (input < 0.0 && self.last_input > 0.0);
            if reversed && !self.arrived && self.edge == Edge::Between {
                self.velocity = 0.0;
                self.wanted = self.fov;
            } else {
                self.velocity += input * self.data.zoom_sensitivity;
                self.wanted -= self.velocity;
            }
            self.wanted = self.wanted.clamp(narrowest, widest);
            let along = inverse_lerp(narrowest, widest, self.wanted);
            let shaped = self.curve.evaluate(along);
            self.target = ((widest - narrowest).abs() * shaped + narrowest).clamp(narrowest, widest);
            self.last_input = input;
        }
        self.arrived = (self.fov - self.target).abs() < 0.0001;
        if self.arrived {
            self.velocity = 0.0;
            false
        } else {
            let step = self.data.speed * self.velocity.abs() * dt;
            self.fov = move_towards(self.fov, self.target, step).clamp(narrowest, widest);
            *kept = self.fov;
            self.note_edge()
        }
    }

    /// The magnification key's snap (`ForceChangeScopeState`): to the end
    /// of the range farther from where it is. Whether its end changed.
    pub(super) fn snap(&mut self, kept: &mut f32) -> bool {
        let [widest, narrowest] = self.data.fov;
        self.fov = if (self.fov - widest).abs() > (self.fov - narrowest).abs() { widest } else { narrowest };
        self.target = self.fov;
        self.wanted = self.fov;
        *kept = self.fov;
        self.note_edge()
    }

    /// At an end the wheel's run is forgotten (`method_9`, `method_8`).
    /// Whether it came to an end or left one (`OnSmoothScopeStateChanged`).
    fn note_edge(&mut self) -> bool {
        let [widest, narrowest] = self.data.fov;
        let was = self.edge;
        self.edge = if self.fov == widest {
            Edge::Widest
        } else if self.fov == narrowest {
            Edge::Narrowest
        } else {
            Edge::Between
        };
        if self.edge != Edge::Between {
            (self.velocity, self.input, self.last_input) = (0.0, 0.0, 0.0);
        }
        self.edge != was
    }
}

/// The wheel's input to a variable scope's zoom on a frame
/// (`ScopeZoomHandler.GetZoomInput`), read straight off the keys and the
/// wheel, not through the bindings' commands: the wheel's turn on the
/// frame times `OpticSensitivity` where a variant of `ScopeZoomIn` or
/// `ScopeZoomOut` that ends in the wheel has its keys held (the install's
/// two: `LeftAlt` and the wheel either way), whichever way the wheel went,
/// turned about with the inverted up and down; a variant of keys alone
/// gives the sensitivity, out for `ScopeZoomIn`'s and in for the other's.
/// Wheel up positive, which narrows.
pub fn zoom_input(controls: &crate::base::bindings::Controls, held: &dyn Fn(crate::base::bindings::Code) -> bool, wheel: f32, optic_sensitivity: f32, inverted: bool) -> f32 {
    let scroll = wheel * optic_sensitivity;
    for (name, mut positive) in [("ScopeZoomIn", true), ("ScopeZoomOut", false)] {
        let Some(group) = controls.group(name) else { continue };
        for variant in &controls.keys[group].variants {
            let keys_held = variant.keys.iter().all(|&key| held(key));
            if let Some(up) = variant.wheel {
                // The game's own reading of a wheel's variant: its way is
                // taken against the inverted switch, and the turn must be
                // the other way to it (`Positive`: above nothing), which
                // lets either variant take either way.
                positive = if inverted { up } else { !up };
                if scroll == 0.0 || (scroll > 0.0) != positive {
                    continue;
                }
                if keys_held {
                    return if inverted { -scroll } else { scroll };
                }
            }
            if !variant.keys.is_empty() && keys_held {
                return if positive { -optic_sensitivity } else { optic_sensitivity };
            }
        }
    }
    0.0
}
