//! Finding custom weapons and parts in their folder and registering them.

use super::*;

/// The workspace's folder of such files: the nearest `custom` folder
/// beside a `Cargo.toml`, from where the program runs upwards.
pub fn folder() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("custom").is_dir() && dir.join("Cargo.toml").is_file() {
            return Some(dir.join("custom"));
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Reads every definition of a folder. One that cannot be read, or whose
/// model is not there, is left out with a word on why.
pub fn definitions(folder: &Path) -> (Vec<Definition>, Vec<String>) {
    let (mut found, mut notes) = (Vec::new(), Vec::new());
    let root = folder.parent().unwrap_or(folder).to_path_buf();
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    for file in files {
        let read = || -> Result<Definition> {
            let text = std::fs::read_to_string(&file)?;
            let mut definition: Definition = serde_json::from_str(&text)?;
            definition.root = root.clone();
            ensure!(definition.model_file().is_file(), "its model {} is not there", definition.model_file().display());
            Ok(definition)
        };
        match read() {
            Ok(definition) => found.push(definition),
            Err(e) => notes.push(format!("{}: {e:#}", file.display())),
        }
    }
    (found, notes)
}

/// Reads every part of a folder of parts' files, each with its file's
/// model. A file that cannot be read, or whose model is not there, is
/// left out with a word on why.
pub fn part_definitions(folder: &Path, root: &Path) -> (Vec<PartDefinition>, Vec<String>) {
    let (mut found, mut notes) = (Vec::new(), Vec::new());
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    for file in files {
        let read = || -> Result<Vec<PartDefinition>> {
            let text = std::fs::read_to_string(&file)?;
            let parts: PartsFile = serde_json::from_str(&text)?;
            ensure!(root.join(&parts.model.file).is_file(), "its model {} is not there", root.join(&parts.model.file).display());
            Ok(parts
                .items
                .into_iter()
                .map(|mut part| {
                    (part.model, part.root) = (parts.model.clone(), root.to_path_buf());
                    part
                })
                .collect())
        };
        match read() {
            Ok(parts) => found.extend(parts),
            Err(e) => notes.push(format!("{}: {e:#}", file.display())),
        }
    }
    (found, notes)
}

/// Adds a part of one's own to the database: its donor's template with
/// what its file changes, its name, and its own slots only.
pub fn register_part(db: &mut Database, part: PartDefinition) -> Result<()> {
    ensure!(!db.items.contains_key(&part.id), "its id is taken");
    let mut template: ItemTemplate = db.item(&part.donor)?.clone();
    template.id = part.id.clone();
    template.name = part.name.clone();
    change(db, &mut template.props, &part.template, None)?;
    for key in ["Name", "ShortName", "Description"] {
        template.props[key] = part.name.clone().into();
    }
    let theirs = template.props.get("Slots").and_then(Json::as_array).cloned().unwrap_or_default();
    let mut slots = Vec::new();
    for (n, own) in part.slots.iter().enumerate() {
        ensure!(!own.takes.is_empty(), "its slot {} takes nothing", own.name);
        let mut slot = theirs.iter().find(|slot| slot.get("_name").and_then(Json::as_str) == Some(own.name.as_str())).cloned().ok_or_else(|| anyhow!("its donor has no slot {}", own.name))?;
        slot["_id"] = format!("{}-slot-{n}", part.id).into();
        slot["_parent"] = part.id.clone().into();
        slot["_props"]["filters"] = serde_json::json!([{ "Filter": own.takes }]);
        slots.push(slot);
    }
    template.props["Slots"] = Json::Array(slots);
    db.items.insert(template.id.clone(), template);
    db.custom_parts.insert(part.id.clone(), Arc::new(part));
    Ok(())
}

impl Definition {
    pub fn model_file(&self) -> PathBuf {
        self.root.join(&self.model.file)
    }

    /// A point of the model, in its own units and axes, on the donor's
    /// weapon bone: metres to the right, back and up. `unit` is how many
    /// centimetres one of the model's units is.
    pub(super) fn on_bone(&self, p: [f32; 3], unit: f32) -> [f32; 3] {
        let [ahead, up, right] = self.model.origin;
        [(p[0] * unit + right) * 0.01, (p[2] * unit - ahead) * 0.01, (p[1] * unit + up) * 0.01]
    }

    /// The item templates this adds to the database, the weapon's and
    /// its magazine's, each with what it is called for short.
    pub fn names(&self) -> [(&str, &str); 2] {
        fn short<'a>(short: &'a str, long: &'a str) -> &'a str {
            if short.is_empty() {
                long
            } else {
                short
            }
        }
        [(&self.id, short(&self.short_name, &self.name)), (&self.magazine.id, short(&self.magazine.short_name, &self.magazine.name))]
    }
}

/// Makes a template's properties what the changes say. `fitted` is what
/// is always fitted to the thing (a weapon's own magazine), whose shares
/// are taken off what a build's figures are set to: the weapon with it
/// fitted then has that build's figures.
fn change(db: &Database, props: &mut Json, changes: &Changes, fitted: Option<&ItemTemplate>) -> Result<()> {
    let map = props.as_object_mut().ok_or_else(|| anyhow!("a template without properties"))?;
    for take in &changes.take {
        let from = db.item(&take.from)?;
        for name in &take.props {
            let value = from.props.get(name).ok_or_else(|| anyhow!("{} has no {name}", from.name))?;
            map.insert(name.clone(), value.clone());
        }
    }
    if let Some(built) = &changes.as_built {
        // What the game adds up over a build: the parts' ergonomics, and
        // their shares of the recoil, which scale both of its forces.
        let preset = db.default_preset(built)?;
        let base = db.item(built)?;
        let parts = || preset.items.iter().filter(|item| item.id != preset.root).filter_map(|item| db.items.get(&item.template));
        let own = |name: &str| fitted.and_then(|part| part.prop_f32(name)).unwrap_or(0.0) as f64;
        let ergonomics: f64 = base.prop_f32("Ergonomics").unwrap_or(0.0) as f64 + parts().filter_map(|part| part.prop_f32("Ergonomics")).map(f64::from).sum::<f64>() - own("Ergonomics");
        let recoil: f64 = (1.0 + parts().filter_map(|part| part.prop_f32("Recoil")).map(f64::from).sum::<f64>() / 100.0).max(0.0) / (1.0 + own("Recoil") / 100.0).max(0.01);
        map.insert("Ergonomics".into(), ergonomics.into());
        for force in ["RecoilForceUp", "RecoilForceBack"] {
            let own = base.prop_f32(force).ok_or_else(|| anyhow!("{} has no {force}", base.name))? as f64;
            map.insert(force.into(), (own * recoil).into());
        }
    }
    for (name, value) in &changes.set {
        map.insert(name.clone(), value.clone());
    }
    Ok(())
}

/// Adds a weapon of one's own to the database: its template, its
/// magazine's and its stock preset.
pub fn register(db: &mut Database, definition: Definition) -> Result<()> {
    ensure!(!db.items.contains_key(&definition.id) && !db.items.contains_key(&definition.magazine.id), "its ids are taken");
    let named = |props: &mut Json, name: &str| {
        for key in ["Name", "ShortName", "Description"] {
            props[key] = name.into();
        }
    };
    let mut magazine: ItemTemplate = db.item(&definition.magazine.clone)?.clone();
    magazine.id = definition.magazine.id.clone();
    magazine.name = definition.magazine.name.clone();
    change(db, &mut magazine.props, &definition.magazine.changes, None).context("its magazine")?;
    named(&mut magazine.props, &definition.magazine.name);

    let mut weapon: ItemTemplate = db.item(&definition.donor)?.clone();
    weapon.id = definition.id.clone();
    weapon.name = definition.name.clone();
    change(db, &mut weapon.props, &definition.template, Some(&magazine))?;
    named(&mut weapon.props, &definition.name);
    // It is one piece: of the donor's slots only the magazine's is kept,
    // and that takes its own magazine.
    let slots = weapon.props.get_mut("Slots").and_then(Json::as_array_mut).ok_or_else(|| anyhow!("the donor has no slots"))?;
    slots.retain(|slot| slot.get("_name").and_then(Json::as_str) == Some("mod_magazine"));
    let slot = slots.first_mut().ok_or_else(|| anyhow!("the donor takes no magazine"))?;
    slot["_parent"] = definition.id.clone().into();
    slot["_props"]["filters"] = serde_json::json!([{ "Filter": [definition.magazine.id] }]);
    // Its own slots, made as the magazine's is, none of them needed.
    let magazine_slot = slot.clone();
    for (n, own) in definition.slots.iter().enumerate() {
        ensure!(own.name != "mod_magazine" && !own.takes.is_empty(), "its slot {} is not one it may have", own.name);
        for part in &own.takes {
            db.item(part).with_context(|| format!("its slot {} takes {part}", own.name))?;
        }
        let mut slot = magazine_slot.clone();
        slot["_name"] = own.name.clone().into();
        slot["_id"] = format!("{}-slot-{n}", definition.id).into();
        slot["_required"] = false.into();
        slot["_props"]["filters"] = serde_json::json!([{ "Filter": own.takes }]);
        slots.push(slot);
    }
    weapon.props["defMagType"] = definition.magazine.id.clone().into();
    if let Some(built) = definition.built_weight {
        let own = built - magazine.prop_f32("Weight").unwrap_or(0.0);
        ensure!(own > 0.0, "it would weigh nothing without its magazine");
        // Kept to the gram, as the database keeps weights.
        weapon.props["Weight"] = ((own as f64 * 1000.0).round() / 1000.0).into();
    }

    let instance = |n: u32| format!("{}-{n}", definition.id);
    let preset = Preset {
        id: format!("{}-preset", definition.id),
        name: definition.name.clone(),
        root: instance(0),
        items: vec![
            PresetItem { id: instance(0), template: definition.id.clone(), parent: None, slot: None },
            PresetItem { id: instance(1), template: definition.magazine.id.clone(), parent: Some(instance(0)), slot: Some("mod_magazine".into()) },
        ],
        encyclopedia: Some(definition.id.clone()),
    };
    db.items.insert(magazine.id.clone(), magazine);
    db.items.insert(weapon.id.clone(), weapon);
    db.presets.push(preset);
    db.custom.insert(definition.id.clone(), Arc::new(definition));
    Ok(())
}

/// Adds every weapon of the workspace's folder to the database. Whatever
/// is left out is noted in `Database::custom_notes`.
pub fn register_all(db: &mut Database) {
    let Some(folder) = folder() else { return };
    // The parts first: a weapon's slots may take them, and one part's
    // another's.
    let root = folder.parent().unwrap_or(&folder).to_path_buf();
    let (parts, mut notes) = part_definitions(&folder.join("parts"), &root);
    for part in parts {
        let name = part.name.clone();
        if let Err(e) = register_part(db, part) {
            notes.push(format!("{name}: {e:#}"));
        }
    }
    for part in db.custom_parts.values() {
        for slot in &part.slots {
            for taken in slot.takes.iter().filter(|taken| !db.items.contains_key(*taken)) {
                notes.push(format!("{}: its slot {} takes {taken}, which is not there", part.name, slot.name));
            }
        }
    }
    let (found, more) = definitions(&folder);
    notes.extend(more);
    for definition in found {
        let name = definition.name.clone();
        if let Err(e) = register(db, definition) {
            notes.push(format!("{name}: {e:#}"));
        }
    }
    db.custom_notes.extend(notes);
}
