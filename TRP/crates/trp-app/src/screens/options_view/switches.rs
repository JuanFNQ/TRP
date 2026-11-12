//! The FEATURES page's switches, their sections and what each requires.

use super::*;

/// A switch of the FEATURES page, few, with short descriptions and
/// recommended defaults. Twenty-nine in place of the sixty-three of the three
/// lists before it: each stands for the fields it sets
/// (`Switches::features`), on and off together; one that needs others
/// (`requires`) cannot be turned on while any of them is off, and goes off
/// with any of them. The grouping, the defaults and the needs are the
/// app's (settings spec, "Features").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Switch {
    Movement,
    Stamina,
    Weight,
    Health,
    Skills,
    WeaponHandling,
    WeaponMotion,
    Ballistics,
    Attachments,
    WeaponCondition,
    WeaponStances,
    TacticalDevices,
    GrenadesAndKnife,
    Inventory,
    WorldInteractions,
    Looting,
    RaidLimits,
    Targets,
    TarkovUi,
    CharacterBody,
    Sounds,
    VisualEffects,
    TarkovLighting,
    ItemIcons,
    Benches,
    Clothing,
    CompatibleHighlight,
    SmoothAnimations,
    GroundedFeet,
    LedgeClimbing,
    Experience,
    SimpleXpGain,
    Gestures,
    HideUnfinished,
    SeeThroughGlass,
    FaceShieldCracks,
    HeadgearHearing,
}

impl Switch {
    /// Every switch, in the order declared (a switch's place is its
    /// number in `Switches`).
    pub const ALL: [Self; 37] = [
        Self::Movement,
        Self::Stamina,
        Self::Weight,
        Self::Health,
        Self::Skills,
        Self::WeaponHandling,
        Self::WeaponMotion,
        Self::Ballistics,
        Self::Attachments,
        Self::WeaponCondition,
        Self::WeaponStances,
        Self::TacticalDevices,
        Self::GrenadesAndKnife,
        Self::Inventory,
        Self::WorldInteractions,
        Self::Looting,
        Self::RaidLimits,
        Self::Targets,
        Self::TarkovUi,
        Self::CharacterBody,
        Self::Sounds,
        Self::VisualEffects,
        Self::TarkovLighting,
        Self::ItemIcons,
        Self::Benches,
        Self::Clothing,
        Self::CompatibleHighlight,
        Self::SmoothAnimations,
        Self::GroundedFeet,
        Self::LedgeClimbing,
        Self::Experience,
        Self::SimpleXpGain,
        Self::Gestures,
        Self::HideUnfinished,
        Self::SeeThroughGlass,
        Self::FaceShieldCracks,
        Self::HeadgearHearing,
    ];

