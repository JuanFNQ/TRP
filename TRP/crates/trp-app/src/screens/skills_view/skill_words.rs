//! What a skill does here, its words, levels and the experience bar.

use super::*;

/// What a skill changes in the app as it stands (`docs/spec/skills.md`,
/// "What each skill changes"): where the game's code has it act and the
/// app has the thing it acts on (`Ok`, what), else why it changes nothing
/// (`Err`). The options' SKILLS page writes these under each skill; the
/// SKILLS tab dims a skill that changes nothing.
pub fn acts_here(skill: Skill) -> Result<&'static str, &'static str> {
    use Skill as S;
    match skill {
        S::Endurance => Ok("the legs', arms' and breath's stamina, their recovery, a jump's cost"),
        S::Strength => Ok("top speeds, jump, weight limits, inertia, aiming's cost, a high throw, a knife's blow; elite: weapons weigh nothing"),
        S::Vitality => Ok("a hit's bleeding odds; elite: a bleeding stops by itself"),
        S::Health => Ok("a fracture's odds; hunger and thirst"),
        S::StressResistance => Ok("pain's strength and time"),
        S::Metabolism => Ok("what food and drink give; elite: no damage from hunger or thirst"),
        S::Immunity => Ok("what food and drink take; a painkiller's time"),
        S::Assault | S::Pistol | S::Revolver | S::Smg | S::Shotgun | S::Sniper | S::Lmg | S::Dmr => {
            Ok("its weapons' reload and draw speeds, ergonomics and recoil, from the next draw; elite: the breath held stiff as the sights come up")
        }
        // A weapon skill is read for the firearm in hand alone
        // (`GetWeaponInfo`, from `FirearmController.Item`): these four name
        // something else, or nothing (skills spec, "The weapon classes").
        S::Launcher => Err("nothing: Tarkov never reads it (it names the underbarrel launcher, a part, and a weapon skill is read for the firearm in hand alone)"),
        S::Melee => Err("nothing: Tarkov never reads it (it names the knife, and a weapon skill is read for the firearm in hand alone)"),
        S::Hmg | S::AttachedLauncher => Err("nothing: Tarkov never reads it (it names no weapon)"),
        S::Throwing => Ok("elite: a throw not weakened by tired arms"),
        S::AimDrills => Ok("how fast the sights come up, from the next draw; elite: the breath held stiff as they come up"),
        S::Surgery => Ok("a surgery's speed and the health it leaves"),
        S::CovertMovement => Ok("the steps' and the kit's noise, walking quietly"),
        S::MagDrills => Ok("loading, unloading and checking magazines; elite: rounds quicker after the first"),
        S::Perception => Ok("how wide the look finds a loose thing; elite: found from above, all round"),
        S::Intellect => Ok("a repair kit's points spent; the durability a repair takes off what no other skill governs"),
        S::Charisma => Ok("elite: half the time a repair takes nothing off the durability"),
        S::TroubleShooting => Ok("how fast a malfunction is cleared; elite: fewer malfunctions"),
        S::WeaponTreatment => Ok("a weapon's wear from its shots; the durability a repair takes off a weapon, its chance of an enhancement"),
        S::LightVests => Ok("light armour's speed penalty; the durability a repair takes off it, its chance of an enhancement"),
        S::HeavyVests => Ok("heavy armour's speed penalty; the durability a repair takes off it, its chance of an enhancement"),
        S::Attention => Ok("how fast a thing is examined and a container searched; elite: a first search over at once one time in two"),
        S::Search => Ok("how fast a container on a body is searched (a dead dummy's, or one worn); elite: two searches at once"),
        S::Crafting | S::HideoutManagement => Err("nothing here: out of a raid in Tarkov"),
        _ => Err("nothing: Tarkov locks it, with no buffs"),
    }
}

/// The skills the tab and the options' SKILLS page show: the game's list
/// but the eight it locks, which have no buffs and change nothing (as the
/// live game's page, 1.3.0.0, leaves them out; the install's, 0.16.9, shows
/// them dimmed, `SkillContainer`).
pub fn shown() -> impl Iterator<Item = Skill> {
    Skill::ALL.into_iter().filter(|skill| !skill.locked())
}

/// Why a skill is dimmed on the tab though the game does not lock it: the
/// options' SKILLS off, or nothing it changes built here; none where it
/// acts.
pub(super) fn dimmed_why(skill: Skill, on: bool) -> Option<String> {
    if !on {
        return Some("In this app it changes nothing: SKILLS is off under the options' FEATURES.".to_owned());
    }
    acts_here(skill).err().map(|why| format!("In this app it changes {why}."))
}

/// A skill's class as the game's page has it (`SkillClassIcon`): its
/// colour and its mark.
pub(crate) fn class_look(class: Class) -> (Color, &'static str) {
    let (rgb, mark) = match class {
        Class::Physical => ([103, 177, 184], "Ph"),
        Class::Combat => ([184, 87, 57], "C"),
        Class::Mental => ([220, 215, 185], "M"),
        Class::Practical => ([175, 220, 169], "P"),
    };
    (Color::srgb_u8(rgb[0], rgb[1], rgb[2]), mark)
}

