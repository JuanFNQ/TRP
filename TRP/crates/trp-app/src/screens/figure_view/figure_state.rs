//! The figure's twin, images, OVERALL figure, held weapon and shots.

use super::*;

/// A piece of the figure drawn as a piece of the body is: shown while that
/// one is.
#[derive(Component)]
pub struct Twin(pub Entity);

/// The picture the figure is drawn onto, for the gear page.
#[derive(Resource, Default)]
pub struct FigureImage(pub Option<Handle<Image>>);

/// The OVERALL tab's view of the figure (`PlayerModelView`): the picture it
/// is drawn onto; what the page says of it: the view's size in the screen's
/// units, the scene's camera, and the model's turn about the vertical in
/// degrees, the engine's (`XCoordRotation.CurrentYaw`: 180 as the model is
/// put in, `PlayerModelLoader.CreatePlayerBody`).
#[derive(Resource)]
pub struct OverallFigure {
    pub image: Option<Handle<Image>>,
    pub rect: Option<[f32; 2]>,
    pub camera: Option<trp_data::interface::screen::ModelCamera>,
    pub yaw: f32,
    /// The model made anew, asked by the page as it shows
    /// (`PlayerModelView.Show`), and whether that drawing is still to come
    /// or under way (the page's spinner shows meanwhile).
    pub remake: bool,
    pub making: bool,
}

impl Default for OverallFigure {
    fn default() -> Self {
        Self { image: None, rect: None, camera: None, yaw: YAW, remake: false, making: false }
    }
}

/// The model's own turn as it is put in, in degrees about the vertical
/// (`TURN`).
pub const YAW: f32 = 180.0;

/// The figure as built: its two cameras' drawings, what it is drawn with
/// (to wait for it on the graphics card), its skeleton, and where its
/// drawing stands.
#[derive(Resource)]
pub struct Figure {
    /// The gear page's, and the OVERALL tab's.
    pub(super) shots: [Shot; 2],
    /// By id alone: the entities drawn with them hold them. A handle kept
    /// here would keep every model ever hung on the figure on the graphics
    /// card after it was taken off.
    pub(super) meshes: Vec<AssetId<Mesh>>,
    pub(super) materials: Vec<AssetId<StandardMaterial>>,
    /// Asked to wait for its meshes and pictures, and they are there.
    pub(super) waiting: bool,
    pub(super) on_card: bool,
    /// The drawing under way: which, and the frames its camera has been on.
    pub(super) drawing: Option<(usize, u32)>,
    /// What later pieces are put on (a suit chosen, `clothing`).
    pub skeleton: Skeleton,
    /// The OVERALL tab's own figure, with `OVERALL FIGURE` on: later pieces
    /// go on its skeleton too (`skeletons`).
    pub overall: Option<Overall>,
}

/// The OVERALL tab's own figure (`OVERALL FIGURE`): its skeleton (the menu
/// player's bones, `trp_sim::figure::FigureData::paths`, each an entity in
/// that order), its outfits' skeletons, the game's menu figure as made at
/// this showing and whether its clock runs (from the frame its picture is
/// first shown), the weapon in hand's model on it (its tree's entities, in
/// its rig's order), and its camera's frames.
pub struct Overall {
    pub skeleton: Skeleton,
    pub(super) data: trp_sim::characters::figure::FigureData,
    pub(super) bones: Vec<Entity>,
    pub(super) ribcage: Option<Entity>,
    pub(super) outfits: Vec<crate::player::death::CorpseOutfit>,
    /// What it is to hold at this showing (none: nothing, or nothing it can
    /// be made with), and whether it is made.
    pub(super) wanted: Option<Held>,
    pub(super) made: bool,
    pub(super) running: Option<trp_sim::characters::figure::Figure>,
    pub(super) clock: bool,
    pub(super) weapon: Option<Vec<Entity>>,
    /// The weapon's tactical devices (`tactical_view` shows them), and its
    /// chambered rounds waiting for their models: each chamber's object and
    /// the round's template.
    pub(super) devices: Vec<trp_data::weapons::tactical::Device>,
    pub(super) chambered: Vec<(Entity, String)>,
    /// Frames its camera has drawn this showing (the spinner goes after the
    /// first drawing is done); its first drawing as the run begins, which
    /// builds its pipelines, done.
    pub(super) frames: u32,
    pub(super) warmed: bool,
    pub(super) showings: u32,
    /// The item in hand as it was made at this showing (`in_hand`).
    pub(super) held_item: Option<trp_sim::items::inventory::Id>,
    /// Bones kept at a place of their own over what the figure gives them
    /// (the pistol's holster bone by the legs worn, `slung_view`).
    pub(super) fixed: Vec<(usize, Transform)>,
    /// What `--figure-hold` reports the places of (the head, the palms,
    /// the muzzle), and the frames it has been held.
    pub(super) landmarks: Vec<(&'static str, Entity)>,
    pub(super) held_frames: u32,
}

impl Overall {
    /// The item its hands hold, which its hung weapons leave off: as made
    /// at its showing (made anew at each), or the hands' now before the
    /// first.
    pub fn in_hand(&self, now: Option<trp_sim::items::inventory::Id>) -> Option<trp_sim::items::inventory::Id> {
        if self.showings > 0 { self.held_item } else { now }
    }

