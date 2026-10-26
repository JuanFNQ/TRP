//! A grenade's prefab beyond the hands (`docs/spec/grenades-and-knife.md`,
//! "The grenade's hands", "The throw"): the parts the hand lets go of in a
//! throw (`GrenadePrefab.ThrowingParts`, the first being the grenade in the
//! hand, which a throw starts from), and the grenade that is thrown
//! (`GrenadeItself`, a `GrenadeSettings` on an object of its own in the same
//! bundle): its tree, what it draws, its colliders, its rigid body, and the
//! offset from the hand's grenade it leaves at (`Offset`).

use anyhow::{anyhow, Context, Result};

use crate::unity::assets::{Assets, ObjRef};
use crate::unity::extract::{read_hierarchy, rigidbody_of, Hierarchy, RigidbodyData};
use crate::unity::value::Value;
use crate::weapon::Weapon;

#[derive(Debug, Clone)]
pub struct GrenadeData {
    /// The names of the objects let go of in a throw, the grenade in the
    /// hand first.
    pub throwing_parts: Vec<String>,
    /// The thrown grenade's tree (node 0 its object), with its colliders.
    pub thrown: Hierarchy,
    /// Its rigid body.
    pub body: RigidbodyData,
    /// Where it leaves the hand, in the hand's grenade's axes.
    pub offset: [f32; 3],
    /// Its collision sound's kind (`CollisionSounds`: 0 a frag grenade's).
    pub collision_sound: i64,
}

impl GrenadeData {
    /// Reads them from a grenade's rig (`weapon::assemble_prefab` of its
    /// item's prefab, whose script is the `GrenadePrefab`).
    pub fn read(assets: &Assets, rig: &Weapon) -> Result<Self> {
        let prefab = &rig.prefab;
        let throwing_parts: Vec<String> = prefab
            .req("ThrowingParts")?
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        anyhow::ensure!(!throwing_parts.is_empty(), "{} names no grenade to throw from", rig.name);
        let settings = prefab
            .req("GrenadeItself")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(rig.prefab_file, p))
            .ok_or_else(|| anyhow!("{} has no grenade of its own to throw", rig.name))?;
        let settings_value = assets.read(settings)?;
        let object: ObjRef = settings_value
            .req("m_GameObject")?
            .as_pptr()
            .and_then(|p| assets.resolve_loaded(settings.file, p))
            .ok_or_else(|| anyhow!("its grenade's settings have no object"))?;
        let offset = settings_value.req("Offset")?.as_vec3().context("Offset is not a vector")?;
        let collision_sound = settings_value.get("CollisionSound").and_then(Value::as_i64).unwrap_or(0);
        let thrown = read_hierarchy(assets, object)?;
        let body = rigidbody_of(assets, object)?.ok_or_else(|| anyhow!("its thrown grenade has no rigid body"))?;
        Ok(Self { throwing_parts, thrown, body, offset, collision_sound })
    }
}