/// A skill's icon's border at elite (`SkillIcon.EliteBorderColor`), white
/// below.
pub(super) const ELITE_BORDER: Color = Color::srgb(183.0 / 255.0, 112.0 / 255.0, 0.0);

/// The points a level of a skill takes as its page counts them
/// (`Skill.CalculateLevelExp`).
pub(super) fn level_exp(level: u32) -> u32 {
    if level >= 9 { 100 } else { (level + 1) * 10 }
}

/// A skill's level and progress as its page says them (`SkillPanel.
/// OnSkillLevelChanged`, `SkillIcon`, `SkillTooltip`): "Level: 23" or the
/// elite's words; the points into the level of what it takes, or "maxed";
/// the fill's share (1 at elite) and the base fill's (0 at elite; the same
/// as the fill, nothing being earned: `BaseProgress`).
pub(super) struct LevelWords {
    pub(super) level: u32,
    pub(super) elite: bool,
    pub(super) said: String,
    pub(super) progress: String,
    pub(super) fill: f32,
    pub(super) base: f32,
}

pub(super) fn level_words(said: &Words, levels: &SkillLevels, skill: Skill) -> LevelWords {
    let level = levels.level(skill);
    let elite = level > 50;
    let share = (levels.progress(skill) / trp_sim::condition::skills::POINTS_PER_LEVEL - level as f32).clamp(0.0, 1.0);
    let exp = level_exp(level);
    let points = ((share * exp as f32) as f64 * 100.0).round() / 100.0;
    let progress = if elite { format!("<color=#c5c3b2>{}</color>", said.word("maxed", "maxed")) } else { format!("<color=#c5c3b2><b>{}</b></color>/{exp}", crate::screens::health_view::trimmed_figure(points as f32)) };
    let words = if elite { said.word(ELITE_LEVEL, ELITE_LEVEL) } else { format!("{}{level}", said.word("Level ", "Level: ")) };
    LevelWords { level, elite, said: words, progress, fill: if elite { 1.0 } else { share }, base: if elite { 0.0 } else { share } }
}

/// A skill's name in the install's words (`ESkillId`'s name localized).
pub(super) fn skill_name(said: &Words, skill: Skill) -> String {
    said.word(skill.name(), skill.name())
}

/// A buff's words (`BuffIcon.GetBuffDescription`): its name's words with
/// its brackets blue, its value in its hole as .NET writes it.
pub(super) fn buff_words(said: &Words, buff: &BuffDef, level: u32) -> String {
    let text = said.word(buff.id, buff.id).replace('[', "<color=#54c1ff>").replace(']', "</color>");
    match text.find("{0") {
        Some(_) => fill_hole(&text, buff.value(level)),
        None => text,
    }
}

/// A text's `{0...}` filled with a value as `string.Format` writes it
/// (`Single`'s formats: `0.#%`, `0.##%`, `0.#`, `F1`, `N2`, none).
pub(super) fn fill_hole(text: &str, value: BuffValue) -> String {
    let Some(open) = text.find("{0") else { return text.to_owned() };
    let Some(close) = text[open..].find('}').map(|at| open + at) else { return text.to_owned() };
    let format = text[open + 2..close].strip_prefix(':').unwrap_or("");
    let written = match value {
        BuffValue::Flag(on) => if on { "True" } else { "False" }.to_owned(),
        BuffValue::Figure(figure) => dotnet_figure(figure, format),
    };
    format!("{}{written}{}", &text[..open], &text[close + 1..])
}

