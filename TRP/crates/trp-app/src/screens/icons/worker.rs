//! The icon worker: its models, the switched parts and their fitting.

use super::*;

/// An item's model as the game makes it for its picture and for the world
/// (`ObjectsFactory`): its tree, what each node draws, whether its first
/// node keeps its own place and turn, and the pose its prefab gives its
/// picture.
pub(crate) struct Model {
    pub hierarchy: Hierarchy,
    pub keeps_own: bool,
    pub looks: PreparedLooks,
    pub parts: Vec<Vec<(usize, usize)>>,
    pub extra: Vec<(usize, usize, usize)>,
    /// What casts its shadow alone (`ShadowCastingMode.ShadowsOnly`: a
    /// repair kit's `shadow_LOD0`), by node, mesh and material: left out
    /// of the picture and of every bound, cast on the ground
    /// (`loose`).
    pub shadows: Vec<(usize, usize, usize)>,
    pub pose: Option<Pose>,
    /// A weapon's tactical devices, as its assembly reads them
    /// (`trp_data::tactical::read`, numbered by `hierarchy`'s objects):
    /// lit on a weapon hung on the body (`slung_view`); none for anything
    /// else.
    pub devices: Vec<trp_data::weapons::tactical::Device>,
    /// A model for the world's, with the levels of detail drawn by
    /// (`lod_view`): what each node draws at a coarser level only (its
    /// `lower` renderer, not on bones), as `parts`. Empty for a picture's.
    pub lower: Vec<Vec<(usize, usize)>>,
}

/// What the icon thread makes of an item: its model, for the studio to
/// draw; or its prefab's own picture, as large as its view, as it lies and
/// turned (`Worker::own_picture`).
pub(crate) enum Made {
    Model(PreparedIcon),
    Own([u32; 2], Vec<u8>, Vec<u8>),
}

/// The icon thread's own reader of the install and its database, and of
/// the other game's folders.
pub(crate) struct Worker {
    pub unity: trp_data::unity::Assets,
    pub(super) db: std::sync::Arc<trp_data::Database>,
    pub(super) stalker: trp_data::formats::xray::Stalker,
    pub(super) studio: Option<Studio>,
    /// It makes items' pictures (`for_pictures`): their colour pictures
    /// read as stored, for the game's own surface alone.
    pub(super) pictures: bool,
}

/// A reader of the install for the icon thread, the cubemaps' bundle open:
/// a material's cube (`_Cube`) is there, and a reference into a bundle not
/// open is not followed (`read_material`), which the game's own surface
/// reflects (`game_surface`). Without it a material is drawn with the
/// engine's default cube, as one that names none.
pub(crate) fn reader(install: &trp_data::Install) -> anyhow::Result<trp_data::unity::Assets> {
    let mut unity = trp_data::unity::Assets::new(install.clone())?;
    if let Err(e) = unity.load("cubemaps") {
        warn!("the cubemaps could not be read ({e:#}): every material is drawn with the default cube");
    }
    Ok(unity)
}

impl Worker {
    /// The run's database is shared (`GameData::db`): made again with the
    /// reader each time the queue emptied, the thread read its own anew.
    pub(crate) fn new(install: &trp_data::Install, db: std::sync::Arc<trp_data::Database>) -> anyhow::Result<Self> {
        let studio = Studio::read(install).unwrap_or_else(|e| {
            warn!("the game's icon camera could not be read ({e:#}): it is taken as it was last read by hand");
            Studio::as_read()
        });
        let stalker = trp_data::formats::xray::Stalker::locate();
        Ok(Self { unity: reader(install)?, db, stalker, studio: Some(studio), pictures: false })
    }

    /// The icon thread's: its models are drawn with the game's own surface
    /// alone, their colour pictures read as stored
    /// (`PreparedLooks::raw_colours`). A model for the world is drawn with
    /// the standard surface too, which reads them as sRGB.
    pub(crate) fn for_pictures(mut self) -> Self {
        self.pictures = true;
        self
    }

