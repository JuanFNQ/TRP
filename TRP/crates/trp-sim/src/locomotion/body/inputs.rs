//! Making the body, taking its events and setting what drives it.

use super::*;

impl Body {
    /// Blind fire's curve by the state's time for a part (`part::IN`,
    /// `part::OUT`) and a side (1 over the top, -1 to the right), as the
    /// install's state script has it (`SmoothLerpAnimCurve`).
    pub fn blind_fire_curve(&self, part: i32, side: f32) -> Option<&Curve> {
        self.blind_curves[usize::from(part == part::OUT)][usize::from(side < 0.0)].as_ref()
    }

    /// The gestures that came to their end since this was last asked, by
    /// number.
    pub fn take_finished_interactions(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.finished)
    }

    /// Whether the gesture of a backpack's drop has passed its mark for
    /// the pack leaving the shoulders since this was last asked.
    pub fn take_backpack_dropped(&mut self) -> bool {
        std::mem::take(&mut self.backpack_dropped)
    }

    /// Whether a bipod's gesture has passed its toggle's mark since this was
    /// last asked (`OnBipodToggle`).
    pub fn take_bipod_toggled(&mut self) -> bool {
        std::mem::take(&mut self.bipod_toggled)
    }

    /// How many of the crawl's sound marks the body's clips have passed
    /// since this was last asked: their events calling `PlaySoundBank`
    /// with `Prone`, which the player's `GenericEventTranslator` hands on
    /// (`docs/spec/footsteps.md`).
    pub fn take_crawl_marks(&mut self) -> usize {
        std::mem::take(&mut self.crawl_marks)
    }

    /// The marks of the moves over an obstacle the body's states have
    /// passed since this was last asked, in order: each the move
    /// (`VAULT_STATES`' place) and the mark (`VAULT_MARKS`' place).
    pub fn take_vault_marks(&mut self) -> Vec<(usize, usize)> {
        std::mem::take(&mut self.vault_due)
    }

    /// Puts the interactions' layer back to rest outright, whatever it
    /// was playing (the game's `ResetInteraction`).
    pub fn reset_interactions(&mut self) {
        let Some((layer, _)) = self.interactions else { return };
        if let Some(rest) = self.animator.controller.state(layer, INTERACTIONS_REST) {
            self.animator.play(layer, rest, 0.0);
        }
    }

    /// The state the interactions' layer is in and how far through it, and
    /// the state it is on its way into: for looking at.
    pub fn interaction_state(&self) -> Option<(&str, f32, Option<&str>)> {
        let (layer, _) = self.interactions?;
        let (current, next) = self.animator.state(layer);
        Some((current.name.as_str(), self.animator.progress(layer).0, next.map(|state| state.name.as_str())))
    }

    /// A parameter of the body's animator by its name, as the last update
    /// left it: for looking at.
    pub fn parameter(&self, name: &str) -> Option<f32> {
        self.animator.controller.parameter(name).map(|id| self.animator.get(id))
    }

    /// The state a layer of the body's animator is in, how far through it,
    /// and the state it is on its way into, by the layer's name: for
    /// looking at.
    pub fn layer_state(&self, layer: &str) -> Option<(&str, f32, Option<&str>)> {
        let layer = self.animator.controller.layer(layer)?;
        let (current, next) = self.animator.state(layer);
        Some((current.name.as_str(), self.animator.progress(layer).0, next.map(|state| state.name.as_str())))
    }

    /// A body standing still. `weapon_width` is the weapon's width in
    /// inventory cells, which picks how the body carries it.
    pub fn new(data: &BodyData, weapon_width: f32) -> Self {
        let mut animator = Animator::new(data.controller.clone());
        if let Some(size) = data.ids.weapon_size {
            animator.set(size, weapon_width);
        }
        let base_machine = data.controller.layers.first().map_or(0, |l| l.machine);
        let interactions = data.controller.layer(INTERACTIONS_LAYER).map(|layer| (layer, data.controller.layers[layer].machine));
        let mut body = Self {
            interactions,
            finished: Vec::new(),
            backpack_drop: data.backpack_drop,
            backpack_passed: false,
            backpack_dropped: false,
            bipod_marks: data.bipod_marks,
            bipod_passed: false,
            bipod_toggled: false,
            vault_marks: data.vault_marks,
            vault_passed: [[false; 4]; 3],
            vault_due: Vec::new(),
            crawl_marks: 0,
            animator,
            ids: data.ids,
            faded_layer: data.faded_layer,
            sidestep_length: data.sidestep_length,
            pose_changes: Vec::new(),
            base_machine,
            left: Vec::new(),
            drawn: data.drawn,
            drawn_root: data.drawn_root,
            kind: hands_kind::RIFLE,
            kind_now: hands_kind::RIFLE,
            point_of_view: PointOfView::FirstPerson,
            utility_weight: 0.0,
            mounted: [false; 2],
            head_bones: data.head_bones,
            head_shares: data.head_shares,
            head_turn: [0.0; 3],
            blind_curves: data.blind_curves.clone(),
            reach: data.reach,
        };
        // A character is made for its own eyes until told otherwise: the
        // layer of the body's own part in the weapon's handling is off.
        body.animator.set_layer_weight(WEAPON_LAYER, 0.0);
        // Settle the pose and the curves of the starting state: a body
        // that has been standing for a while.
        body.animator.settle();
        body.update(
            0.0,
            &BodyInput {
                speed: 1.0,
                level: 1.0,
                grounded: true,
                transition_speed: 1.0,
                vault_figures: [0.0, 1.0, 0.0, 0.0],
                vaulting_speed: 1.0,
                ..BodyInput::default()
            },
        );
        body
    }

    /// Tells the animator how wide the weapon in hand is, in inventory cells.
    pub fn set_weapon_width(&mut self, cells: f32) {
        if let Some(size) = self.ids.weapon_size {
            self.animator.set(size, cells);
        }
    }

    /// Tells the animator what kind of thing is in the hands (`hands_kind`).
    /// The whole number is the animator's at once; the figure its blend
    /// trees go by follows over the next frames, a share of the way each
    /// frame (`update`), as the game's does for a character that has hands.
    pub fn set_hands_kind(&mut self, kind: f32) {
        self.kind = kind;
        if let Some(id) = self.ids.weapon_type {
            self.animator.set(id, kind);
        }
    }

    /// `set_hands_kind` for a character that has had the thing in hand for
    /// a while: the figure is there already, with none of the way to go.
    pub fn start_hands_kind(&mut self, kind: f32) {
        self.set_hands_kind(kind);
        self.kind_now = kind;
        if let Some(id) = self.ids.weapon_type_float {
            self.animator.set(id, kind);
        }
    }

    /// The figure the animator's trees have for the kind of thing in hand.
    pub fn hands_kind(&self) -> f32 {
        self.kind_now
    }

    /// Whose eyes the character is seen with (`Player.PointOfView`). Seen
    /// from outside the animator has its flag and its figure for that, and
    /// the layer of the body's own part in the weapon's handling is on;
    /// what the clips' curves hand over of the weapon and the arms then
    /// no longer waits on `First_Person_Curve_Weight`, which is taken as
    /// 1 (`carried`, `arm_pose`, `held`). The layer that reaches for the
    /// vest keeps whatever weight it last had, as in the game.
    pub fn set_point_of_view(&mut self, point_of_view: PointOfView) {
        self.point_of_view = point_of_view;
        let outside = point_of_view == PointOfView::ThirdPerson;
        for id in [self.ids.third_person, self.ids.third_person_float].into_iter().flatten() {
            self.animator.set(id, outside as u8 as f32);
        }
        self.animator.set_layer_weight(WEAPON_LAYER, outside as u8 as f32);
    }

    pub fn point_of_view(&self) -> PointOfView {
        self.point_of_view
    }

    /// What the animator of the thing in hand says the body is to do with
    /// it, by the game's number (the mark `ThirdAction` of a weapon's
    /// states: 1 as it comes to the hands, 2 as it leaves them, 3 to 9 a
    /// grenade's, 8 and 12 a knife's strokes, 13 to 15 an axe's). The
    /// animator is told whoever is looking; only a character seen from
    /// outside shows it.
    pub fn set_weapon_action(&mut self, action: i32) {
        if let Some(id) = self.ids.first_action {
            self.animator.set(id, action as f32);
        }
    }

    /// How fast the body's own part in a weapon drawn and put away plays
    /// (`SpeedDraw`: the weapon's animator's figure, told to the body's
    /// too).
    pub fn set_draw_speed(&mut self, speed: f32) {
        if let Some(id) = self.ids.speed_draw {
            self.animator.set(id, speed);
        }
    }

    /// How fast the body's own part in a blade's stroke plays (`SwingSpeed`,
    /// told the body with the blade's animator by `KnifeController.
    /// SetMeleeSpeed`: the character's melee speed). The controller starts
    /// it at 0, so a stroke's state stands still until a blade has told it.
    pub fn set_swing_speed(&mut self, speed: f32) {
        if let Some(id) = self.ids.swing_speed {
            self.animator.set(id, speed);
        }
    }

    /// Whether the blade's stroke glanced off (`Deflected`, told the body
    /// with the blade's animator by `KnifeController.SetDeflected`). Each
    /// stroke's state blends its clip with the same clip by it, so the
    /// body's pose does not change with it.
    pub fn set_deflected(&mut self, deflected: bool) {
        if let Some(id) = self.ids.deflected {
            self.animator.set(id, deflected as u8 as f32);
        }
    }

    /// Weighs the layer that reaches for the vest, for a character seen
    /// from outside, once a frame (`Player.AdjustUtilityLayerWeight`).
    /// `reach` is how far the left hand's marker is from the weapon's
    /// marker for the vest, in metres, while a reload is in hand or the
    /// inventory is open, and the character is not sprinting; `None`
    /// otherwise, and the layer then fades. A weapon with no such marker
    /// has the layer off (`with_markers` false).
    pub fn weigh_reach_for_vest(&mut self, with_markers: bool, reach: Option<f32>, dt: f32) {
        if !with_markers {
            self.animator.set_layer_weight(UTILITY_LAYER, 0.0);
            return;
        }
        self.utility_weight = match reach {
            Some(distance) => ((distance - UTILITY_RANGE[0]) / (UTILITY_RANGE[1] - UTILITY_RANGE[0])).clamp(0.0, 1.0),
            None => self.utility_weight + (0.0 - self.utility_weight) * (dt * UTILITY_FADE).clamp(0.0, 1.0),
        };
        self.animator.set_layer_weight(UTILITY_LAYER, self.utility_weight);
    }

    /// The weight of the layer that reaches for the vest, as last set.
    pub fn reach_for_vest(&self) -> f32 {
        self.animator.layer_weight(UTILITY_LAYER)
    }

    /// How far the free look has the head turned, as the bones take it
    /// (`HeadLook::bones_turn`): after the animator has posed them the
    /// game turns the neck and the head each by its share of it, in the
    /// bone's own axes (`PlayerBones.RotateHead`), for the character's
    /// own eyes (whose head only casts a shadow) and for any other's.
    /// `drawn_bones` gives them so.
    pub fn set_head_turn(&mut self, turn: [f32; 3]) {
        self.head_turn = turn;
    }

    /// The free look's turn as last told (`set_head_turn`): what the
    /// drawn bones of an update's mix are turned by (`drawn_bones_turned`).
    pub fn head_turn(&self) -> [f32; 3] {
        self.head_turn
    }

    /// How far the skeleton's limbs reach at rest, as it was read
    /// (`limbs_reach`): each leg from the pelvis, the neck and the head
    /// from the ribcage, each arm from the ribcage to the palm. Clips turn
    /// a limb's bones and keep their lengths, so no bone of a limb lies
    /// farther from where it starts (a host's broad test: `Puppet::bound`).
    pub fn reach(&self) -> [f32; 3] {
        self.reach
    }

    /// What the clips' curves that hand the weapon and the arms over to
    /// the body are weighed by: the clips' own `First_Person_Curve_Weight`
    /// for the character's own eyes, and 1 for anyone else's.
    pub(super) fn first_person_weight(&self) -> f32 {
        match self.point_of_view {
            PointOfView::FirstPerson => self.ids.first_person.map_or(0.0, |id| self.animator.get(id)),
            PointOfView::ThirdPerson => 1.0,
        }
    }
}
