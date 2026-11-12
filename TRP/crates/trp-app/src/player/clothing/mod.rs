//! The clothing chosen on the OVERALL tab, every side's suits for any character, each loaded when chosen.
//! After the inventory spec, "The OVERALL tab"; the switch `CLOTHING`.
//!
//! Every suit of the install's customization list is offered, upper and
//! lower, whatever its side (`trp_data::suits`). **A suit is read when it is
//! first chosen** (lazy loading): its parts' bundles, meshes and pictures
//! made ready on another thread (`PreparedLooks`, as a weapon's rig is), put
//! into the engine's stores on the frame they come, and kept for a later
//! choice of the same suit. Then it is put on as the game puts a suit on
//! (`CustomizationSuite.SetClothingsToProfile`, the model drawn again): an
//! upper suit's body in place of the body's own top, on the body, the gear
//! page's and the OVERALL tab's model and the corpse (not on the range's
//! dummies, which wear their own: `dummy`), and its hands in
//! place of the hands the character's own eyes see; a lower suit's legs in
//! place of the legs. A torso of the newer kind has its three meshes, of
//! which the one for what is worn is drawn (`body_view::show_torso`). The
//! bones every suit hangs on are in the skeleton from the start
//! (`body_view::SUIT_BONES`). The choice is kept in the options' file for the
//! next run, which puts it on as it begins.

use std::collections::{HashMap, HashSet};

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future::poll_once, AsyncComputeTaskPool, Task};
use trp_data::character::arms::Torso;
use trp_data::character::suits::Suit;
use trp_data::unity::anim::path_hash;
use trp_data::unity::Assets as UnityAssets;

use crate::player::body_view::{BodyParts, BodyRig, LowerBody, OwnBody, SuitPiece, SuitRole, TorsoPiece, UpperBody, SEEN};
use crate::assets::render_data::PreparedLooks;
use crate::weapons::weapon::{FirstPersonHands, WeaponRig};

mod prepare_keep;
mod wearing;

pub use prepare_keep::*;
use prepare_keep::{Loaded, LoadedPart, Prepared, into_stores, keep, prepare, slot};
use wearing::{wear};

/// Where the choice is kept in the options' file (`options_view`'s, which
/// keeps it as it is when it writes the rest).
pub const KEPT_AS: &str = "clothing";

/// The name a suit is waited for under on the graphics card, before its
/// place in the list (`weapon::UploadWatch`).
const SUIT_WAITED: &str = "a suit, the list's ";

pub struct ClothingPlugin;

impl Plugin for ClothingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, wear.before(crate::player::body_view::show_outfit));
    }
}

/// The suits, which are worn, which are asked for, and those read.
#[derive(Resource)]
pub struct Clothing {
    pub suits: Vec<Suit>,
    /// The upper suit and the lower worn, by their places in `suits`.
    pub worn: [Option<usize>; 2],
    /// Those asked for on the OVERALL tab, put on once read.
    pub wanted: [Option<usize>; 2],
    /// The bundle of the hands an upper suit last put on; none while the
    /// run's own are worn (`Options::arms`). The wrist watch goes by it
    /// (`watch_view`).
    pub hands: Option<String>,
    /// The bits the upper suit and the lower put on add to the head
    /// things' fittings (`trp_data::dress::part_clipping`; none until one
    /// is put on here: the body's own then), read only with `HEAD FITTING`
    /// on (`fitting`, as the run began).
    pub bits: [Option<u8>; 2],
    /// The holster of the legs a lower suit put on here carries (`LegsView`;
    /// none: they carry none, and no pistol is hung), which the weapons on
    /// the body go by (`slung_view::hang`) in place of the legs the run
    /// began in; not yet one, while no lower suit has been put on.
    pub holster: Option<Option<trp_data::character::arms::LegsHolster>>,
    fitting: bool,
    loaded: HashMap<usize, Loaded>,
    /// Those read whose meshes and pictures are all on the graphics card:
    /// a suit is put on only then, the one it takes the place of drawn
    /// until it is (`weapon::UploadWatch`).
    on_card: HashSet<usize>,
    /// Those that could not be read, not asked for again.
    pub failed: Vec<usize>,
    task: Option<(usize, Task<(Option<UnityAssets>, anyhow::Result<Prepared>)>)>,
    /// The thread's reader of the install, kept while suits are asked for.
    worker: Option<UnityAssets>,
    install: trp_data::Install,
    /// The options' file, where the choice is kept; none in a run nobody
    /// is at the keys of.
    file: Option<std::path::PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::{keep, kept, KEPT_AS};

    /// The choice kept in the options' file and read back, the rest of the
    /// file as it was.
    #[test]
    fn the_choice_is_kept_beside_the_options() {
        let file = std::env::temp_dir().join(format!("trp_clothing_{}.json", std::process::id()));
        std::fs::write(&file, r#"{ "graphics": { "vsync": true } }"#).expect("written");
        keep(&file, [Some("5d1f609d86f7744bca3f1689"), None]);
        assert_eq!(kept(&file), [Some("5d1f609d86f7744bca3f1689".to_owned()), None]);
        let all: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("json");
        assert_eq!(all["graphics"]["vsync"], serde_json::Value::Bool(true));
        assert!(all.get(KEPT_AS).is_some());
        let _ = std::fs::remove_file(&file);
        assert_eq!(kept(&file), [None, None]);
    }
}
