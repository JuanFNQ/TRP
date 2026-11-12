//! Turning extracted Unity objects into Bevy meshes, images and materials.
//!
//! The materials are an approximation: Tarkov's weapon shader is a
//! specular/gloss model and Bevy's is metallic/roughness.

use std::collections::HashMap;

use anyhow::Result;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_resource::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};
use trp_data::unity::extract::{read_material, read_mesh, read_texture_shared, MeshData, TextureData};
use trp_data::unity::texture::{self, format};
use trp_data::unity::{Assets as UnityAssets, ObjRef};

use crate::platform::convert;

mod converted;
mod prepared_looks;
mod pictures_added;
mod mesh_build;

pub use converted::*;
pub use prepared_looks::*;
pub use pictures_added::*;
pub use mesh_build::*;
use converted::{CLOTH, stalker_picture};
use mesh_build::{build_mesh, image, picture_image, picture_of};
