//! The edit build screen as the install's scene has it (workbench spec,
//! "The edit build screen"): the screen's objects (`EditBuildScreen` of
//! `CommonUIScene.unity`), its preview camera's figures, the height its
//! preview stands at, and the preview's own lights.

use anyhow::{Context, Result};

use crate::install::Install;
use crate::screen::{scene_paths, Scene, ScenePanel};
use crate::unity::reader::Reader;

/// The screen's object in its scene.
const SCREEN: &str = "Common UI/Common UI/EditBuildScreen";
/// The engine's class numbers of a camera and of a light.
const CAMERA: i32 = 20;
const LIGHT: i32 = 108;

/// A light of the preview (`Light`, 2022.3's layout): its kind (0 a spot,
/// 1 directional, 2 a point), whether it is on, its colour (linear as
/// kept), intensity, range, spot angle, and where it stands and how it is
/// turned under the preview's lights' object (which the game sets at the
/// world's middle, unscaled: `WeaponPreview.DisplaySettingsChangedHandler`),
/// the engine's axes; the layers it lights.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewLight {
    pub name: String,
    pub kind: i32,
    pub on: bool,
    pub color: [f32; 4],
    pub intensity: f32,
    pub range: f32,
    pub spot_angle: f32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub culling_mask: u32,
}

/// The screen as its scene has it.
#[derive(Debug, Clone, Default)]
pub struct EditBuildScene {
    pub panel: ScenePanel,
    /// The preview camera's (`Weapon Camera`, the template the preview's
    /// camera is copied from) field of view up and down, near and far
    /// planes (degrees, metres).
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    /// The height the preview's turning parent is put at in the world:
    /// the preview object's own (`WeaponPreview.CreateRotatorParent`).
    pub height: f32,
    pub lights: Vec<PreviewLight>,
}

impl EditBuildScene {
    pub fn load(install: &Install) -> Result<Self> {
        let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/CommonUIScene.unity")).context("the build has no common scene of the interface")?;
        let file = format!("level{index}");
        let scene = Scene::load(install, &file, true)?;
        let top = (0..scene.nodes.len()).find(|&node| scene.path(node) == SCREEN).with_context(|| format!("{file} has no edit build screen"))?;
        let below = |path: &str| (0..scene.nodes.len()).find(|&node| scene.path(node) == format!("{SCREEN}/{path}"));
        let mut panel = ScenePanel::of(&scene, top);
        panel.file = file.clone();
        // The camera's figures by the engine's order of a camera's fields
        // (as `screen::model_camera` reads them).
        let camera = below("Preview Panel/Weapon Camera").context("the screen has no weapon camera")?;
        let bytes = scene.component(camera, CAMERA).context("the weapon camera has no camera")?;
        let mut r = Reader::new(bytes.get(116..).context("a short camera")?);
        let (near, far, fov) = (r.f32()?, r.f32()?, r.f32()?);
        let preview = below("Preview Panel/Weapon Preview").context("the screen has no weapon preview")?;
        let height = scene.local(preview).map_or(0.0, |local| local.position[1]);
        let lights_top = below("Preview Panel/Lights").context("the screen has no preview lights")?;
        let mut lights = Vec::new();
        for node in (0..scene.nodes.len()).filter(|&node| scene.nodes[node].parent == Some(lights_top)) {
            let Some(bytes) = scene.component(node, LIGHT) else { continue };
            let mut light = read_light(bytes).with_context(|| scene.path(node))?;
            light.name = scene.nodes[node].name.clone();
            let local = scene.local(node).unwrap_or(crate::screen::Local { rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3], scale: [1.0; 3] });
            light.on &= scene.nodes[node].active;
            (light.position, light.rotation) = (local.position, local.rotation);
            lights.push(light);
        }
        Ok(Self { panel, fov, near, far, height, lights })
    }
}

/// A light by the engine's order of its fields (`level_light::light`'s):
/// its object, on, kind, shape, colour, intensity, range, the spot's
/// angles, the cookie's size, its shadows' figures, cookie, halo, baking,
/// flare, render mode, and the layers it lights.
fn read_light(bytes: &[u8]) -> Result<PreviewLight> {
    let mut r = Reader::new(bytes);
    r.skip(12)?;
    let on = r.u8()? != 0;
    r.align(4);
    let (kind, _shape) = (r.i32()?, r.i32()?);
    let color = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
    let intensity = r.f32()?;
    let (range, spot_angle) = (r.f32()?, r.f32()?);
    r.skip(4 + 4)?; // inner spot angle, cookie size
    r.skip(4 + 8 + 4)?; // shadows' kind, resolutions, strength
    r.skip(3 * 4 + 64)?; // bias, normal bias, near plane, culling matrix
    r.u8()?;
    r.align(4);
    r.skip(12)?; // cookie
    r.u8()?; // halo
    r.align(4);
    r.skip(4 * 4)?; // baking
    r.u8()?;
    r.align(4);
    r.skip(12)?; // flare
    let _render_mode = r.i32()?;
    let culling_mask = r.u32()?;
    Ok(PreviewLight { name: String::new(), kind, on, color, intensity, range, spot_angle, position: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0], culling_mask })
}