    /// The weapon in its hands as made at this showing: its model's objects
    /// (in its rig's order), its tactical devices, and its item.
    pub fn held_weapon(&self) -> Option<(&[Entity], &[trp_data::weapons::tactical::Device], Option<trp_sim::items::inventory::Id>)> {
        self.weapon.as_deref().map(|tree| (tree, self.devices.as_slice(), self.held_item))
    }

    /// A bone kept at that place in its parent (the pistol's holster bone,
    /// `PlayerBones.AdjustPistolHolster`), whatever the figure's pose.
    pub fn fix_bone(&mut self, path: &str, place: Transform) {
        let Some(bone) = self.data.paths().iter().position(|known| known.ends_with(path)) else { return };
        self.fixed.retain(|(at, _)| *at != bone);
        self.fixed.push((bone, place));
    }
}

/// What the figure holds at a showing (`PlayerModelLoader.GetItemInHands`):
/// the item, the rig's key and how it is read, the kind the body is told,
/// and what the item tells the weapon's animator (its chambered rounds, its
/// stock folded); and whether a model is made for its hands at all.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Held {
    pub(super) item: trp_sim::items::inventory::Id,
    pub(super) key: String,
    pub(super) prefab: Option<String>,
    pub(super) kind: i32,
    pub(super) chamber: u32,
    pub(super) folded: bool,
    /// The game makes the item's own prefab, animated, and holds it only
    /// where that is a weapon's prefab (`CreateWeapon`: `CreateItem(..,
    /// isAnimated: true).GetComponent<WeaponPrefab>()`): a firearm, a
    /// thrown thing, a knife, the rangefinder, a radio transmitter
    /// (`ObjectsFactory.CreateItemAsync`'s list of those with a weapon's
    /// prefab). Medicine, food and drink are made as their loot models,
    /// which are none: the body is still told the item's kind (0), and its
    /// hands hold nothing, unsolved.
    pub(super) model: bool,
}

/// One of the figure's drawings: its camera (the OVERALL tab's made when it
/// is first wanted), whether something changed since it was last drawn, its
/// picture's size in pixels, the turn the model was last drawn at, and
/// whether its page has been shown yet.
pub(super) struct Shot {
    pub(super) camera: Option<Entity>,
    pub(super) wanted: bool,
    pub(super) size: UVec2,
    pub(super) yaw: f32,
    pub(super) shown: bool,
}

pub(super) const GEAR: usize = 0;

pub(super) const OVERALL: usize = 1;

impl Figure {
    /// Asks for a drawing: something of it has moved.
    pub fn ask(&mut self) {
        for shot in &mut self.shots {
            shot.wanted = true;
        }
    }

    /// New meshes and materials of it (a suit put on): drawn once they are
    /// on the graphics card as well.
    pub fn wait_for(&mut self, meshes: &[Handle<Mesh>], materials: &[Handle<StandardMaterial>]) {
        for mesh in meshes {
            if !self.meshes.contains(&mesh.id()) {
                self.meshes.push(mesh.id());
            }
        }
        for material in materials {
            if !self.materials.contains(&material.id()) {
                self.materials.push(material.id());
            }
        }
        (self.waiting, self.on_card) = (false, false);
        self.ask();
    }

    /// The gear page's skeleton and the OVERALL tab's own (`OVERALL
    /// FIGURE`), for pieces put on later on both.
    pub fn skeletons(&mut self) -> (&mut Skeleton, Option<&mut Skeleton>) {
        (&mut self.skeleton, self.overall.as_mut().map(|overall| &mut overall.skeleton))
    }

    /// The OVERALL tab's own figure's skeleton, where it has one.
    pub fn overall_skeleton(&self) -> Option<&Skeleton> {
        self.overall.as_ref().map(|overall| &overall.skeleton)
    }

    /// The OVERALL tab's own figure's root, the menu player's prefab's root,
    /// which its page's drag turns (what hangs on it turns with it: the
    /// shadow at its feet, `boot_shadow`).
    pub fn overall_root(&self) -> Option<Entity> {
        self.overall.as_ref().map(|overall| overall.skeleton.root)
    }
}
