//! Reading a scene file and walking its objects, transforms and behaviours.

use super::*;

/// Each file of scripts' classes by the scripts' numbers, read once a run
/// (`script_classes`).
static SCRIPT_CLASSES: std::sync::LazyLock<std::sync::Mutex<HashMap<std::path::PathBuf, Option<std::sync::Arc<HashMap<i64, String>>>>>> = std::sync::LazyLock::new(Default::default);

/// The classes of the scripts a file lists (a script object begins with its
/// name, which is its class's), by their numbers: read the first time a
/// scene names its scripts from it, and kept, since every scene of the
/// screens names them from the same file (each scene read it anew and
/// searched it script by script: 120 ms a scene).
fn script_classes(folder: &Path, name: &str) -> Option<std::sync::Arc<HashMap<i64, String>>> {
    let path = folder.join(name);
    let mut known = SCRIPT_CLASSES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(classes) = known.get(&path) {
        return classes.clone();
    }
    let read = || -> Option<HashMap<i64, String>> {
        let data = std::fs::read(&path).ok()?;
        let meta = SerializedFile::parse(&data).ok()?;
        let classes = meta
            .objects
            .iter()
            .filter(|o| o.class_id == class::MONO_SCRIPT)
            .filter_map(|o| Some((o.path_id, string(&mut Reader::new(data.get(o.offset..o.offset + o.size)?)).ok()?)))
            .collect();
        Some(classes)
    };
    let classes = read().map(std::sync::Arc::new);
    known.insert(path, classes.clone());
    classes
}

impl Scene {
    /// Reads the scene file of that name (`level49`). With `scripts`, each
    /// object's scripts are named too, which reads the file the scripts
    /// are listed in as well.
    pub fn load(install: &Install, file: &str, scripts: bool) -> Result<Self> {
        let folder = install.root.join("EscapeFromTarkov_Data");
        Self::read(&folder, file, scripts)
    }

