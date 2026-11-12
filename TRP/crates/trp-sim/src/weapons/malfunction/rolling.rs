//! Heat, the chances and rolling a malfunction on each shot.

use super::*;

/// The skills that touch it, as the character has them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Skills {
    /// Weapon Maintenance's level (`WeaponTreatment`): each lowers a shot's wear.
    pub maintenance: f32,
    /// Troubleshooting's level: each quickens a repair; elite lowers the
    /// chances once a malfunction of a source has been had with the weapon.
    pub troubleshooting: f32,
    pub troubleshooting_elite: bool,
}

/// What the weapon keeps of it, with its item (`Weapon.MalfunctionState`'s
/// saved part, `MalfunctionDescriptor`): the malfunction, its source,
/// whether the shooter knows of it and of its kind, the last shot's heat and
/// the raid's clock then, and the slide the heat holds; and what is drawn
/// once and not saved (the fire rate's factor, the stray shot's time).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub malf: Malf,
    pub source: Source,
    pub known: bool,
    pub type_known: bool,
    pub last_shot_heat: f32,
    pub last_shot_time: f64,
    pub slide_reached: bool,
    /// The sources this shooter has had a malfunction from while elite at
    /// Troubleshooting (`AddMalfReduceChance`): any one lowers the
    /// durability's chance too (the game's flag 0 is every flag).
    pub reduce_flags: u8,
    pub firerate_mult: f32,
    pub autoshot_drawn: bool,
    pub autoshot_at: Option<f64>,
}

/// The four chances of a shot (`GetTotalMalfunctionChance`): the total, the
/// durability's (a probability), the round's, the magazine's and the heat's
/// (per mille), and the durability's floor (`weaponDurability`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Chances {
    pub total: f32,
    pub durability: f32,
    pub ammo: f32,
    pub magazine: f32,
    pub heat: f32,
    pub floor: f32,
}

/// The heat now, from the last shot's and the time since it
/// (`GetCurrentOverheat`): falling at a rate the last shot's heat sets.
pub fn current_heat(state: &State, now: f64, w: &WeaponFigures, s: &Settings) -> f32 {
    let h0 = state.last_shot_heat;
    if h0 <= 0.0 || !w.allow_overheat {
        return 0.0;
    }
    let dt = (now - state.last_shot_time).max(0.0) as f32;
    let k = w.cool * s.mod_cool_factor;
    let c = s.cool_coef;
    let d = h0 - (h0 / s.max_overheat + 0.0338 * c + 0.2925).powf(9.667 + 0.0668 * c);
    (h0 - dt * h0 * k / d).clamp(s.min_overheat, s.max_overheat)
}

/// The chance of the shot about to fire (`GetTotalMalfunctionChance`): by
/// the weapon's durability (relative to 100), its round, its magazine (if
/// one is in) and the heat at the trigger; a repair buff's malfunction
/// protection (`protection`, 1 without one: `Weapon.Buff.MalfunctionProtections`)
/// times the heat's chance right after its lerp and the durability's before
/// elite Troubleshooting's cut.
#[allow(clippy::too_many_arguments)]
pub fn chances(w: &WeaponFigures, s: &Settings, durability: f32, round: Option<&RoundFigures>, magazine: Option<f32>, heat: f32, skills: &Skills, reduce_flags: u8, protection: f32) -> Chances {
    if !w.malfunctions() {
        return Chances::default();
    }
    let mut mag = magazine.map_or(0.0, |chance| chance * s.magazine_malf_chance_mult);
    let mut ammo = round.map_or(0.0, |r| (r.misfire + r.feed) * s.ammo_malf_chance_mult);
    let num = durability;
    let floor = num.floor();
    let heat_chance = (if heat >= s.problems_start { lerp(s.min_malf_chance, s.max_malf_chance, (heat - s.problems_start) / (s.max_overheat - s.problems_start)) } else { 0.0 }) * protection;
    let (b, fr, wd) = (w.base_chance as f64, w.fire_rate as f64, floor as f64);
    let d = if floor > 59.0 {
        ((b + 1.0).powf(3.0 + (100.0 - wd) / (20.0 - 10.0 / (fr / 10.0).powf(0.322))) - 1.0) / 1000.0
    } else {
        ((b + 1.0).powf(((101.0 - wd).powf((50.0 - wd.powf(1.286) / 4.8) / (fr.powf(0.17) / 2.9815 + 2.1))).log10()) - 1.0) / 1000.0
    };
    // (The game's double times the float of the protection.)
    let mut d = (d * protection as f64) as f32;
    if skills.troubleshooting_elite && reduce_flags != 0 {
        d *= s.elite_chance_reduce[0];
    }
    if skills.troubleshooting_elite && reduce_flags & 1 != 0 {
        ammo *= s.elite_chance_reduce[1];
    }
    if skills.troubleshooting_elite && reduce_flags & 2 != 0 {
        mag *= s.elite_chance_reduce[2];
    }
    if s.dur_ignore[0] <= num && num <= s.dur_ignore[1] {
        (d, ammo, mag) = (0.0, 0.0, 0.0);
    }
    let d = d.clamp(0.0, 1.0);
    let total = ((d as f64 + ((ammo + mag + heat_chance) / 1000.0) as f64) * 100_000.0).round() / 100_000.0;
    Chances { total: (total as f32).clamp(0.0, 1.0), durability: d, ammo, magazine: mag, heat: heat_chance, floor }
}