/// A `Single` as .NET writes it in a format: its seven figures first, then
/// rounded half away from zero to the format's places (`#` dropped where
/// nought, `0` and `F`/`N` kept), a percent a hundred times it.
pub(super) fn dotnet_figure(value: f32, format: &str) -> String {
    let seven: f64 = format!("{value:.6e}").parse().unwrap_or(value as f64);
    let percent = format.ends_with('%');
    let body = format.trim_end_matches('%');
    let value = if percent { seven * 100.0 } else { seven };
    let rounded = |places: usize| {
        let k = 10f64.powi(places as i32);
        ((value.abs() * k + 0.5 + 1e-9).floor() / k).copysign(value)
    };
    let (places, kept) = match body {
        "" => return crate::screens::health_view::trimmed_figure(value as f32),
        "F1" => (1, true),
        "N2" => (2, true),
        b => match b.split_once('.') {
            Some((_, decimals)) => (decimals.len(), decimals.chars().all(|c| c == '0')),
            None => (0, true),
        },
    };
    let mut written = format!("{:.*}", places, rounded(places));
    if !kept && written.contains('.') {
        written = written.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    if body == "N2" {
        let (whole, decimals) = written.split_once('.').unwrap_or((&written, ""));
        let (sign, digits) = whole.strip_prefix('-').map_or(("", whole), |d| ("-", d));
        let grouped: Vec<String> = digits.as_bytes().rchunks(3).rev().map(|chunk| String::from_utf8_lossy(chunk).into_owned()).collect();
        written = format!("{sign}{}.{decimals}", grouped.join(","));
    }
    if percent {
        written.push('%');
    }
    written
}

/// A whole number as the game writes it with its thousands apart
/// (`ToThousandsString`: a space between).
pub(crate) fn thousands(value: u32) -> String {
    let digits = value.to_string();
    let grouped: Vec<String> = digits.as_bytes().rchunks(3).rev().map(|chunk| String::from_utf8_lossy(chunk).into_owned()).collect();
    grouped.join(" ")
}

/// The skills listed, filtered and sorted as the page's dropdowns have
/// them (`SkillContainer`): all shown, or one class's (the filter's line
/// less one, `ESkillClass`); in the game's order, by level from the most (a
/// stable sort), or by when each last earned (none earns here: the game's
/// order, `PROVISIONAL`).
#[cfg(test)]
pub(super) fn listed(levels: &SkillLevels, filter: usize, sort: usize) -> Vec<Skill> {
    listed_by(levels, filter, sort, &[])
}

/// The same, "Most recent" by when each last earned (`LastCall`, the
/// latest first; those that never earned after, in the game's order),
/// `recent` by the skills' places.
pub(super) fn listed_by(levels: &SkillLevels, filter: usize, sort: usize, recent: &[Option<f64>]) -> Vec<Skill> {
    let class = filter.checked_sub(1).map(|n| CLASSES[n]);
    let mut skills: Vec<Skill> = shown().filter(|skill| class.is_none_or(|class| skill.class().name() == class)).collect();
    if sort == 1 {
        skills.sort_by_key(|skill| std::cmp::Reverse(levels.level(*skill)));
    }
    if sort == 2 {
        let last = |skill: &Skill| recent.get(skill.index()).copied().flatten().unwrap_or(f64::NEG_INFINITY);
        skills.sort_by(|a, b| last(b).total_cmp(&last(a)));
    }
    skills
}

/// The character's experience as the page's panel shows it
/// (`PlayerExperiencePanel.Set(exp, exp)`): its level and the next, what
/// it has, where its level begins and the next does; the bar's share.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct XpShown {
    pub level: u32,
    pub total: i64,
    pub start: i64,
    pub next: i64,
}

impl XpShown {
    pub(super) fn of(xp: Option<&crate::player::experience::Experience>, look: &HealthLook) -> Self {
        match xp {
            Some(xp) => {
                let level = xp.level();
                Self { level, total: xp.total, start: xp.table.experience_at(level), next: xp.table.experience_at(level + 1) }
            }
            None => Self { level: 1, total: 0, start: 0, next: look.level_experience.iter().take(2).sum::<u32>() as i64 },
        }
    }

    /// The bar's share of the level (`TwoValueBar.SetBase`): full at the
    /// top level, where the next is where it is.
    pub(super) fn share(&self) -> f32 {
        if self.next > self.start { ((self.total - self.start) as f32 / (self.next - self.start) as f32).clamp(0.0, 1.0) } else { 1.0 }
    }

    /// The rank's picture's level (`PlayerLevelPanel.Set`): the next five
    /// up, at most 80.
    fn rank(level: u32) -> u32 {
        if level > 80 { 80 } else { (level / 5 + 1) * 5 }
    }
}

/// The experience's bar set as `TwoValueBar` sets it (`SetBase(share)`,
/// `SetNew(share)`: no new fill, the glow at the share).
pub(super) fn set_bar(panel: &mut ScenePanel, share: f32) {
    if let Some(base) = panel.find(&format!("{EXPERIENCE}/Bar/Base Fill")) {
        panel.nodes[base].layout.anchor_max[0] = share;
    }
    if let Some(new) = panel.find(&format!("{EXPERIENCE}/Bar/New Fill")) {
        let layout = &mut panel.nodes[new].layout;
        (layout.anchor_min[0], layout.anchor_max[0], layout.offset[0], layout.size_delta[0]) = (share, share, 0.0, 0.0);
    }
    if let Some(glow) = panel.find(&format!("{EXPERIENCE}/Bar/New Glow")) {
        let layout = &mut panel.nodes[glow].layout;
        (layout.anchor_min[0], layout.anchor_max[0]) = (share, share);
    }
}

/// The two level panels' rank pictures, by the levels they show.
pub(super) fn rank_sprites(panel: &ScenePanel, skills: &SkillsLook, xp: &XpShown, sprites: &mut HashMap<usize, Picture>) {
    for (end, level) in [("Current Level Panel/Level Icon", xp.level), ("Next Level Panel/Level Icon", xp.level + 1)] {
        let want = XpShown::rank(level);
        let picture = skills.ranks.iter().find(|(rank, _)| *rank == want).or(skills.ranks.last()).map(|(_, picture)| *picture);
        if let (Some(at), Some(picture)) = (panel.find(&format!("{EXPERIENCE}/{end}")), picture) {
            sprites.insert(at, picture);
        }
    }
}