    /// Its name in the options file and on the command line
    /// (`--feature <key>=on|off`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Movement => "movement",
            Self::Stamina => "stamina",
            Self::Weight => "weight",
            Self::Health => "health",
            Self::Skills => "skills",
            Self::WeaponHandling => "weapon_handling",
            Self::WeaponMotion => "weapon_motion",
            Self::Ballistics => "ballistics",
            Self::Attachments => "attachments",
            Self::WeaponCondition => "weapon_condition",
            Self::WeaponStances => "weapon_stances",
            Self::TacticalDevices => "tactical_devices",
            Self::GrenadesAndKnife => "grenades_and_knife",
            Self::Inventory => "inventory",
            Self::WorldInteractions => "world_interactions",
            Self::Looting => "looting",
            Self::RaidLimits => "raid_limits",
            Self::Targets => "targets",
            Self::TarkovUi => "tarkov_ui",
            Self::CharacterBody => "character_body",
            Self::Sounds => "sounds",
            Self::VisualEffects => "visual_effects",
            Self::TarkovLighting => "tarkov_lighting",
            Self::ItemIcons => "item_icons",
            Self::Benches => "benches",
            Self::Clothing => "clothing",
            Self::CompatibleHighlight => "compatible_items_highlight",
            Self::SmoothAnimations => "smooth_animations",
            Self::GroundedFeet => "grounded_feet",
            Self::LedgeClimbing => "ledge_climbing",
            Self::Experience => "experience",
            Self::SimpleXpGain => "simple_xp_gain",
            Self::Gestures => "gestures",
            Self::HideUnfinished => "hide_unfinished_items",
            Self::SeeThroughGlass => "see_through_glass",
            Self::FaceShieldCracks => "face_shield_cracks",
            Self::HeadgearHearing => "headgear_hearing",
        }
    }

    /// Its name on the page, and what it does, in a line.
    pub fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Movement => ("MOVEMENT", "Walking, sprinting, stances, leaning, jumping and vaulting."),
            Self::Stamina => ("STAMINA", "Legs, arms and breath drain as you act; running dry holds you back."),
            Self::Weight => ("WEIGHT", "What you carry slows you, adds inertia and costs stamina."),
            Self::Health => ("HEALTH", "Body parts, bleeding, fractures, pain, hunger, thirst and death."),
            Self::Skills => ("SKILLS", "Skill levels and weapon mastery from the SKILLS page take effect."),
            Self::WeaponHandling => ("WEAPON HANDLING", "Firing, reloading, checks, fire modes, drawing and holstering."),
            Self::WeaponMotion => ("WEAPON MOTION", "Sway, inertia, breathing and recoil."),
            Self::Ballistics => ("BALLISTICS", "Bullets fly, penetrate and ricochet; hits leave holes, dust and sound."),
            Self::Attachments => ("ATTACHMENTS", "Weapons built from their parts; scopes show a magnified picture."),
            Self::WeaponCondition => ("WEAPON CONDITION", "Wear, malfunctions and overheating."),
            Self::WeaponStances => ("WEAPON STANCES", "Blind fire, mounting and bipods, and the left shoulder."),
            Self::TacticalDevices => ("TACTICAL DEVICES", "Weapon lights, lasers and helmet lights."),
            Self::GrenadesAndKnife => ("GRENADES AND KNIFE", "Throwing grenades; slashing and stabbing with the knife."),
            Self::Inventory => ("INVENTORY", "Gear slots, containers, moving items and quick slots."),
            Self::WorldInteractions => ("WORLD INTERACTIONS", "Doors, containers and items on the ground; dropped items fall."),
            Self::Looting => ("LOOTING", "Unknown items must be examined; crates and bodies must be searched."),
            Self::RaidLimits => ("RAID LIMITS", "Tarkov's limits on how many of an item you may carry or discard in a raid."),
            Self::Targets => ("TARGETS", "Target dummies and steel poppers on the range."),
            Self::TarkovUi => ("TARKOV UI", "Screens drawn as Tarkov's own, from your install."),
            Self::CharacterBody => ("CHARACTER BODY", "Your body in first person, your wrist watch, slung weapons, headgear fitted, your model on GEAR."),
            Self::Sounds => ("SOUNDS", "Footsteps, weapon handling, gunshots and the world's sounds."),
            Self::VisualEffects => ("VISUAL EFFECTS", "Muzzle flash, smoke, explosions, tracers, cases and screen effects."),
            Self::TarkovLighting => ("TARKOV LIGHTING", "Customs' sun, sky and clouds, with Tarkov's camera and shading."),
            Self::ItemIcons => ("ITEM ICONS", "Item pictures rendered from their 3D models, kept between runs."),
            Self::Benches => ("BENCHES", "A gun bench and an armor bench on the range to modify your gear."),
            Self::Clothing => ("CLOTHING", "Any suit from the OVERALL tab, with its own first-person hands."),
            Self::CompatibleHighlight => ("COMPATIBLE ITEMS HIGHLIGHT", "What fits the item under the mouse glows orange, as in S.T.A.L.K.E.R."),
            Self::SmoothAnimations => ("SMOOTH ANIMATIONS", "Hands and weapon animations drawn on smooth curves."),
            Self::GroundedFeet => ("GROUNDED FEET", "Feet drawn on the ground where Tarkov lets them float."),
            Self::LedgeClimbing => ("LEDGE CLIMBING", "Not built yet: jumping up to a ledge too high to climb and pulling yourself up. Not Tarkov's."),
            Self::Experience => ("EXPERIENCE", "Skills, weapon mastery and your level grow from what you do, with Tarkov's notices."),
            Self::SimpleXpGain => ("SIMPLE XP GAIN", "Every action earns its full experience: no falling returns. Not Tarkov's."),
            Self::Gestures => ("GESTURES", "Hand signs and voice phrases: the menu (Y twice), the quick prompt, held Y and the F-key binds."),
            Self::HideUnfinished => ("HIDE UNFINISHED ITEMS", "Items Tarkov never hands out, by SPT's own list, are left off the range and the benches."),
            Self::SeeThroughGlass => ("SEE-THROUGH GLASS", "Face shields' and window magazines' glass seen through, dithered as Tarkov draws it."),
            Self::FaceShieldCracks => ("FACE SHIELD CRACKS", "Hits on a lowered face shield mark it; its wearer sees them as cracks in the mask."),
            Self::HeadgearHearing => ("HEADGEAR HEARING", "Helmets and masks over the ears muffle what you hear, as Tarkov's do."),
        }
    }

    /// As the app comes: what is Tarkov's own, on; what departs
    /// from how Tarkov plays or looks, off (the raid limits; Tarkov's
    /// lighting); what the range adds without changing Tarkov (the
    /// benches, the clothing) and what is drawn between frames alone (the
    /// smooth animations, rule 8's one liberty), on; the unfinished items
    /// hidden, on.
    pub(super) fn recommended(self) -> bool {
        !matches!(self, Self::RaidLimits | Self::TarkovLighting | Self::CompatibleHighlight | Self::GroundedFeet | Self::LedgeClimbing)
    }

    /// What it needs on to do anything, by what the app's code goes by:
    /// the weight's slowing is the movement's; the condition and the
    /// stances act on the weapon the handling holds; the searches open
    /// containers through the world's interactions and show in the
    /// inventory, as the examining does; the limits, the clothing and the
    /// highlight are the inventory's; the dummies are made only with the
    /// bullets that hit them, and the poppers fall to them; the benches are
    /// worked in the world and show the edit build screen, whose parts are
    /// the attachments'.
    pub fn requires(self) -> &'static [Self] {
        match self {
            Self::Weight => &[Self::Movement],
            Self::WeaponCondition | Self::WeaponStances => &[Self::WeaponHandling],
            Self::Looting => &[Self::Inventory, Self::WorldInteractions],
            Self::RaidLimits | Self::Clothing | Self::CompatibleHighlight => &[Self::Inventory],
            Self::Targets => &[Self::Ballistics],
            Self::Benches => &[Self::Inventory, Self::WorldInteractions, Self::Attachments],
            Self::SimpleXpGain => &[Self::Experience],
            Self::FaceShieldCracks => &[Self::VisualEffects],
            Self::HeadgearHearing => &[Self::Sounds],
            _ => &[],
        }
    }

    /// Whether the app has what it stands for. One that is not built keeps
    /// its row on the page, drawn dim, and its key in the options file,
    /// and cannot be turned on: ledge climbing, taken out until its move
    /// reads right.
    pub fn built(self) -> bool {
        !matches!(self, Self::LedgeClimbing)
    }

    /// Taken at once, not at the next start: the
    /// highlight is looked at on every frame the inventory draws
    /// (`live_switches`).
    pub fn at_once(self) -> bool {
        matches!(self, Self::CompatibleHighlight)
    }

    /// The switches that need this one.
    pub(super) fn needed_by(self) -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().filter(move |other| other.requires().contains(&self))
    }
}