/// The roll's own generator (`MalfunctionRandom`): each roll puts Unity's
/// generator to the seed plus the rolls so far and draws from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MalfRandom {
    pub seed: i32,
    pub count: i32,
}

impl MalfRandom {
    pub fn new(seed: i32) -> Self {
        Self { seed, count: 0 }
    }

    /// `GetRandomFloat`: the draw, and the generator left after it.
    fn float(&mut self) -> (f32, UnityRandom) {
        let mut random = UnityRandom::new(self.seed.wrapping_add(self.count));
        self.count = self.count.wrapping_add(1);
        let u = random.value();
        (u, random)
    }
}

/// What the shot is to come to (`GetMalfunctionState`), the weapon's figures
/// and its state as they stand before it, the heat at the trigger: the
/// heat's slide first, with no draw; else one draw decides whether and from
/// which source, a second which malfunction, each by its weights where the
/// weapon allows it and the magazine has what it needs.
#[allow(clippy::too_many_arguments)]
pub fn roll(random: &mut MalfRandom, w: &WeaponFigures, s: &Settings, chances: &Chances, state: &State, heat: f32, rounds_in_magazine: bool, magazine_in: bool) -> (Malf, Source) {
    if !w.malfunctions() {
        return (Malf::None, Source::Durability);
    }
    if state.slide_reached && heat > s.fix_slide && w.allow_slide && rounds_in_magazine {
        return (Malf::SoftSlide, Source::Overheat);
    }
    let (u, mut generator) = random.float();
    if u > chances.total {
        return (Malf::None, Source::Durability);
    }
    let mut sources = vec![(Source::Durability, chances.durability)];
    if chances.ammo > 0.0 {
        sources.push((Source::Ammo, chances.ammo / 1000.0));
    }
    if chances.magazine > 0.0 && rounds_in_magazine && magazine_in {
        sources.push((Source::Magazine, chances.magazine / 1000.0));
    }
    if chances.heat > 0.0 {
        sources.push((Source::Overheat, chances.heat / 1000.0));
    }
    let source = pick(&sources, u);
    let check_jam = rounds_in_magazine || !w.bolt_catch || !magazine_in;
    let feeds = rounds_in_magazine && magazine_in && w.allow_feed;
    let slides = rounds_in_magazine && w.allow_slide;
    let hard = chances.floor <= 5.0;
    let share = 1.0 - chances.floor / 5.0;
    let mut variants: Vec<(Malf, f32)> = Vec::new();
    let mut add = |malf: Malf, weight: f32, allowed: bool| {
        if allowed {
            variants.push((malf, weight));
        }
    };
    match source {
        Source::Durability => {
            add(Malf::Feed, s.dur_feed_wt, feeds);
            add(Malf::Misfire, s.dur_misfire_wt, w.allow_misfire);
            add(Malf::Jam, s.dur_jam_wt, check_jam && w.allow_jam);
            add(Malf::SoftSlide, s.dur_soft_slide_wt, slides);
            add(Malf::HardSlide, lerp(s.dur_hard_slide[0], s.dur_hard_slide[1], share), slides && hard);
        }
        Source::Ammo => {
            let both = (chances.ammo / s.ammo_malf_chance_mult.max(1e-6)).max(1e-6);
            add(Malf::Misfire, s.ammo_misfire_wt / both, w.allow_misfire);
            add(Malf::Feed, s.ammo_feed_wt, feeds);
            add(Malf::Jam, s.ammo_jam_wt, check_jam && w.allow_jam);
        }
        Source::Magazine => add(Malf::Feed, 1.0, feeds),
        Source::Overheat => {
            add(Malf::Feed, s.overheat_feed_wt, feeds);
            add(Malf::Jam, s.overheat_jam_wt, check_jam && w.allow_jam);
            add(Malf::SoftSlide, s.overheat_soft_slide_wt, slides);
            add(Malf::HardSlide, lerp(s.overheat_hard_slide[0], s.overheat_hard_slide[1], share), slides && hard);
        }
    }
    if variants.is_empty() {
        return (Malf::None, source);
    }
    let sum: f32 = variants.iter().map(|(_, weight)| weight).sum();
    let v = generator.range(0.0, sum);
    (pick(&variants, v), source)
}