    /// The picture an item's prefab names for itself (`PreviewPivot.Icon.
    /// overrideIcon`, which the game takes in place of the model drawn:
    /// an armour's built-in insert, whose model draws nothing), as large as
    /// its view of `cells` (63 units a cell and one): its sprite at its own
    /// size in the middle, shrunk to fit where larger (the game's item view
    /// sets it to its own size, `SetNativeSize`; a slot shrinks it to fit,
    /// `UpdateScale`); as it lies and turned. None where it names none.
    pub(super) fn own_picture(&mut self, key: &str, cells: [u32; 2]) -> anyhow::Result<Option<([u32; 2], Vec<u8>, Vec<u8>)>> {
        // (By the key's template alone: a magazine's key carries its
        // rounds, a folding weapon's its fold.)
        let template = split_key(key).template;
        // A part of one's own is drawn from its own model, never its donor's
        // picture.
        if self.db.custom_parts.contains_key(template) {
            return Ok(None);
        }
        let Some(prefab) = self.db.item(template)?.prefab_key().map(str::to_owned) else { return Ok(None) };
        let files = self.unity.load(&prefab)?;
        let Some(([w, h], sprite)) = trp_data::interface::icon::override_icon(&self.unity, &files)? else { return Ok(None) };
        let size = [cells[0] * CELL_PIXELS + 1, cells[1] * CELL_PIXELS + 1];
        let share = (size[0] as f32 / w as f32).min(size[1] as f32 / h as f32).min(1.0);
        let (dw, dh) = (((w as f32 * share).round() as u32).max(1), ((h as f32 * share).round() as u32).max(1));
        let (left, top) = ((size[0] - dw) / 2, (size[1] - dh) / 2);
        let mut canvas = vec![0u8; (size[0] * size[1] * 4) as usize];
        for y in 0..dh {
            for x in 0..dw {
                // (Nearest of the sprite's pixels, `PROVISIONAL`: the
                // engine scales it on the card.)
                let sx = (((x as f32 + 0.5) / share) as u32).min(w - 1);
                let sy = (((y as f32 + 0.5) / share) as u32).min(h - 1);
                let from = ((sy * w + sx) * 4) as usize;
                let to = (((top + y) * size[0] + left + x) * 4) as usize;
                canvas[to..to + 4].copy_from_slice(&sprite[from..from + 4]);
            }
        }
        let turned = trp_data::interface::icon::turned(&canvas, size[0] as usize, size[1] as usize);
        Ok(Some((size, canvas, turned)))
    }

    /// An item's model, posed for its picture of `cells`, by the picture's
    /// key (`picture_key`: its template or its parts, marked where its fold
    /// is folded, with a magazine's rounds drawn and its switches on); or,
    /// for a model with no renderer switched on (the TKPD's muzzle, an
    /// empty object), an empty picture of its size, as the game's camera
    /// draws nothing of it (icons spec, "The same model").
    pub(super) fn prepare(&mut self, key: &str, cells: [u32; 2]) -> anyhow::Result<Made> {
        let asked = split_key(key);
        let Model { hierarchy, keeps_own, looks, parts, extra, shadows, pose, .. } = self.model_switched(asked.build.unwrap_or(asked.template), asked.folded, &asked.rounds, &asked.on)?;
        Ok(match self.framed(key, cells, hierarchy, keeps_own, looks, (parts, extra, &shadows), pose)? {
            Some(prepared) => Made::Model(prepared),
            None => {
                let size = [cells[0] * CELL_PIXELS + 1, cells[1] * CELL_PIXELS + 1];
                let empty = vec![0u8; (size[0] * size[1] * 4) as usize];
                Made::Own(size, empty.clone(), empty)
            }
        })
    }

    /// An item's model, read, by its template or a weapon's build's key
    /// (`trp_sim::inventory::rig_key`: the weapon with those parts); a
    /// weapon's with its fold folded or not, as its model not animated
    /// shows it (`FoldableStockView`); a magazine's with `rounds` (its
    /// rounds' templates from the top) in its points
    /// (`SpringMagazineVisual`; inventory spec, "A magazine's rounds
    /// drawn").
    pub(crate) fn model(&mut self, key: &str, folded: bool, rounds: &[&str]) -> anyhow::Result<Model> {
        self.model_switched(key, folded, rounds, &[])
    }

