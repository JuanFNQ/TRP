//! The sprite names and icon tables of attributes, figures, effects and wishes.

use super::*;

/// A TextMeshPro sprite asset's sprites by their names (`TMP_SpriteAsset`,
/// as a text's `<sprite name="…">` finds them), each the engine's own
/// sprite its glyph points to (file and number), from a resource's path
/// (`Sprite Assets/armor_classes`). The asset's fields by TextMeshPro's
/// published order: its character table (each its kind, its code, its
/// glyph's number, its scale, its name, its name's hash), then its glyph
/// table (each its number, its metrics, its rectangle, its scale, its
/// atlas, its kind, and a pointer to its sprite: 64 bytes); found as the
/// first run of characters of the sprite kind that the glyph table follows.
pub fn tmp_sprite_names(install: &Install, path: &str) -> Result<Vec<(String, (String, i64))>> {
    let wanted = path.to_lowercase();
    let mut asset = None;
    for (at, file, id) in resource_paths(install)? {
        if at == wanted {
            let (class, bytes) = object_bytes(install, &file, id)?;
            if class == crate::unity::class::MONO_BEHAVIOUR {
                asset = Some((file, bytes));
            }
        }
    }
    let (file, bytes) = asset.with_context(|| format!("no sprite asset at {path}"))?;
    let word = |at: usize| bytes.get(at..at + 4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]));
    // The character table: a count, then each character.
    let characters = |start: usize| -> Option<(Vec<(String, u32)>, usize)> {
        let count = word(start)? as usize;
        if !(1..=4096).contains(&count) {
            return None;
        }
        let mut at = start + 4;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if word(at)? != 2 {
                return None;
            }
            let glyph = word(at + 8)?;
            let len = word(at + 16)? as usize;
            let name = std::str::from_utf8(bytes.get(at + 20..at + 20 + len)?).ok()?;
            if len == 0 || !name.chars().all(|c| c.is_ascii_graphic()) {
                return None;
            }
            out.push((name.to_owned(), glyph));
            at += 20 + ((len + 3) & !3) + 4;
        }
        Some((out, at))
    };
    let (names, glyphs_at) = (0..bytes.len()).step_by(4).find_map(characters).context("the sprite asset has no characters")?;
    let count = word(glyphs_at).context("no glyph table")? as usize;
    let mut sprites = HashMap::new();
    for g in 0..count {
        let at = glyphs_at + 4 + g * 64;
        let (Some(index), Some(file_index)) = (word(at), word(at + 52)) else { break };
        let path_id = bytes.get(at + 56..at + 64).map(|b| i64::from_le_bytes(b.try_into().unwrap_or_default())).unwrap_or(0);
        if file_index == 0 && path_id != 0 {
            sprites.insert(index, path_id);
        }
    }
    Ok(names.into_iter().filter_map(|(name, glyph)| Some((name, (file.clone(), *sprites.get(&glyph)?)))).collect())
}

/// How large the window that shows a thing may be stretched
/// (`StretchWindow._maxSize`), where it has the script. The script keeps
/// Odin's serialized data first (its dictionary of areas), so the size is
/// found as the two figures just before its pointer to what it stretches
/// (`_stretchableObject`, the window's own `LayoutElement`, on its top).
pub fn stretch_limit(window: &ScenePanel) -> Option<[f32; 2]> {
    let fields = (0..window.nodes.len()).find_map(|index| window.script(index, "StretchWindow"))?;
    let figure = |at: usize| fields.get(at..at + 4).map(|word| f32::from_le_bytes([word[0], word[1], word[2], word[3]]));
    (0..fields.len().saturating_sub(20)).step_by(4).find_map(|at| {
        let size = [figure(at)?, figure(at + 4)?];
        (window.pointer_at(fields, at + 8) == Some(0) && size.iter().all(|side| side.is_finite() && (100.0..10000.0).contains(side))).then_some(size)
    })
}

/// Every sprite the build keeps as a resource under a path's beginning
/// (`Resources/Slots/`: lower case, as the resource manager keeps them),
/// each with what of its path follows the beginning and its file and
/// number; read in one pass over the resources' list.
pub fn resource_sprites(install: &Install, under: &str) -> Result<Vec<(String, (String, i64))>> {
    let wanted = under.to_lowercase();
    let mut found = Vec::new();
    let mut builds: HashMap<String, Option<BuildFile>> = HashMap::new();
    for (path, file, id) in resource_paths(install)? {
        let Some(rest) = path.strip_prefix(&wanted) else { continue };
        let build = builds.entry(file.clone()).or_insert_with(|| BuildFile::open(install, &file).ok());
        if build.as_ref().is_some_and(|opened| opened.meta.object(id).is_some_and(|object| object.class_id == SPRITE)) {
            found.push((rest.to_owned(), (file, id)));
        }
    }
    Ok(found)
}

