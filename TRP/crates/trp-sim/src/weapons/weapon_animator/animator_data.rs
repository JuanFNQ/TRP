//! The weapon animator's data and parts loaded from the install.

use super::*;

/// A weapon's controller, read once.
#[derive(Clone)]
pub struct WeaponAnimatorData {
    pub(super) controller: Arc<Controller>,
    pub(super) marks: Arc<MarkTable>,
    pub(super) hands: usize,
    /// For a weapon of one's own on another's rig: the edits made to
    /// every pose (`trp_data::custom::PoseEdits`).
    pub(super) edits: Option<Arc<PoseEdits>>,
    /// The animators of parts of the build that have their own (a bipod's:
    /// its legs, fed `Bipod`). The game runs them as one with the weapon's
    /// (`WeaponPrefab.UpdateAnimatorHierarchy`): a parameter set on it is
    /// set on them, they move with it, and their marks reach the same
    /// listeners.
    pub(super) parts: Vec<PartData>,
}

/// A part's own animator, read once: its controller, its marks, and for
/// each of its bones the weapon's bone it is.
#[derive(Clone)]
pub(super) struct PartData {
    pub(super) controller: Arc<Controller>,
    pub(super) marks: Arc<MarkTable>,
    nodes: Arc<Vec<usize>>,
}

impl PartData {
    /// The part at `node` of the weapon's tree with the controller given:
    /// its bones the node and those below it, named by their paths below it
    /// as its clips name them.
    fn load(assets: &Assets, weapon: &Weapon, node: usize, controller: trp_data::unity::ObjRef) -> Result<Self> {
        let table = StateTable::from_controller(assets, controller, &std::collections::HashMap::new())?;
        let tree = &weapon.hierarchy;
        let root = tree.path(node);
        let mut bones = Vec::new();
        let mut nodes = Vec::new();
        for index in 0..tree.nodes.len() {
            let path = tree.path(index);
            let below = if index == node {
                String::new()
            } else if let Some(rest) = path.strip_prefix(&root).and_then(|rest| rest.strip_prefix('/')) {
                rest.to_owned()
            } else {
                continue;
            };
            bones.push(BoneRest { path: below, position: tree.nodes[index].translation, rotation: tree.nodes[index].rotation, scale: None });
            nodes.push(index);
        }
        let controller = Controller::load(assets, &table, bones, &|_| true)?;
        let marks = MarkTable::new(&table, &controller);
        Ok(Self { controller: Arc::new(controller), marks: Arc::new(marks), nodes: Arc::new(nodes) })
    }
}

/// A part's animator running.
#[derive(Clone)]
pub(super) struct Part {
    pub(super) animator: Animator,
    pub(super) marks: Marks,
    nodes: Arc<Vec<usize>>,
}

impl Part {
    pub(super) fn new(data: &PartData) -> Self {
        Self { animator: Animator::new(data.controller.clone()), marks: Marks::new(data.marks.clone()), nodes: data.nodes.clone() }
    }

    /// Its bones' pose written over the weapon's.
    pub(super) fn put(&self, own: &[([f32; 3], [f32; 4])], pose: &mut [([f32; 3], [f32; 4])]) {
        for (bone, &node) in own.iter().zip(self.nodes.iter()) {
            if let Some(place) = pose.get_mut(node) {
                *place = *bone;
            }
        }
    }
}

impl WeaponAnimatorData {
    /// Reads the controller of a weapon, or of a prefab like one, with
    /// every clip of every state. The bones posed are the rig's own, in
    /// the order of its hierarchy.
    pub fn load(assets: &Assets, weapon: &Weapon) -> Result<Self> {
        Self::load_with(assets, weapon, false)
    }

    /// As `load`, keeping the clips' scale of the bones too: for an item's
    /// rig, whose item is drawn by its bone scaled up from nought. (A
    /// weapon's clips scale nearly every bone, by the look of it to what it
    /// rests at; they are not taken.)
    pub fn load_scaled(assets: &Assets, weapon: &Weapon) -> Result<Self> {
        Self::load_with(assets, weapon, true)
    }

    fn load_with(assets: &Assets, weapon: &Weapon, scaled: bool) -> Result<Self> {
        let table = StateTable::load(assets, weapon)?;
        let tree = &weapon.hierarchy;
        let mut bones: Vec<BoneRest> = (0..tree.nodes.len())
            .map(|index| BoneRest {
                path: tree.path(index),
                position: tree.nodes[index].translation,
                rotation: tree.nodes[index].rotation,
                scale: scaled.then_some(tree.nodes[index].scale),
            })
            .collect();
        // A weapon of one's own has the bones its edits move resting
        // where the edits have them; the animator is the donor's and
        // takes the donor's own rest, and its pose is edited afterwards.
        let edits = weapon.custom.as_ref().map(|custom| custom.edits.clone());
        for (bone, rest) in edits.iter().flat_map(|edits| edits.donor_rest()) {
            bones[*bone].position = *rest;
        }
        let controller = Controller::load(assets, &table, bones, &|_| true)?;
        let hands = controller.layer(HANDS_LAYER).ok_or_else(|| anyhow!("{} has no layer named {HANDS_LAYER}", weapon.name))?;
        let marks = MarkTable::new(&table, &controller);
        // The bipod's own animator, where the build has one (its legs and
        // their marks; mounting spec, "The bipod"). One that cannot be read
        // leaves the bipod as its prefab has it.
        let parts = weapon
            .bipod
            .as_ref()
            .and_then(|bipod| Some((bipod.node, bipod.animator?)))
            .and_then(|(node, controller)| match PartData::load(assets, weapon, node, controller) {
                Ok(part) => Some(part),
                Err(e) => {
                    eprintln!("the bipod's animator of {} could not be read: {e:#}", weapon.name);
                    None
                }
            })
            .into_iter()
            .collect();
        Ok(Self { controller: Arc::new(controller), marks: Arc::new(marks), hands, edits, parts })
    }