    /// The same, a thing other than a weapon with the parts its key names
    /// hung on it, and those of its parts named in `on` (`picture_key`'s
    /// places) switched on (`fit_parts`).
    fn model_switched(&mut self, key: &str, folded: bool, rounds: &[&str], on: &[&str]) -> anyhow::Result<Model> {
        let template = trp_sim::items::inventory::key_template(key);
        // An outfit of the other game's has that game's model of it, where
        // its file names one and it can be read; else its donor's.
        if let Some(world) = self.db.outfits.get(template).and_then(|outfit| outfit.world_visual.clone()) {
            match self.stalker_model(template, &world) {
                Ok(model) => return Ok(model),
                Err(e) => warn!("the model of {template} is its donor's: its own model {world} could not be read: {e:#}"),
            }
        }
        let item = self.db.item(template)?;
        let prefab = item.prefab_key().ok_or_else(|| anyhow::anyhow!("it has no model (a container of the character's own, as the pockets)"))?.to_owned();
        // A weapon of one's own's magazine is its object as that weapon
        // hangs it, the model's magazine drawn (`trp_data::custom::
        // magazine_model`), posed for its picture as the prefab it is made
        // from would be.
        if let Some(owner) = self.db.custom.values().find(|definition| definition.magazine.id == template).map(|definition| definition.id.clone()) {
            let assembled = trp_data::weapons::weapon::assemble(&mut self.unity, &self.db, &owner)?;
            let (mut hierarchy, build) = trp_data::weapons::custom::magazine_model(&assembled)?;
            if !rounds.is_empty() {
                self.fill_points(&mut hierarchy, rounds);
            }
            let files = self.unity.load(&prefab)?;
            let pose = trp_data::interface::icon::pose_in(&self.unity, &files)?.map(|(_, pose)| pose);
            return Ok(self.model_from(hierarchy, false, pose, Some(&build)));
        }
        // A part of one's own is its donor's prefab made its own, with the
        // parts its key names in its slots (`trp_data::custom::
        // part_model`), posed for its picture as its donor would be.
        if self.db.custom_parts.contains_key(template) {
            let build = trp_data::weapons::weapon::preset_of_key(key).unwrap_or_else(|| trp_data::weapons::weapon::alone(template));
            let (hierarchy, own) = trp_data::weapons::custom::part_model(&mut self.unity, &self.db, &build)?;
            let files = self.unity.load(&prefab)?;
            let pose = trp_data::interface::icon::pose_in(&self.unity, &files)?.map(|(_, pose)| pose);
            return Ok(self.model_from(hierarchy, false, pose, Some(&own)));
        }
        let weapon = self.db.is_a(item, "Weapon");
        // Any other item whose prefab is a weapon's container holds no
        // model of its own: the game's pool hangs the object its
        // `WeaponPrefab` names under it as it makes it, whatever the item
        // (`trp_data::weapon::is_weapon_container`): a grenade's, a melee
        // weapon's, the rangefinder's, an under-barrel launcher's, the
        // radio transmitter's. (Taken by the item's class, it left the
        // launchers and the transmitter with nothing drawn.)
        let held = !weapon && trp_data::weapons::weapon::is_weapon_container(&mut self.unity, &prefab)?;
        // A weapon with its parts, hung from its container's pose; any
        // other item, its prefab from the object its pose is on.
        let mut devices = Vec::new();
        let (hierarchy, pose, custom) = if weapon || held {
            let mut assembled = if weapon { trp_data::weapons::weapon::assemble_key(&mut self.unity, &self.db, key)? } else { trp_data::weapons::weapon::assemble_prefab(&mut self.unity, &prefab)? };
            // A weapon's iron sights folded under a scope that looks along
            // the bore, as the game makes any model of a weapon.
            if weapon {
                trp_sim::weapons::sights::fold_sights_as_made(&mut assembled);
            }
            assembled.fold_tree(folded);
            let files = self.unity.load(&prefab)?;
            let pose = trp_data::interface::icon::pose_in(&self.unity, &files)?.map(|(_, pose)| pose);
            devices = std::mem::take(&mut assembled.devices);
            (assembled.hierarchy, pose, assembled.custom)
        } else {
            let (mut hierarchy, pose) = trp_data::interface::icon::model_of(&mut self.unity, &prefab)?;
            if !rounds.is_empty() {
                self.fill_points(&mut hierarchy, rounds);
            }
            if let Some(build) = trp_data::weapons::weapon::preset_of_key(key) {
                hierarchy = self.fit_parts(template, hierarchy, &build, on);
            }
            (hierarchy, pose, None)
        };
        Ok(Model { devices, ..self.model_from(hierarchy, weapon || held, pose, custom.as_deref()) })
    }

