//! The command line's arguments of the character: its kit, skills, mastering and the weapon's state.

use super::*;

/// Takes an argument of the character's kit, skills and weapon state; false if it is none of them.
pub(super) fn character(options: &mut Options, arg: &str, args: &mut impl Iterator<Item = String>) -> bool {
    match arg {
        "--weapon" => options.weapon = args.next().unwrap_or_default(),
        // Another weapon for the crate, by its template or a part of
        // its name (`--add-weapon sv-98`): any the database has a
        // stock preset for. Its rig is built when it is first hung on
        // a slot, and it can be worked if its kind of logic is built.
        "--add-weapon" => options.added.extend(args.next()),
        "--preset" => options.preset = args.next(),
        "--measure-rigs" => options.measure_crate = args.next(),
        "--arms" => options.arms = args.next().unwrap_or_default(),
        "--body" => options.body = args.next().unwrap_or_default(),
        "--legs" => options.legs = args.next().unwrap_or_default(),
        "--head" => options.head = args.next().unwrap_or_default(),
        "--suit" => options.suits.extend(args.next()),
        "--wear" => options.wear.extend(args.next()),
        "--carry" => options.carry.extend(args.next()),
        "--put-on" => options.put_on.extend(args.next()),
        "--shield-hits" => options.shield_hits = args.next().and_then(|v| v.parse().ok()),
        "--lay" => options.lay.extend(args.next()),
        "--lay-ahead" => options.lay_ahead = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.8),
        "--lod-pixels" => options.lod_pixels = args.next().and_then(|v| v.parse::<f32>().ok()).filter(|v| *v > 0.0),
        "--weight" => {
            options.weight = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            options.weight_given = true;
        }
        "--loadout" => options.loadout = args.next(),
        "--inventory" => options.inventory = true,
        "--loot" => (options.inventory, options.loot) = (true, true),
        "--search-body" => (options.inventory, options.search_body) = (true, true),
        "--equip" => options.equip = true,
        "--dummy-lights" => options.dummy_lights = true,
        "--bare" => options.bare = true,
        "--no-spare-magazines" => options.no_spare_magazines = true,
        "--crash" => options.crash = true,
        "--strength" => options.strength = Some(args.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).clamp(0.0, 51.0) as u32),
        "--endurance" => options.endurance = Some(args.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).clamp(0.0, 51.0) as u32),
        "--skill-at" => {
            let given = args.next().unwrap_or_default();
            let parsed = given.split_once(':').and_then(|(frame, rest)| {
                let (name, level) = rest.split_once('=')?;
                Some((frame.parse::<u32>().ok()?, name.to_owned(), level.parse::<u32>().ok()?.min(51)))
            });
            match parsed {
                Some(at) => options.skill_at.push(at),
                None => eprintln!("--skill-at takes frame:Name=level: {given}"),
            }
        }
        "--experience" => {
            let given = args.next().unwrap_or_default();
            match given.parse::<i64>() {
                Ok(experience) => options.experience = Some(experience.max(0)),
                Err(_) => eprintln!("--experience takes the character's experience, a whole number: {given}"),
            }
        }
        "--skill" => {
            let given = args.next().unwrap_or_default();
            match given.split_once('=').and_then(|(name, level)| Some((name.to_owned(), level.parse::<u32>().ok()?.min(51)))) {
                Some(pair) => options.skill_levels.push(pair),
                None => eprintln!("--skill takes Name=level (the game's name, 0 to 51): {given}"),
            }
        }
        "--malfunction" => {
            use trp_sim::weapons::malfunction::Malf;
            options.malfunction = match args.next().as_deref() {
                Some("misfire") => Some(Malf::Misfire),
                Some("jam") => Some(Malf::Jam),
                Some("feed") => Some(Malf::Feed),
                Some("soft") => Some(Malf::SoftSlide),
                Some("hard") => Some(Malf::HardSlide),
                other => {
                    eprintln!("--malfunction takes misfire, jam, feed, soft or hard, not {other:?}");
                    None
                }
            }
        }
        "--heat" => options.heat = args.next().and_then(|v| v.parse().ok()).map(|h: f32| h.clamp(0.0, 200.0)),
        "--repair" => {
            options.repair = args.next().and_then(|v| v.parse().ok()).map(|d: f32| d.clamp(0.0, 100.0));
            options.inventory = true;
        }
        "--buffed" => {
            use trp_sim::items::repair::{BuffKind, Rarity};
            let given = args.next().unwrap_or_default();
            let (kind, rare) = given.split_once(':').map_or((given.as_str(), false), |(kind, rarity)| (kind, rarity == "rare"));
            let rarity = if rare { Rarity::Rare } else { Rarity::Common };
            options.buffed = match kind {
                "spread" => Some((BuffKind::WeaponSpread, rarity)),
                "protection" => Some((BuffKind::MalfunctionProtections, rarity)),
                "reduction" => Some((BuffKind::DamageReduction, rarity)),
                _ => {
                    eprintln!("--buffed takes spread, protection or reduction, with :rare or not, not {given:?}");
                    None
                }
            };
            options.inventory = true;
        }
        "--seed" => options.seed = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
        "--aim-speed-bonus" => {
            options.aim_speed_bonus = Some(args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(0.0, 5.0))
        }
        // The game's levels are 0 to 2 (`Mastering.GetLevelForValue`).
        "--mastering" => options.mastering = Some(args.next().and_then(|v| v.parse().ok()).unwrap_or(0u32).min(trp_sim::condition::mastering::TOP_LEVEL)),
        "--mastering-family" => {
            let given = args.next().unwrap_or_default();
            match given.split_once('=').and_then(|(name, level)| Some((name.to_owned(), level.parse::<u32>().ok()?.min(trp_sim::condition::mastering::TOP_LEVEL)))) {
                Some(pair) => options.mastering_families.push(pair),
                None => eprintln!("--mastering-family takes Name=level (the install's family, 0 to 2): {given}"),
            }
        }
        "--weapon-skill" => options.weapon_skill = Some(args.next().and_then(|v| v.parse().ok()).unwrap_or(0u32).min(51)),
        _ => return false,
    }
    true
}