    /// The controller of a part of the build that is a weapon of its own in the
    /// model (an underbarrel launcher: its `WeaponPrefab`'s controller,
    /// underbarrel-launcher spec, "Two animators"), as a weapon's: its bones the
    /// part's node and those below it, named by their paths below it as its
    /// clips name them; and for each bone of its pose, the weapon's node it is.
    pub fn load_part(assets: &Assets, weapon: &Weapon, node: usize, controller: trp_data::unity::ObjRef) -> Result<(Self, Vec<usize>)> {
        let table = StateTable::from_controller(assets, controller, &std::collections::HashMap::new())?;
        let tree = &weapon.hierarchy;
        let root = tree.path(node);
        let mut bones = Vec::new();
        let mut nodes = Vec::new();
        for index in 0..tree.nodes.len() {
            let path = tree.path(index);
            let below = if index == node {
                String::new()
            } else if let Some(rest) = path.strip_prefix(&root).and_then(|rest| rest.strip_prefix('/')) {
                rest.to_owned()
            } else {
                continue;
            };
            bones.push(BoneRest { path: below, position: tree.nodes[index].translation, rotation: tree.nodes[index].rotation, scale: None });
            nodes.push(index);
        }
        let controller = Controller::load(assets, &table, bones, &|_| true)?;
        let hands = controller.layer(HANDS_LAYER).ok_or_else(|| anyhow!("the part's controller has no layer named {HANDS_LAYER}"))?;
        let marks = MarkTable::new(&table, &controller);
        Ok((Self { controller: Arc::new(controller), marks: Arc::new(marks), hands, edits: None, parts: Vec::new() }, nodes))
    }

    /// A copy whose controllers work out at each update only the bones the
    /// host follows (`followed`, by the rig's bones' places; their parts'
    /// bones by the weapon's nodes they are) and those a layer leaves as they
    /// stood on that update, the rest made from each update's mix when asked
    /// (`Controller::follow`, `WeaponAnimator::pose_of`). For a character
    /// posed only as it is seen or met (a puppet's weapon: `puppet::
    /// followed_rig_bones`); the player's own animators work out every bone.
    pub fn following(&self, followed: &[bool]) -> Result<Self> {
        let mut controller = (*self.controller).clone();
        controller.follow(followed)?;
        let parts = self
            .parts
            .iter()
            .map(|part| {
                let mut controller = (*part.controller).clone();
                let own: Vec<bool> = part.nodes.iter().map(|&node| followed.get(node).copied().unwrap_or(true)).collect();
                controller.follow(&own)?;
                Ok(PartData { controller: Arc::new(controller), ..part.clone() })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { controller: Arc::new(controller), parts, ..self.clone() })
    }

    /// The weapon's own controller.
    pub fn controller(&self) -> &Arc<Controller> {
        &self.controller
    }

    /// A controller written out by hand with its marks, for trying the
    /// logic where there is no install.
    pub fn by_hand(controller: Controller, marks: MarkTable) -> Result<Self> {
        let hands = controller.layer(HANDS_LAYER).ok_or_else(|| anyhow!("no layer named {HANDS_LAYER}"))?;
        Ok(Self { controller: Arc::new(controller), marks: Arc::new(marks), hands, edits: None, parts: Vec::new() })
    }

    /// One of the rig's bones by its path below the rig's root, as its
    /// place in the animator's pose.
    pub fn bone(&self, path: &str) -> Option<usize> {
        self.controller.bones.iter().position(|bone| bone.path == path)
    }

    /// Whether the hands' layer has a state of this full name.
    pub fn has_state(&self, state: &str) -> bool {
        self.controller.state(self.hands, state).is_some()
    }

    /// The full names of the hands' states.
    pub fn state_names(&self) -> Vec<String> {
        let machine = self.controller.layers[self.hands].machine;
        self.controller.machines[machine].states.iter().map(|state| state.name.clone()).collect()
    }

    /// How many clips were read for it.
    pub fn clip_count(&self) -> usize {
        self.controller.clips.len()
    }

    /// The paths of the bones some clip scales.
    pub fn scaled_bones(&self) -> Vec<&str> {
        self.controller.scaled.iter().map(|bone| self.controller.bones[*bone].path.as_str()).collect()
    }

    /// The marks of one of the hands' states, by its full name, each with
    /// the fraction of the state it sits at.
    pub fn marks_of(&self, state: &str) -> Vec<(String, f32)> {
        let machine = self.controller.layers[self.hands].machine;
        let Some(index) = self.controller.state(self.hands, state) else { return Vec::new() };
        self.marks.of(machine, index).map(|(mark, time)| (mark.name.clone(), time)).collect()
    }
}
