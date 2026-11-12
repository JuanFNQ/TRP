//! Assembling a weapon from a preset of its parts.

use super::*;

/// A build out of its key (`build_key`): its items numbered in the order
/// they are named, the weapon first. None for a key with no parts' list
/// (a template alone) or one not well made.
pub fn preset_of_key(key: &str) -> Option<Preset> {
    fn item(text: &str, at: &mut usize, parent: Option<(String, String)>, items: &mut Vec<PresetItem>) -> Option<()> {
        let rest = &text[*at..];
        let end = rest.find(KEY_OPEN)?;
        let template = rest[..end].to_owned();
        if template.is_empty() || template.contains([KEY_CLOSE, KEY_NEXT, KEY_SLOT]) {
            return None;
        }
        *at += end + 1;
        let id = items.len().to_string();
        let (parent, slot) = parent.map_or((None, None), |(parent, slot)| (Some(parent), Some(slot)));
        items.push(PresetItem { id: id.clone(), template, parent, slot });
        loop {
            match text[*at..].chars().next()? {
                KEY_CLOSE => {
                    *at += 1;
                    return Some(());
                }
                KEY_NEXT => *at += 1,
                _ => {
                    let rest = &text[*at..];
                    let equals = rest.find(KEY_SLOT)?;
                    let slot = rest[..equals].to_owned();
                    *at += equals + 1;
                    item(text, at, Some((id.clone(), slot)), items)?;
                }
            }
        }
    }
    if !key.contains(KEY_OPEN) {
        return None;
    }
    let mut items = Vec::new();
    let mut at = 0;
    item(key, &mut at, None, &mut items)?;
    if at != key.len() {
        return None;
    }
    Some(Preset { id: key.to_owned(), name: String::new(), root: "0".to_owned(), items, encyclopedia: None })
}