/// The inspect window's preview as the game makes it (inventory spec, "The
/// window's model"): the pool of previews in the preloader's scene
/// (`PreloaderUIScene`, `UIContext/Previews`, `WeaponPreviewPool.
/// _weaponPreviewTemplate`) and its template, a prefab of another file of
/// the build: the camera's figures (`Weapon Camera`: field of view up and
/// down, near and far planes; its place under the template), the lights it
/// switches on as it draws (`CameraLightSwitcher.Lights`), each turned as it
/// stands in the template, and its photographic tonemap's exposure
/// (`Tonemapping`, Standard Assets' image effect, of the type 3), where it
/// has one.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectPreview {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub camera_position: [f32; 3],
    pub lights: Vec<PreviewLight>,
    pub exposure: Option<f32>,
}

impl InspectPreview {
    pub fn load(install: &Install) -> Result<Self> {
        let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader scene")?;
        let file = format!("level{index}");
        let scene = Scene::load(install, &file, true)?;
        let pool = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with("UIContext/Previews")).with_context(|| format!("{file} has no pool of previews"))?;
        let fields = scene.fields(scene.behaviour(pool, "WeaponPreviewPool").context("the pool has no WeaponPreviewPool")?);
        let mut r = Reader::new(fields);
        let (template_file, template) = (r.i32()?, r.i64()?);
        let template_file = match template_file {
            0 => file.clone(),
            n => scene.externals.get(n as usize - 1).map(|name| name.rsplit('/').next().unwrap_or(name).to_owned()).context("the template's file")?,
        };
        let prefab = Scene::load_prefab_of_script(install, &template_file, template)?;
        let camera = (0..prefab.nodes.len()).find(|&node| prefab.component(node, CAMERA).is_some()).context("the template has no camera")?;
        let bytes = prefab.component(camera, CAMERA).context("the camera's fields")?;
        let mut r = Reader::new(bytes.get(116..).context("a short camera")?);
        let (near, far, fov) = (r.f32()?, r.f32()?, r.f32()?);
        let camera_position = prefab.world_position(camera);
        // The lights the camera's switcher puts on as it draws.
        let listed: Vec<i64> = match prefab.behaviour(camera, "CameraLightSwitcher") {
            Some(switcher) => {
                let mut r = Reader::new(prefab.fields(switcher));
                let count = r.u32()? as usize;
                (0..count).map(|_| -> Result<i64> { r.i32()?; Ok(r.i64()?) }).collect::<Result<_>>()?
            }
            None => Vec::new(),
        };
        let mut lights = Vec::new();
        for node in 0..prefab.nodes.len() {
            let (Some(id), Some(bytes)) = (prefab.component_id(node, LIGHT), prefab.component(node, LIGHT)) else { continue };
            if !listed.contains(&id) {
                continue;
            }
            let mut light = read_light(bytes).with_context(|| prefab.path(node))?;
            light.name = prefab.nodes[node].name.clone();
            (light.position, light.rotation) = (prefab.world_position(node), prefab.world_rotation(node));
            lights.push(light);
        }
        // Standard Assets' `Tonemapping`: its type, the adaptive texture's
        // size, the remapping curve (its keys, 28 bytes each, and three
        // more words), then the exposure's adjustment.
        let exposure = prefab.behaviour(camera, "Tonemapping").filter(|tonemap| tonemap.enabled).and_then(|tonemap| {
            let mut r = Reader::new(prefab.fields(tonemap));
            let kind = r.i32().ok()?;
            r.i32().ok()?;
            let keys = r.u32().ok()? as usize;
            r.skip(keys * 28 + 12).ok()?;
            let exposure = r.f32().ok()?;
            (kind == 3 && exposure > 0.0 && exposure < 20.0).then_some(exposure)
        });
        Ok(Self { fov, near, far, camera_position, lights, exposure })
    }
}