/// The first whose running sum passes the value, else the last
/// (`DropManager.GenerateDrop`).
fn pick<T: Copy>(items: &[(T, f32)], value: f32) -> T {
    let mut sum = 0.0;
    for (item, weight) in items {
        sum += weight;
        if value < sum {
            return *item;
        }
    }
    items.last().expect("a list to pick from").0
}

/// What one shot that fires leaves (`Weapon.OnShot`): the heat added and
/// its effects drawn, the wear taken.
#[allow(clippy::too_many_arguments)]
pub fn on_shot(state: &mut State, now: f64, w: &WeaponFigures, s: &Settings, round: &RoundFigures, skills: &Skills, durability: &mut f32, max_durability: &mut f32, random: &mut UnityRandom) {
    // The heat.
    if w.allow_overheat {
        let current = current_heat(state, now, w, s);
        let shot = w.heat_by_shot * (round.heat * w.parts_heat) * w.heat_gun * s.mod_heat_factor;
        state.last_shot_heat = (current + shot).clamp(s.min_overheat, s.max_overheat);
        state.last_shot_time = now;
    } else {
        state.last_shot_heat = 0.0;
    }
    let heat = state.last_shot_heat;
    let t = heat.clamp(s.problems_start, s.max_overheat) / 100.0 - 1.0;
    // The wear.
    if *durability > 0.0 {
        let loss = 100.0 / w.operating_resource * w.burn_ratio * (w.parts_burn * round.burn) * lerp(s.dur_reduce[0], s.dur_reduce[1], t) * (1.0 - skills.maintenance * s.dur_loss_reduce_per_level);
        *durability = (*durability - loss).max(0.0);
    }
    // The greatest durability, worn by heat.
    if heat >= s.problems_start && *max_durability / 100.0 >= s.wear_limit {
        let wear = if (heat - s.max_overheat).abs() < f32::EPSILON { random.range(s.wear_on_max_overheat[0], s.wear_on_max_overheat[1]) } else { lerp(s.wear_on_overheat[0], s.wear_on_overheat[1], t) };
        if *durability < *max_durability - wear {
            *max_durability = (*max_durability - wear).max(s.wear_limit * 100.0);
        }
    }
    // The slide the heat holds.
    if s.slide_on_max {
        if heat >= s.start_slide {
            state.slide_reached = true;
        } else if heat <= s.fix_slide {
            state.slide_reached = false;
        }
    }
    // The fire rate's factor.
    if heat > s.firerate_border {
        if state.firerate_mult <= 0.0 {
            state.firerate_mult = random.range(s.firerate_reduce[0], s.firerate_reduce[1]);
        }
    } else {
        state.firerate_mult = 0.0;
    }
    // The stray shot.
    if heat >= s.autoshot_min {
        if !state.autoshot_drawn {
            state.autoshot_drawn = true;
            let delay = random.range(0.05, s.autoshot_duration);
            state.autoshot_at = (random.range(0.0, 1.0) <= s.autoshot_chance).then_some(now + delay as f64);
        }
    } else {
        state.autoshot_drawn = false;
        state.autoshot_at = None;
    }
}

/// The spread's factor from the heat (`FirearmController.Shot`): 1 to
/// `MaxCOIIncreaseMult` from the problems' start to the most.
pub fn spread_factor(heat: f32, s: &Settings) -> f32 {
    if heat < s.problems_start {
        return 1.0;
    }
    lerp(1.0, s.max_coi_increase, (heat - s.problems_start) / (s.max_overheat - s.problems_start))
}

/// A repair's speed (`SpeedFix`): Troubleshooting's, the arms', a hard
/// slide's.
pub fn repair_speed(s: &Settings, skills: &Skills, arms_broken: u32, malf: Malf) -> f32 {
    let hands = match arms_broken {
        0 => 1.0,
        1 => s.repair_one_hand_broken,
        _ => s.repair_two_hands_broken,
    };
    (1.0 + skills.troubleshooting * s.repair_speed_per_level) * hands * if malf == Malf::HardSlide { s.repair_hard_slide } else { 1.0 }
}