/// Assembles a build of a weapon: its root item and each part with the
/// slot of its parent it is in (a preset's list: the stock one, another of
/// the install's, or a build out of its key).
pub fn assemble_preset(assets: &mut Assets, db: &Database, preset: &Preset) -> Result<Weapon> {
    let root_item = preset
        .items
        .iter()
        .find(|i| i.id == preset.root)
        .ok_or_else(|| anyhow!("preset {} has no root item", preset.id))?;
    let weapon_template = root_item.template.as_str();
    let base = db.item(&root_item.template)?;
    let key = base.prefab_key().ok_or_else(|| anyhow!("{} has no prefab", base.name))?;
    let (prefab_obj, prefab, mut hierarchy) = open_prefab(assets, key)?;

    let mut root_of: HashMap<&str, usize> = HashMap::from([(root_item.id.as_str(), 0)]);
    let mut skipped = Vec::new();
    let mut remaining: Vec<_> = preset.items.iter().filter(|i| i.id != root_item.id).collect();
    // A weapon of one's own is the donor's bare prefab with the model's
    // own parts on its bones. Its magazine's own prefab is hung as the
    // game hangs it, the model's magazine below it and nothing of the
    // prefab drawn, for what the game goes by on a magazine's object
    // (where its rounds are drawn), and another weapon's muzzle effects
    // are laid at its fireport where its file names one (`crate::custom`).
    // The install's parts in its own slots (`custom::SlotDef`) are hung
    // below as any weapon's, on the bones its model brings.
    let mut muzzle_key = None;
    let mut custom = match db.custom.get(weapon_template) {
        Some(definition) => {
            let magazine = remaining.iter().find(|item| item.slot.as_deref() == Some("mod_magazine")).copied();
            remaining.retain(|item| item.slot.as_deref() != Some("mod_magazine"));
            let mut hung = Vec::new();
            if let Some(magazine) = magazine {
                let item = db.item(&magazine.template)?;
                match attach_part(assets, &mut hierarchy, 0, "mod_magazine", item.prefab_key()) {
                    Ok(root) => {
                        hung.push(root);
                        root_of.insert(magazine.id.as_str(), root);
                    }
                    Err(e) => skipped.push(format!("{} in mod_magazine: {e:#}", item.name)),
                }
            }
            let build = crate::weapons::custom::build(definition, &mut hierarchy, &hung).with_context(|| format!("building {}", definition.name))?;
            if let Some(from) = &definition.muzzle_from {
                let other = assemble(assets, db, from).with_context(|| format!("{}'s muzzle", definition.name))?;
                crate::weapons::custom::take_muzzle(&other.hierarchy, &mut hierarchy).with_context(|| format!("{}'s muzzle from {}", definition.name, other.name))?;
                let preset = db.default_preset(from)?;
                let root = preset.items.iter().find(|item| item.id == preset.root).ok_or_else(|| anyhow!("preset {} has no root item", preset.id))?;
                muzzle_key = db.item(&root.template)?.prefab_key().map(str::to_owned);
            }
            Some(build)
        }
        None => None,
    };
    // A part of one's own (`crate::custom::PartDefinition`) is hung as its
    // donor is, made its own, and its model drawn with the weapon's: on an
    // install's weapon, with no edits to its pose.
    while !remaining.is_empty() {
        let before = remaining.len();
        let mut later = Vec::new();
        for part in remaining {
            let parent_root = part.parent.as_deref().and_then(|p| root_of.get(p).copied());
            let (Some(parent_root), Some(slot)) = (parent_root, part.slot.as_deref()) else {
                later.push(part);
                continue;
            };
            let item = db.item(&part.template)?;
            let hung = hierarchy.find_below(parent_root, slot).ok_or_else(|| anyhow!("no bone named {slot}")).and_then(|bone| crate::weapons::custom::hang(assets, db, &mut hierarchy, bone, &part.template));
            match hung {
                Ok((index, own)) => {
                    root_of.insert(part.id.as_str(), index);
                    if let Some(own) = own {
                        custom.get_or_insert_with(|| crate::weapons::custom::Build { parts: Vec::new(), materials: Vec::new(), edits: std::sync::Arc::new(crate::weapons::custom::PoseEdits::none()) }).take(own);
                    }
                }
                Err(e) => skipped.push(format!("{} in {slot}: {e:#}", item.name)),
            }
        }
        if later.len() == before {
            skipped.extend(later.iter().map(|p| format!("{}: parent was not placed", p.template)));
            break;
        }
        remaining = later;
    }

    let sum = |prop: &str| -> f32 {
        preset.items.iter().filter_map(|i| db.items.get(&i.template)?.prop_f32(prop)).sum()
    };
    let parts = || preset.items.iter().filter(|i| i.id != root_item.id).filter_map(|i| db.items.get(&i.template));
    let recoil_delta = parts().filter_map(|t| t.prop_f32("Recoil")).sum::<f32>() / 100.0;
    let magazine_capacity = preset
        .items
        .iter()
        .find(|i| i.slot.as_deref() == Some("mod_magazine"))
        .and_then(|i| db.items.get(&i.template))
        .and_then(|t| t.props.get("Cartridges")?.get(0)?.get("_max_count")?.as_u64())
        .map(|n| n as u32);
    let magazine_animation = preset
        .items
        .iter()
        .find(|i| i.slot.as_deref() == Some("mod_magazine"))
        .and_then(|i| db.items.get(&i.template))
        .and_then(|t| t.props.get("magAnimationIndex")?.as_i64())
        .map(|n| n as i32);
    let magazine_external = preset
        .items
        .iter()
        .find(|i| i.slot.as_deref() == Some("mod_magazine"))
        .and_then(|i| db.items.get(&i.template))
        .map(|t| t.props.get("ReloadMagType").and_then(serde_json::Value::as_str) == Some("ExternalMagazine"));
    // Every part of the build, placed or not (`Item.AllSlots`).
    let unique_animation = preset
        .items
        .iter()
        .filter(|i| i.id != preset.root)
        .filter_map(|i| db.items.get(&i.template)?.props.get("UniqueAnimationModID")?.as_i64())
        .max()
        .unwrap_or(0)
        .max(0) as i32;
    // The game weighs a weapon with what is loaded in it. A recorded trace
    // gives the stock AKM as 3.867 kg: the parts, and 31 rounds.
    let round_weight = base
        .props
        .get("defAmmo")
        .and_then(|a| a.as_str())
        .and_then(|id| db.items.get(id))
        .and_then(|a| a.prop_f32("Weight"))
        .unwrap_or(0.0);
    let rounds = magazine_capacity.unwrap_or(0) as f32 + base.prop_f32("chamberAmmoCount").unwrap_or(0.0);
    // The weapon's own width, the longest reach of any part to each side,
    // and every part that adds its length whatever the others do.
    let (mut left, mut right, mut forced) = (0.0f32, 0.0f32, 0.0f32);
    for part in parts() {
        let (l, r) = (part.prop_f32("ExtraSizeLeft").unwrap_or(0.0), part.prop_f32("ExtraSizeRight").unwrap_or(0.0));
        if part.props.get("ExtraSizeForceAdd").and_then(|v| v.as_bool()).unwrap_or(false) {
            forced += l + r;
        } else {
            left = left.max(l);
            right = right.max(r);
        }
    }
    let cell_width = (base.prop_f32("Width").unwrap_or(1.0) + left + right + forced).max(1.0) as u32;

    // The stock that folds: the weapon's own fold if its template has one,
    // else the first part that has (a stock). It folds if what its folded
    // slot names is fitted and no part blocks folding.
    let flag = |props: &serde_json::Value, name: &str| props.get(name).and_then(|v| v.as_bool()).unwrap_or(false);
    let folds = |props: &serde_json::Value| flag(props, "Foldable") || flag(props, "Retractable");
    let text = |props: &serde_json::Value, name: &str| props.get(name).and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let part_items: Vec<_> = preset.items.iter().filter(|i| i.id != root_item.id).collect();
    let template_of = |id: &str| preset.items.iter().find(|i| i.id == id).and_then(|i| db.items.get(&i.template));
    // Whether an item of the preset hangs below another, at any depth.
    let below = |item: &str, ancestor: &str| {
        let mut at = preset.items.iter().find(|i| i.id == item).and_then(|i| i.parent.clone());
        while let Some(parent) = at {
            if parent == ancestor {
                return true;
            }
            at = preset.items.iter().find(|i| i.id == parent).and_then(|i| i.parent.clone());
        }
        false
    };
    let folding = if folds(&base.props) {
        Some(root_item.id.clone())
    } else {
        part_items.iter().find(|i| template_of(&i.id).is_some_and(|t| folds(&t.props))).map(|i| i.id.clone())
    };
    let shoulder = |id: &str| template_of(id).is_some_and(|t| flag(&t.props, "HasShoulderContact"));
    let compact = flag(&base.props, "CompactHandling");
    let blocked = part_items.iter().any(|i| template_of(&i.id).is_some_and(|t| flag(&t.props, "BlocksFolding")));
    let fold = folding.as_deref().filter(|_| !blocked).and_then(|id| {
        let template = template_of(id)?;
        let slot = text(&template.props, "FoldedSlot");
        // What sits in the slot the fold names, if it names one.
        let folded_part = part_items.iter().find(|i| i.parent.as_deref() == Some(id) && i.slot.as_deref() == Some(slot.as_str()));
        if !slot.is_empty() && folded_part.is_none() {
            return None;
        }
        let reduce = template.prop_f32("SizeReduceRight").unwrap_or(0.0);
        let own = id == root_item.id;
        // Folded, the stock's own recoil is left out of the parts' share.
        let stock_recoil: f32 = part_items
            .iter()
            .filter_map(|i| template_of(&i.id))
            .filter(|t| db.is_a(t, "Stock"))
            .filter_map(|t| t.prop_f32("Recoil"))
            .sum();
        // And its reach to the right is shorter; what hangs in the folded
        // slot no longer counts for the size at all.
        let (mut left, mut right, mut forced) = (0.0f32, 0.0f32, if own { -reduce } else { 0.0 });
        for part in &part_items {
            if folded_part.is_some_and(|folded| part.id == folded.id || below(&part.id, &folded.id)) {
                continue;
            }
            let Some(t) = template_of(&part.id) else { continue };
            let (l, mut r) = (t.prop_f32("ExtraSizeLeft").unwrap_or(0.0), t.prop_f32("ExtraSizeRight").unwrap_or(0.0));
            if part.id == id {
                r -= reduce;
            }
            if flag(&t.props, "ExtraSizeForceAdd") {
                forced += l + r;
            } else {
                left = left.max(l);
                right = right.max(r);
            }
        }
        // Folded, neither the stock nor what its slot holds reaches the
        // shoulder.
        let reaches = part_items.iter().any(|part| {
            let folded_away = part.id == id || folded_part.is_some_and(|folded| part.id == folded.id || below(&part.id, &folded.id));
            !folded_away && shoulder(&part.id)
        });
        Some(Fold {
            recoil_delta: recoil_delta - stock_recoil / 100.0,
            cell_width: (base.prop_f32("Width").unwrap_or(1.0) + left + right + forced).max(1.0) as u32,
            held_closer: compact && !reaches,
        })
    });
    // Unfolded, a weapon whose own template folds with no slot named
    // always counts as reaching the shoulder.
    // The bones a fold turns: the fold's view on the model of the thing
    // that folds, the weapon's own or a part's (`FoldableStockView` is set
    // up with its own model's item, and does nothing where that item has
    // no fold: the M3's weapon has one that names its stock, which is the
    // stock's to fold), found below that model's root by name.
    let mut fold_bones = Vec::new();
    if let Some((id, key)) = folding.as_deref().and_then(|id| Some((id, template_of(id)?.prefab_key()?.to_owned()))) {
        if let Some(&root) = root_of.get(id) {
            let files = assets.load(&key)?;
            // A bone not on the model is passed over, as the game's view
            // passes it (the SCAR-L's names its stocks of both colours).
            for (name, folded, unfolded) in fold_view(assets, &files)? {
                if let Some(node) = hierarchy.find_below(root, &name) {
                    fold_bones.push(FoldBone { node, folded, unfolded });
                }
            }
        }
    }
    let own_fold_reaches = fold.is_some() && folds(&base.props) && text(&base.props, "FoldedSlot").is_empty();
    let held_closer = compact && !own_fold_reaches && !part_items.iter().any(|part| shoulder(&part.id));
    let silenced = parts().any(|t| db.is_a(t, "Silencer") || flag(&t.props, "IsSilencer"));
    let files = assets.load(key)?;
    let (extraction, patron_extraction) = extraction(assets, &files)?.unzip();
    let vest_linked = vest_linked(assets, &files)?;
    let shell_port = shell_port(&hierarchy);
    let shell_ports = shell_ports(&hierarchy);
    let muzzle = match &muzzle_key {
        Some(other) => {
            let theirs = assets.load(other)?;
            crate::weapons::muzzle::read(assets, &theirs, &hierarchy)?
        }
        None => crate::weapons::muzzle::read(assets, &files, &hierarchy)?,
    };
    let (mut sights, folding_sights) = crate::weapons::sight::read(assets, db, preset, &root_of, &hierarchy)?;
    // A scope of one's own with a reticle of its own: its lens draws that
    // in place of its donor's (`crate::custom::MarkDef`).
    for sight in &mut sights {
        let Some(mark) = db.custom_parts.get(&sight.template).and_then(|part| part.mark_picture()) else { continue };
        for lens in sight.lenses.iter_mut().filter(|lens| matches!(lens.kind, crate::weapons::sight::LensKind::Optic(_))) {
            lens.pictures.retain(|(name, _)| name != "_MarkTex");
            lens.pictures.push(("_MarkTex".to_owned(), mark.clone()));
        }
    }
    let devices = crate::weapons::tactical::read(assets, db, preset, &root_of, &hierarchy)?;
    // The bipod: the first part of the build, in the game's walk of the
    // slots, that is one (`WeaponModified`: `Item.AllSlots` where the
    // contained item is a `Bipod`).
    let mut bipod = None;
    for item in crate::weapons::sight::all_slots_order(db, preset) {
        let Some(template) = db.items.get(&item.template) else { continue };
        if !db.is_a(template, "Bipod") {
            continue;
        }
        let Some(&node) = root_of.get(item.id.as_str()) else { continue };
        let (forward, animator) = match template.prefab_key() {
            Some(key) => {
                let files = assets.load(key)?;
                (bipod_forward(assets, &files)?, part_animator(assets, &files))
            }
            None => (false, None),
        };
        bipod = Some(BipodPart { template: item.template.clone(), node, ground: hierarchy.find_below(node, "bipod_ground"), forward, bundle: template.prefab_key().map(str::to_owned), animator });
        break;
    }
    // The underbarrel launcher: the first part, in the same walk, that is
    // one (`Weapon.GetUnderbarrelWeapon`).
    let mut launcher = None;
    let mut launcher_muzzle = None;
    for item in crate::weapons::sight::all_slots_order(db, preset) {
        let Some(template) = db.items.get(&item.template) else { continue };
        if !db.is_a(template, "Launcher") {
            continue;
        }
        let Some(&node) = root_of.get(item.id.as_str()) else { continue };
        let (controller, extraction) = match template.prefab_key() {
            Some(key) => {
                let files = assets.load(key)?;
                launcher_muzzle = crate::weapons::muzzle::read_under(assets, &files, &hierarchy, node)?;
                (launcher_controller(assets, &files), self::extraction(assets, &files)?.map(|(shot, _)| shot))
            }
            None => (None, None),
        };
        launcher = Some(LauncherPart {
            template: item.template.clone(),
            linked: template.props.get("LinkedWeapon").and_then(|id| id.as_str()).filter(|id| !id.is_empty()).map(str::to_owned),
            node,
            bundle: template.prefab_key().map(str::to_owned),
            controller,
            caseless: template.props.get("UseAmmoWithoutShell").and_then(|on| on.as_bool()).unwrap_or(false),
            fireport: hierarchy.find_first_below(node, "fireport"),
            shell_port: hierarchy.find_first_below(node, "shellport"),
            chamber: hierarchy.find_first_below(node, "patron_in_weapon"),
            extraction,
            stock_recoil_delta: part_items.iter().filter_map(|i| template_of(&i.id)).filter(|t| db.is_a(t, "Stock")).filter_map(|t| t.prop_f32("Recoil")).sum::<f32>() / 100.0,
        });
        break;
    }
    let hot = crate::weapons::heat::read(assets, &hierarchy);
    Ok(Weapon {
        vest_linked,
        bipod,
        launcher,
        launcher_muzzle,
        hot,
        sights,
        folding_sights,
        devices,
        extraction,
        patron_extraction,
        shell_port,
        shell_ports,
        muzzle,
        template: weapon_template.to_owned(),
        name: base.name.clone(),
        hierarchy,
        prefab,
        prefab_file: prefab_obj.file,
        skipped,
        total_weight: sum("Weight") + rounds * round_weight,
        total_ergonomics: sum("Ergonomics"),
        recoil_delta,
        speed_factor: 1.0 + sum("Velocity") / 100.0,
        magazine_capacity,
        magazine_animation,
        magazine_external,
        unique_animation,
        cell_width,
        held_closer,
        fold,
        fold_bones,
        custom: custom.map(std::sync::Arc::new),
        silenced,
        sound_kind: Some(crate::audio::sound::sound_weapon_kind(db, base)),
    })
}
