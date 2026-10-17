//! Unity's on-disk formats, as written by the Unity 2022.3 build Tarkov ships.

pub mod anim;
pub mod assets;
pub mod bundle;
pub mod extract;
pub mod reader;
pub mod serialized;
pub mod texture;
pub mod typetree;
pub mod value;

pub use assets::{Assets, ObjRef, ResourcePlace};
pub use bundle::Bundle;
pub use serialized::SerializedFile;
pub use value::{PPtr, Value};

/// Unity class ids used by this project.
pub mod class {
    pub const GAME_OBJECT: i32 = 1;
    pub const TRANSFORM: i32 = 4;
    pub const MATERIAL: i32 = 21;
    pub const MESH_RENDERER: i32 = 23;
    pub const TEXTURE_2D: i32 = 28;
    pub const MESH_FILTER: i32 = 33;
    pub const MESH: i32 = 43;
    pub const SHADER: i32 = 48;
    pub const ANIMATION_CLIP: i32 = 74;
    pub const AVATAR: i32 = 90;
    pub const ANIMATOR_CONTROLLER: i32 = 91;
    pub const ANIMATOR: i32 = 95;
    pub const LIGHT: i32 = 108;
    pub const MONO_BEHAVIOUR: i32 = 114;
    pub const MONO_SCRIPT: i32 = 115;
    pub const SKINNED_MESH_RENDERER: i32 = 137;
    pub const ASSET_BUNDLE: i32 = 142;
    pub const LOD_GROUP: i32 = 205;
    pub const ANIMATOR_OVERRIDE_CONTROLLER: i32 = 221;

    pub fn name(id: i32) -> &'static str {
        match id {
            GAME_OBJECT => "GameObject",
            TRANSFORM => "Transform",
            MATERIAL => "Material",
            MESH_RENDERER => "MeshRenderer",
            TEXTURE_2D => "Texture2D",
            MESH_FILTER => "MeshFilter",
            MESH => "Mesh",
            SHADER => "Shader",
            ANIMATION_CLIP => "AnimationClip",
            AVATAR => "Avatar",
            ANIMATOR_CONTROLLER => "AnimatorController",
            ANIMATOR => "Animator",
            MONO_BEHAVIOUR => "MonoBehaviour",
            MONO_SCRIPT => "MonoScript",
            SKINNED_MESH_RENDERER => "SkinnedMeshRenderer",
            ASSET_BUNDLE => "AssetBundle",
            ANIMATOR_OVERRIDE_CONTROLLER => "AnimatorOverrideController",
            65 => "BoxCollider",
            82 => "AudioSource",
            83 => "AudioClip",
            108 => "Light",
            135 => "SphereCollider",
            136 => "CapsuleCollider",
            198 => "ParticleSystem",
            199 => "ParticleSystemRenderer",
            205 => "LODGroup",
            _ => "",
        }
    }
}