/// The FEATURES page's sections, each with its switches: EXPERIMENTAL
/// (not in Tarkov: the range's additions and the app's own drawing), then
/// CHARACTER (how the character learns; the page puts the character's name
/// under its switches). Only the switches a player has a reason to throw
/// are on a page; the rest are `BACKEND`.
pub(super) const SECTIONS: [(&str, &[Switch]); 2] = [("EXPERIMENTAL", &[Switch::Benches, Switch::Clothing, Switch::SmoothAnimations, Switch::GroundedFeet, Switch::LedgeClimbing]), ("CHARACTER", &[Switch::SimpleXpGain])];

/// CHARACTER's place among `SECTIONS`.
pub(super) const CHARACTER_SECTION: usize = 1;

/// The switch on the HUD page's SCREENS section.
pub(super) const HUD_SWITCHES: [Switch; 1] = [Switch::CompatibleHighlight];

/// The switches on no page: TRP's own systems, which a
/// player has no reason to turn off. Each still stands in the options file
/// under its key and on the command line (`--feature <key>=on|off`), and
/// RESET TO DEFAULT on the FEATURES page puts every one back as
/// recommended.
#[cfg(test)]
pub const BACKEND: [Switch; 30] = [
    Switch::Movement,
    Switch::Stamina,
    Switch::Weight,
    Switch::Health,
    Switch::Skills,
    Switch::Experience,
    Switch::Gestures,
    Switch::WeaponHandling,
    Switch::WeaponMotion,
    Switch::Ballistics,
    Switch::Attachments,
    Switch::WeaponCondition,
    Switch::WeaponStances,
    Switch::TacticalDevices,
    Switch::GrenadesAndKnife,
    Switch::Inventory,
    Switch::WorldInteractions,
    Switch::Looting,
    Switch::RaidLimits,
    Switch::HideUnfinished,
    Switch::Targets,
    Switch::CharacterBody,
    Switch::Sounds,
    Switch::HeadgearHearing,
    Switch::VisualEffects,
    Switch::FaceShieldCracks,
    Switch::TarkovLighting,
    Switch::SeeThroughGlass,
    Switch::ItemIcons,
    Switch::TarkovUi,
];

/// The FEATURES page's switches as they stand, by their places in
/// `Switch::ALL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Switches(pub(super) [bool; 37]);

impl Default for Switches {
    fn default() -> Self {
        Self(Switch::ALL.map(Switch::recommended))
    }
}

impl Switches {
    pub fn on(&self, switch: Switch) -> bool {
        self.0[switch as usize]
    }

    /// Whether it may be turned on: it is built, and everything it needs
    /// is on.
    pub fn allowed(&self, switch: Switch) -> bool {
        switch.built() && switch.requires().iter().all(|need| self.on(*need))
    }