/// A row's colours and pictures as its script keeps them (inspect-window
/// spec, "A row"): a compact row's increasing and decreasing colours
/// (`_increasingColor`, `_decreasingColor`, after its value's text), and a
/// bar row's own (`CurrentColor`, `LowColor`, `SubColor`, `AddColor`, after
/// its comparison's bar) with its adding and subtracting pictures
/// (`AddSprite`, `SubSprite`), each a file and a number.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RowLook {
    pub increasing: [f32; 4],
    pub decreasing: [f32; 4],
    pub bar: Option<BarLook>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BarLook {
    pub current: [f32; 4],
    pub low: [f32; 4],
    pub sub: [f32; 4],
    pub add: [f32; 4],
    pub add_sprite: Option<(String, i64)>,
    pub sub_sprite: Option<(String, i64)>,
}

/// A row's look from its script (`class`) on its top, found by the
/// script's pointers to its value's text and its comparison's bar, which
/// the colours follow.
pub fn row_look(panel: &ScenePanel, class: &str) -> Option<RowLook> {
    let fields = panel.script(0, class)?;
    let at = |path: &str| -> Option<usize> {
        let node = panel.find(path)?;
        (0..fields.len().saturating_sub(11)).step_by(4).find(|&start| panel.pointer_at(fields, start) == Some(node))
    };
    let colour = |start: usize| -> Option<[f32; 4]> {
        let mut r = Reader::new(fields.get(start..start + 16)?);
        Some([r.f32().ok()?, r.f32().ok()?, r.f32().ok()?, r.f32().ok()?])
    };
    let value = at("Value")?;
    let mut look = RowLook { increasing: colour(value + 12)?, decreasing: colour(value + 28)?, bar: None };
    if let Some(compare) = at("bars/CompareDelta") {
        let sprite = |start: usize| -> Option<(String, i64)> {
            let to = pointer(&mut Reader::new(fields.get(start..)?)).ok()?;
            Some((panel.file_of(to.file)?, to.path)).filter(|(_, id)| *id != 0)
        };
        look.bar = Some(BarLook {
            current: colour(compare + 12)?,
            low: colour(compare + 28)?,
            sub: colour(compare + 44)?,
            add: colour(compare + 60)?,
            add_sprite: sprite(compare + 76),
            sub_sprite: sprite(compare + 88),
        });
    }
    Some(look)
}

/// The attributes' icons (`StaticIcons.ItemAttributeSprites`): each
/// attribute's name (its `EItemAttributeId`) and its sprite's file and
/// number. The Resource `Static Icons` keeps it as an enum dictionary (a
/// list of key names, then a list of as many pointers): the one whose keys
/// have the weapon's ergonomics and accuracy.
pub fn attribute_icons(install: &Install) -> Result<Vec<(String, (String, i64))>> {
    icon_table(install, &["Ergonomics", "CenterOfImpact"], 8).context("the static icons have no attributes' icons")
}

/// The other three tables of icons of a thing's figures that the same
/// Resource keeps (`StaticIcons.GetAttributeIcon` picks a table by the
/// enum the attribute's id is of): a health effect's
/// (`HealEffectSprites`, by `EHealthFactorType`), a damage effect's
/// (`DamageEffectSprites`, `EDamageEffectType`) and a stimulant's buff's
/// (`StimulatorBuffSprites`, `EStimulatorBuffType`); each found by names
/// only it has among the four. Names may be in more than one (`Pain`,
/// `Fracture`, the bleedings): the table goes by the enum.
pub fn figure_icons(install: &Install) -> Result<[Vec<(String, (String, i64))>; 3]> {
    Ok([
        icon_table(install, &["Hydration", "Energy"], 3).context("the static icons have no health effects' icons")?,
        icon_table(install, &["RadExposure", "HeavyBleeding"], 3).context("the static icons have no damage effects' icons")?,
        icon_table(install, &["HealthRate", "SkillRate"], 3).context("the static icons have no buffs' icons")?,
    ])
}

/// The first enum dictionary of the Resource `Static Icons` whose keys
/// hold every name of `must`, with at least `least` keys.
fn icon_table(install: &Install, must: &[&str], least: usize) -> Result<Vec<(String, (String, i64))>> {
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "static icons").context("no resource is at static icons")?;
    let (_, bytes) = object_bytes(install, file, *id)?;
    let table_at = |start: usize| -> Option<Vec<(String, (String, i64))>> {
        let mut r = Reader::new(bytes.get(start..)?);
        let count = r.u32().ok()? as usize;
        if !(least..=400).contains(&count) {
            return None;
        }
        let mut keys = Vec::with_capacity(count);
        for _ in 0..count {
            let len = r.u32().ok()? as usize;
            if !(1..=48).contains(&len) {
                return None;
            }
            let key = std::str::from_utf8(r.bytes(len).ok()?).ok()?;
            if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            keys.push(key.to_owned());
            r.align(4);
        }
        if !must.iter().all(|name| keys.iter().any(|key| key == name)) || r.u32().ok()? as usize != count {
            return None;
        }
        let mut icons = Vec::new();
        for key in keys {
            let to = pointer(&mut r).ok()?;
            if to.file == 0 && to.path != 0 {
                icons.push((key, (file.clone(), to.path)));
            }
        }
        Some(icons)
    };
    (0..bytes.len()).step_by(4).find_map(table_at).context("no such table")
}