    /// A thing's model with the parts of its build (`preset_of_key`) hung
    /// on it, as the game makes any model of a thing with what its slots
    /// hold (`ObjectsFactory.CreateItemAsync`, `AttachMods`: the worn
    /// thing's rule, `trp_data::dress::head_model`), and each hinge turned
    /// at once by its face shield's switch, as a model made not animated
    /// has it (`HelmetDress.Init`, its filter set at once; icons spec, "A
    /// thing's parts and switches").
    fn fit_parts(&mut self, template: &str, hierarchy: Hierarchy, build: &trp_data::setup::database::Preset, on: &[&str]) -> Hierarchy {
        let items = &build.items;
        let of = |id: &str| items.iter().find(|item| item.id == id);
        // Each part's way down: the slots and templates from the thing to it.
        let path = |item: &trp_data::setup::database::PresetItem| {
            let mut path = Vec::new();
            let mut at = Some(item);
            while let Some(part) = at.filter(|part| part.parent.is_some()) {
                path.push((part.slot.clone().unwrap_or_default(), part.template.clone()));
                at = part.parent.as_deref().and_then(of);
            }
            path.reverse();
            path
        };
        let fitted: Vec<Vec<(String, String)>> = items.iter().filter(|item| item.parent.is_some()).map(path).collect();
        let wanted: HashSet<String> = fitted.iter().flatten().map(|(_, template)| template.clone()).collect();
        let (mut hierarchy, hung, hinges) = trp_data::character::dress::head_model(&mut self.unity, &self.db, template, hierarchy, &|taken| wanted.contains(taken));
        for part in hung.iter().filter(|part| fitted.contains(&part.path)) {
            hierarchy.nodes[part.root].active = true;
        }
        let db = &self.db;
        let props = |item: &trp_data::setup::database::PresetItem, name: &str| db.items.get(&item.template).and_then(|kind| kind.props.get(name)).and_then(|flag| flag.as_bool()).unwrap_or(false);
        let switched = |item: &trp_data::setup::database::PresetItem| props(item, "HasHinge") || db.items.get(&item.template).is_some_and(|kind| db.is_a(kind, "NightVision") || db.is_a(kind, "ThermalVision"));
        // Each hinge by the thing whose model carries it (the thing itself,
        // or a part fitted: a face shield's own).
        for hinge in hinges {
            let Some(owner) = items.iter().find(|item| path(item) == hinge.owner) else { continue };
            // Its first face shield, itself first, each part before the
            // parts fitted to it, by its slots' order
            // (`GetItemComponentsInChildren<FaceShieldComponent>`); down
            // where its switch is on or where it has none (`trp_sim::
            // inventory::hinge_down`). With none the filter is never set as
            // the model is made (`SetFaceShield` holds the same none it
            // began with), and the hinge keeps the model's own turn.
            let mut order = Vec::new();
            let mut stack = vec![owner];
            while let Some(item) = stack.pop() {
                order.push(item);
                let slots: Vec<String> = db.items.get(&item.template).map(|kind| kind.slots().into_iter().map(|slot| slot.name).collect()).unwrap_or_default();
                let mut below: Vec<&trp_data::setup::database::PresetItem> = items.iter().filter(|part| part.parent.as_deref() == Some(item.id.as_str())).collect();
                below.sort_by_key(|part| slots.iter().position(|slot| Some(slot) == part.slot.as_ref()).unwrap_or(usize::MAX));
                stack.extend(below.into_iter().rev());
            }
            let Some(shield) = order.into_iter().find(|item| props(item, "FaceShieldComponent")) else { continue };
            let down = !switched(shield) || on.contains(&place_of(&path(shield)).as_str());
            let mut filter = trp_sim::base::hysteresis::Hysteresis::new(hinge.forward.clone(), hinge.backward.clone());
            let value = filter.set(down, true).unwrap_or(if down { 1.0 } else { 0.0 });
            hierarchy.nodes[hinge.node].rotation = trp_sim::base::math::quat_lerp(hinge.off, hinge.on, value);
        }
        hierarchy
    }

    /// A magazine's points filled with so many rounds, their templates from
    /// the top (`FillAmmoObjectsOnStart`): a point numbered past them off,
    /// any other on with its round's model hung on it.
    fn fill_points(&mut self, hierarchy: &mut trp_data::unity::extract::Hierarchy, rounds: &[&str]) {
        let count = rounds.len() as u32;
        let points = trp_sim::items::magazine_view::points(hierarchy.nodes.iter().map(|node| (node.parent, node.name.as_str())), 0);
        for (place, (node, number)) in points.into_iter().enumerate() {
            let round = trp_sim::items::magazine_view::round_at(place, number, count).and_then(|from_top| rounds.get(from_top as usize));
            hierarchy.nodes[node].active = round.is_some();
            let Some(key) = round.and_then(|round| self.db.items.get(*round)).and_then(|item| item.prefab_key()).map(str::to_owned) else { continue };
            if let Err(e) = trp_data::weapons::weapon::attach_round(&mut self.unity, hierarchy, node, &key) {
                warn!("a magazine's round {key} is not drawn in its picture: {e:#}");
            }
        }
    }
}