    /// A switch thrown: on only where it is allowed; off, everything that
    /// needs it goes off with it, and what needs those. Says whether it
    /// took.
    pub fn set(&mut self, switch: Switch, on: bool) -> bool {
        if on && !self.allowed(switch) {
            return false;
        }
        self.0[switch as usize] = on;
        if !on {
            for other in switch.needed_by() {
                if self.on(other) {
                    self.set(other, false);
                }
            }
        }
        true
    }

    /// Whatever has a need off, or is not built, goes off (a file kept by
    /// hand, or by a version that had it).
    pub fn settle(&mut self) {
        while let Some(stray) = Switch::ALL.into_iter().find(|switch| self.on(*switch) && !self.allowed(*switch)) {
            self.0[stray as usize] = false;
        }
    }

    /// The fields as the switches set them. The ambient occlusion has no
    /// switch: the GRAPHICS page's SSAO has its own OFF, as the game's.
    pub fn features(&self) -> Features {
        use Switch as S;
        let on = |switch: Switch| self.on(switch);
        Features {
            custom_hands: on(S::Clothing),
            smooth_animations: on(S::SmoothAnimations),
            grounded_feet: on(S::GroundedFeet),
            visual_effects: on(S::VisualEffects),
            face_shield_mask: on(S::VisualEffects),
            face_shield_cracks: on(S::FaceShieldCracks),
            spent_cases: on(S::VisualEffects),
            magazine_rounds: on(S::VisualEffects),
            muzzle_flash: on(S::VisualEffects),
            particles: on(S::VisualEffects),
            tracers: on(S::VisualEffects),
            customs_light: on(S::TarkovLighting),
            camera_color_filter: on(S::TarkovLighting),
            camera_sharpening: on(S::VisualEffects),
            sky_scattering: on(S::TarkovLighting),
            customs_clouds: on(S::TarkovLighting),
            ambient_highlight: on(S::TarkovLighting),
            tarkov_shading: on(S::TarkovLighting),
            ambient_occlusion: true,
            hall_lamps: on(S::TarkovLighting),
            icon_generation: on(S::ItemIcons),
            compatible_highlight: on(S::CompatibleHighlight),
            character_model: on(S::CharacterBody),
            tarkov_ui: on(S::TarkovUi),
            movement: on(S::Movement),
            vaulting: on(S::Movement),
            stamina: on(S::Stamina),
            stamina_drain: on(S::Stamina),
            weight: on(S::Weight),
            exhaustion: on(S::Stamina),
            weapon_motion: on(S::WeaponMotion),
            recoil: on(S::WeaponMotion),
            weapon_handling: on(S::WeaponHandling),
            first_person_body: on(S::CharacterBody),
            weapons_on_body: on(S::CharacterBody),
            wrist_watch: on(S::CharacterBody),
            head_fitting: on(S::CharacterBody),
            armband: on(S::CharacterBody),
            overall_figure: on(S::CharacterBody),
            distance_detail: on(S::CharacterBody),
            inventory: on(S::Inventory),
            world_interactions: on(S::WorldInteractions),
            health: on(S::Health),
            skills: on(S::Skills),
            weapon_mastering: on(S::Skills),
            experience: on(S::Experience),
            simple_xp_gain: on(S::SimpleXpGain),
            gestures: on(S::Gestures),
            item_physics: on(S::WorldInteractions),
            world_distance_detail: on(S::WorldInteractions),
            sounds: on(S::Sounds),
            movement_sounds: on(S::Sounds),
            hands_sounds: on(S::Sounds),
            shot_sounds: on(S::Sounds),
            world_sounds: on(S::Sounds),
            headgear_hearing: on(S::HeadgearHearing),
            grenades_and_knife: on(S::GrenadesAndKnife),
            ballistics: on(S::Ballistics),
            dummy: on(S::Targets),
            reactive_targets: on(S::Targets),
            posing_economy: on(S::Targets),
            attachments: on(S::Attachments),
            workbench: on(S::Benches),
            armor_bench: on(S::Benches),
            raid_limits: on(S::RaidLimits),
            examining: on(S::Looting),
            searching: on(S::Looting),
            hide_unfinished: on(S::HideUnfinished),
            clothing: on(S::Clothing),
            tactical_devices: on(S::TacticalDevices),
            blind_fire: on(S::WeaponStances),
            mounting: on(S::WeaponStances),
            left_shoulder: on(S::WeaponStances),
            underbarrel_launcher: on(S::WeaponHandling),
            durability: on(S::WeaponCondition),
            malfunctions: on(S::WeaponCondition),
            overheating: on(S::WeaponCondition),
            see_through_glass: on(S::SeeThroughGlass),
        }
    }
}