/// The health's effects the game has an icon for, in the order of the
/// hard settings' table of them (`StaticIcons.EffectSprites`' fields).
pub const EFFECT_ICONS: [&str; 28] = [
    "Painkiller",
    "Tremor",
    "Fracture",
    "Exhaustion",
    "TunnelVision",
    "Dehydration",
    "Contusion",
    "Wound",
    "LightBleeding",
    "HeavyBleeding",
    "Pain",
    "Berserk",
    "Stun",
    "StimulatorBuff",
    "StimulatorDebuff",
    "ChronicStaminaFatigue",
    "Encumbered",
    "OverEncumbered",
    "Intoxication",
    "LethalIntoxication",
    "ImmunityPreventedContamination",
    "HalloweenBuff",
    "MisfireEffect",
    "PanicEffect",
    "MildMusclePain",
    "SevereMusclePain",
    "ZombieInfection",
    "Frostbite",
];

/// Each effect's icon (`EFFECT_ICONS`) and its sprite's file and number:
/// the table is the last field of the Resource `Static Icons`
/// (`StaticIcons.EffectIcons`, an `EffectSprites` of one sprite's pointer a
/// field), read from its end.
pub fn effect_icons(install: &Install) -> Result<Vec<(&'static str, (String, i64))>> {
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "static icons").context("no resource is at static icons")?;
    let (_, bytes) = object_bytes(install, file, *id)?;
    let start = bytes.len().checked_sub(EFFECT_ICONS.len() * 12).context("the static icons are too short")?;
    let mut r = Reader::new(&bytes[start..]);
    let mut icons = Vec::new();
    for name in EFFECT_ICONS {
        let to = pointer(&mut r)?;
        if to.file == 0 && to.path != 0 {
            icons.push((name, (file.clone(), to.path)));
        }
    }
    Ok(icons)
}

/// The wish list's groups, by the game's names (`EWishlistGroup`).
pub const WISH_GROUPS: [&str; 5] = ["Quests", "Hideout", "Trading", "Equipment", "Other"];

/// The wish list's groups' icons: those of its menu's lines and of the
/// window that shows a thing (`StaticIcons.WishlistSprites`), then the
/// outlined ones of a thing's view (`WishlistSpritesOutline`); each a
/// group's name and its sprite's file and number. The Resource `Static
/// Icons` keeps its tables as the game's enum dictionaries (a list of key
/// names, then a list of as many pointers); these two are the first two
/// whose keys are the five groups.
pub fn wishlist_icons(install: &Install) -> Result<[Vec<(&'static str, (String, i64))>; 2]> {
    let paths = resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "static icons").context("no resource is at static icons")?;
    let (_, bytes) = object_bytes(install, file, *id)?;
    let table_at = |start: usize| -> Option<Vec<(&'static str, (String, i64))>> {
        let mut r = Reader::new(bytes.get(start..)?);
        if r.u32().ok()? != WISH_GROUPS.len() as u32 {
            return None;
        }
        let mut keys = Vec::new();
        for _ in 0..WISH_GROUPS.len() {
            let len = r.u32().ok()? as usize;
            if len > 16 {
                return None;
            }
            let key = std::str::from_utf8(r.bytes(len).ok()?).ok()?;
            keys.push(*WISH_GROUPS.iter().find(|group| **group == key)?);
            r.align(4);
        }
        if r.u32().ok()? != WISH_GROUPS.len() as u32 {
            return None;
        }
        let mut icons = Vec::new();
        for key in keys {
            let to = pointer(&mut r).ok()?;
            if to.file == 0 && to.path != 0 {
                icons.push((key, (file.clone(), to.path)));
            }
        }
        Some(icons)
    };
    let mut found = (0..bytes.len()).step_by(4).filter_map(table_at);
    let plain = found.next().context("the static icons have no wish list's icons")?;
    let outlined = found.next().context("the static icons have no wish list's outlined icons")?;
    Ok([plain, outlined])
}

pub fn scene_paths(install: &Install) -> Result<Vec<String>> {
    let path = install.root.join("EscapeFromTarkov_Data").join("globalgamemanagers");
    let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let meta = SerializedFile::parse(&data).context("globalgamemanagers")?;
    let object = meta.objects.iter().find(|o| o.class_id == BUILD_SETTINGS).context("globalgamemanagers has no build settings")?;
    let mut r = Reader::new(&data[object.offset..object.offset + object.size]);
    let count = r.u32()? as usize;
    anyhow::ensure!(count < 10_000, "the build settings do not begin with the list of scenes");
    (0..count).map(|_| string(&mut r)).collect()
}