    pub(super) fn read(folder: &Path, file: &str, scripts: bool) -> Result<Self> {
        let path = folder.join(file);
        let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let meta = SerializedFile::parse(&data).with_context(|| file.to_owned())?;
        let bytes = |offset: usize, size: usize| data.get(offset..offset + size).context("an object lies outside its file");

        // The objects by their own number, then each transform onto its object.
        let mut nodes: Vec<Node> = Vec::new();
        let mut by_object: HashMap<i64, usize> = HashMap::new();
        let mut components: HashMap<i64, usize> = HashMap::new();
        let mut layers: HashMap<usize, u32> = HashMap::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::GAME_OBJECT) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let count = r.i32()?.max(0) as usize;
            let index = nodes.len();
            for _ in 0..count {
                let component = pointer(&mut r)?;
                if component.file == 0 {
                    components.insert(component.path, index);
                }
            }
            layers.insert(index, r.u32()?);
            let name = string(&mut r)?;
            r.skip(2)?; // tag
            let active = r.u8()? != 0;
            by_object.insert(object.path_id, index);
            nodes.push(Node { name, active, parent: None, children: Vec::new(), layout: None, scripts: Vec::new() });
        }
        let mut of_transform: HashMap<i64, usize> = HashMap::new();
        let mut fathers: Vec<(usize, i64)> = Vec::new();
        let mut lists: Vec<(usize, Vec<i64>)> = Vec::new();
        let mut locals = HashMap::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::TRANSFORM || o.class_id == RECT_TRANSFORM) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let owner = pointer(&mut r)?;
            let Some(&index) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
            let rotation = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
            let turn = z_turn(rotation);
            let position = [r.f32()?, r.f32()?, r.f32()?];
            let whole_scale = [r.f32()?, r.f32()?, r.f32()?];
            let scale = [whole_scale[0], whole_scale[1]];
            locals.insert(index, Local { rotation, position, scale: whole_scale });
            let children = r.i32()?.max(0) as usize;
            let list: Vec<i64> = (0..children).map(|_| pointer(&mut r).map(|child| child.path)).collect::<Result<_>>()?;
            let father = pointer(&mut r)?;
            of_transform.insert(object.path_id, index);
            if father.file == 0 && father.path != 0 {
                fathers.push((index, father.path));
            }
            lists.push((index, list));
            if object.class_id == RECT_TRANSFORM {
                let (anchor_min, anchor_max, offset, size_delta, pivot) = (pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?, pair(&mut r)?);
                nodes[index].layout = Some(Layout { anchor_min, anchor_max, offset, size_delta, pivot, scale, turn });
            }
        }
        for (index, father) in fathers {
            if let Some(&parent) = of_transform.get(&father) {
                nodes[index].parent = Some(parent);
            }
        }
        // Children in the order of their parent's own list, which is the
        // order the screen draws them in (each over the one before).
        for (index, list) in lists {
            nodes[index].children = list.iter().filter_map(|child| of_transform.get(child).copied()).filter(|&child| nodes[child].parent == Some(index)).collect();
        }
        // Every script with the object it is on and where its own fields
        // begin: after the object, the enabled flag, the script and a name.
        let mut behaviours = Vec::new();
        for object in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
            let mut r = Reader::new(bytes(object.offset, object.size)?);
            let owner = pointer(&mut r)?;
            let Some(&node) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
            let Ok(enabled) = r.u32().map(|word| word & 0xff != 0) else { continue };
            if r.skip(12).is_err() || string(&mut r).is_err() {
                continue;
            }
            let read = object.size - r.remaining();
            behaviours.push(Behaviour { id: object.path_id, node, enabled, fields: (object.offset + read, object.size - read) });
        }
        let mut classes = HashMap::new();
        if scripts {
            // A script names its class in a file of its own, listed among
            // the scene's outside files.
            let mut outside: HashMap<i32, Option<std::sync::Arc<HashMap<i64, String>>>> = HashMap::new();
            for object in meta.objects.iter().filter(|o| o.class_id == class::MONO_BEHAVIOUR) {
                let mut r = Reader::new(bytes(object.offset, object.size)?);
                let owner = pointer(&mut r)?;
                r.skip(4)?; // enabled, padded
                let script = pointer(&mut r)?;
                let Some(&index) = by_object.get(&owner.path).filter(|_| owner.file == 0) else { continue };
                let Some(listed) = (script.file > 0).then(|| meta.externals.get(script.file as usize - 1)).flatten() else { continue };
                let named = outside.entry(script.file).or_insert_with(|| script_classes(folder, listed.rsplit('/').next().unwrap_or(listed)));
                let Some(name) = named.as_ref().and_then(|named| named.get(&script.path)) else { continue };
                classes.insert(object.path_id, name.clone());
                nodes[index].scripts.push(name.clone());
            }
        }
        let component_classes = meta.objects.iter().filter(|o| components.contains_key(&o.path_id)).map(|o| (o.path_id, o.class_id)).collect();
        let spans = meta.objects.iter().map(|o| (o.path_id, (o.offset, o.size))).collect();
        Ok(Self { nodes, behaviours, classes, objects: by_object, transforms: of_transform, components, component_classes, locals, spans, layers, externals: meta.externals.clone(), data })
    }

    /// An object's layer (`GameObject.m_Layer`; 0 where not read).
    pub fn layer(&self, node: usize) -> u32 {
        self.layers.get(&node).copied().unwrap_or(0)
    }

    /// Every engine part of that class on the file's objects (its colliders,
    /// 64 and 65), each one's number and its object, in the numbers' order.
    pub fn parts_of_class(&self, class: i32) -> Vec<(i64, usize)> {
        let mut parts: Vec<(i64, usize)> = self.component_classes.iter().filter(|(_, &c)| c == class).filter_map(|(&id, _)| Some((id, *self.components.get(&id)?))).collect();
        parts.sort();
        parts
    }

    /// Whether an object is on and every object above it too
    /// (`GameObject.activeInHierarchy`).
    pub fn active_in_hierarchy(&self, node: usize) -> bool {
        let mut at = Some(node);
        while let Some(n) = at {
            if !self.nodes[n].active {
                return false;
            }
            at = self.nodes[n].parent;
        }
        true
    }

    /// A point in an object's own axes in the world's: scaled, turned and
    /// moved by it and by each object above it (`Transform.TransformPoint`,
    /// the engine's axes).
    pub fn to_world(&self, node: usize, point: [f32; 3]) -> [f32; 3] {
        let mut at = Some(node);
        let mut p = point;
        while let Some(n) = at {
            let local = self.local(n).unwrap_or(Local { rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3], scale: [1.0; 3] });
            let scaled = [p[0] * local.scale[0], p[1] * local.scale[1], p[2] * local.scale[2]];
            let turned = crate::level::level_light::rotate(local.rotation, scaled);
            p = [turned[0] + local.position[0], turned[1] + local.position[1], turned[2] + local.position[2]];
            at = self.nodes[n].parent;
        }
        p
    }

    /// An object's size in the world as the engine gives it
    /// (`Transform.lossyScale`: each scale above it times its own, the
    /// turns between taken as square).
    pub fn lossy_scale(&self, node: usize) -> [f32; 3] {
        let own = self.local(node).map_or([1.0; 3], |local| local.scale);
        match self.nodes[node].parent {
            Some(parent) => {
                let above = self.lossy_scale(parent);
                [own[0] * above[0], own[1] * above[1], own[2] * above[2]]
            }
            None => own,
        }
    }

    /// An object's own transform, where the scene was read from a scene
    /// file.
    pub fn local(&self, node: usize) -> Option<Local> {
        self.locals.get(&node).copied()
    }

    /// An object's turn in the world: its own after each of those above it
    /// (the engine's `Transform.rotation`), in the engine's axes.
    pub fn world_rotation(&self, node: usize) -> [f32; 4] {
        let own = self.local(node).map_or([0.0, 0.0, 0.0, 1.0], |local| local.rotation);
        match self.nodes[node].parent {
            Some(parent) => quaternion_product(self.world_rotation(parent), own),
            None => own,
        }
    }

    /// The object an engine part of that number is on (what a script's
    /// pointer to a `Light` leads to).
    pub fn part_owner(&self, id: i64) -> Option<usize> {
        self.components.get(&id).copied()
    }

    /// An object's place in the world: its own place, scaled and turned by
    /// each of those above it and moved by their places
    /// (`Transform.position`), in the engine's axes.
    pub fn world_position(&self, node: usize) -> [f32; 3] {
        let own = self.local(node).map_or([0.0; 3], |local| local.position);
        match self.nodes[node].parent {
            Some(parent) => {
                let scale = self.local(parent).map_or([1.0; 3], |local| local.scale);
                let scaled = [own[0] * scale[0], own[1] * scale[1], own[2] * scale[2]];
                let turned = crate::level::level_light::rotate(self.world_rotation(parent), scaled);
                let at = self.world_position(parent);
                [at[0] + turned[0], at[1] + turned[1], at[2] + turned[2]]
            }
            None => own,
        }
    }

    /// The bytes of an engine part of that class on an object (a `Light`,
    /// 108), as the file has them.
    /// The number of an object's part of that engine class, which is what
    /// points to it (a level of detail's renderers).
    pub fn component_id(&self, node: usize, class: i32) -> Option<i64> {
        self.components.iter().find(|(id, &at)| at == node && self.component_classes.get(id) == Some(&class)).map(|(&id, _)| id)
    }

    pub fn component(&self, node: usize, class: i32) -> Option<&[u8]> {
        let (&id, _) = self.components.iter().find(|(id, &at)| at == node && self.component_classes.get(id) == Some(&class))?;
        let &(offset, size) = self.spans.get(&id)?;
        self.data.get(offset..offset + size)
    }

    /// The engine classes of an object's parts (its transform, a canvas, a
    /// group, its scripts as 114), with each one's number.
    pub fn parts_of(&self, node: usize) -> Vec<(i64, i32)> {
        let mut parts: Vec<(i64, i32)> = self.components.iter().filter(|(_, &at)| at == node).map(|(&id, _)| (id, self.component_classes.get(&id).copied().unwrap_or(0))).collect();
        parts.sort();
        parts
    }

    /// The class of a script, where the scene was read with its scripts.
    pub fn class_of(&self, behaviour: &Behaviour) -> Option<&str> {
        self.classes.get(&behaviour.id).map(String::as_str)
    }

    /// The script of that class on an object.
    pub fn behaviour(&self, node: usize, class: &str) -> Option<&Behaviour> {
        self.behaviours.iter().find(|behaviour| behaviour.node == node && self.class_of(behaviour) == Some(class))
    }

    /// Every script of that class on an object, in the object's order
    /// (`GetComponents`).
    pub fn behaviours_of<'a>(&'a self, node: usize, class: &'a str) -> impl Iterator<Item = &'a Behaviour> + 'a {
        self.behaviours.iter().filter(move |behaviour| behaviour.node == node && self.class_of(behaviour) == Some(class))
    }

    /// A prefab of one of the build's own files (`resources.assets`) and
    /// all under it, read object by object from its root game object, as a
    /// scene of its own with its scripts named: the file is some 300 MB,
    /// and only what the prefab is made of is read.
    pub fn load_prefab(install: &Install, file: &str, root: i64) -> Result<Self> {
        let mut build = BuildFile::open(install, file)?;
        Self::load_prefab_from(install, &mut build, root)
    }

    /// The same from a file already open.
    pub(super) fn load_prefab_from(install: &Install, build: &mut BuildFile, root: i64) -> Result<Self> {
        let externals = build.meta.externals.clone();
        let mut reader = PrefabReader {
            install: install.clone(),
            build,
            scene: Scene { nodes: Vec::new(), behaviours: Vec::new(), classes: HashMap::new(), objects: HashMap::new(), transforms: HashMap::new(), components: HashMap::new(), component_classes: HashMap::new(), locals: HashMap::new(), spans: HashMap::new(), layers: HashMap::new(), externals, data: Vec::new() },
            outside: HashMap::new(),
        };
        reader.visit(root, None)?;
        Ok(reader.scene)
    }

    /// The prefab a script of one of the build's own files is on (a
    /// template a scene points to by its script): the script's game object
    /// and all under it, as `load_prefab` reads them.
    pub fn load_prefab_of_script(install: &Install, file: &str, script: i64) -> Result<Self> {
        let mut build = BuildFile::open(install, file)?;
        let object = *build.meta.object(script).with_context(|| format!("no object {script} in {file}"))?;
        let bytes = build.bytes(object.offset, object.size)?;
        let owner = pointer(&mut Reader { data: &bytes, pos: 0, big_endian: build.meta.big_endian })?;
        Self::load_prefab(install, file, owner.path)
    }

    /// A script's own fields as the file has them.
    pub fn fields(&self, behaviour: &Behaviour) -> &[u8] {
        &self.data[behaviour.fields.0..behaviour.fields.0 + behaviour.fields.1]
    }

    /// The object the script of that number is on.
    pub fn owner(&self, id: i64) -> Option<usize> {
        self.behaviours.iter().find(|behaviour| behaviour.id == id).map(|behaviour| behaviour.node)
    }

    /// The object that a pointer of the scene's own leads to: the object
    /// itself, or one of its scripts.
    pub fn pointed(&self, id: i64) -> Option<usize> {
        self.objects.get(&id).copied().or_else(|| self.owner(id))
    }

    /// The objects of that name.
    pub fn named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = usize> + 'a {
        self.nodes.iter().enumerate().filter(move |(_, node)| node.name == name).map(|(index, _)| index)
    }

    /// The first object of that name below another, however deep.
    pub fn below(&self, above: usize, name: &str) -> Option<usize> {
        self.nodes[above].children.iter().find_map(|&child| if self.nodes[child].name == name { Some(child) } else { self.below(child, name) })
    }

    /// The path of names from the scene's top down to an object.
    pub fn path(&self, index: usize) -> String {
        match self.nodes[index].parent {
            Some(parent) => format!("{}/{}", self.path(parent), self.nodes[index].name),
            None => self.nodes[index].name.clone(),
        }
    }

    /// An object's rectangle within the rectangle of one it hangs from,
    /// however far above, whose size is given: the engine's layout of a
    /// screen transform, level by level. An object on the way that is not
    /// laid out on a screen, or `above` not being above it, gives none.
    /// Sizes that a layout script would set while the game runs are as
    /// the scene file has them.
    pub fn rect_in(&self, index: usize, above: usize, size: [f32; 2]) -> Option<Rect> {
        if index == above {
            return Some(Rect { at: [0.0, 0.0], size });
        }
        let outer = self.rect_in(self.nodes[index].parent?, above, size)?;
        Some(place(self.nodes[index].layout?, outer))
    }
}
