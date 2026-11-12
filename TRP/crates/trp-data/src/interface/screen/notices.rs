//! The version label, notices, named sprites and the malfunction glow.

use super::*;

/// The build's scenes by their paths in the game's project, in order: the
/// scene at place `n` is the file `level<n>`.
/// The line at the screen's foot that says the game's version
/// (`PreloaderUI._alphaVersionLabel`, `Preloader UI/BottomPanel/Content/
/// UpperPart/AlphaLabel` in the scene whose path ends in
/// `UI/PreloaderUIScene.unity`), which shows over every screen: its text's
/// look, and how high it lies over the screen's foot (its column's padding
/// below it, `Content`'s `VerticalLayoutGroup`).
pub fn version_label(install: &Install) -> Result<(ScenePanel, f32)> {
    const LABEL: &str = "/BottomPanel/Content/UpperPart/AlphaLabel";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let label = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(LABEL)).with_context(|| format!("{file} has no version label"))?;
    let below = scene.nodes[label]
        .parent
        .and_then(|upper| scene.nodes[upper].parent)
        .and_then(|content| scene.behaviour(content, "VerticalLayoutGroup"))
        .and_then(|group| row_group(scene.fields(group)))
        .map_or(0.0, |group| group.padding[3]);
    Ok((ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, label) }, below))
}

/// A notice as the game's notifier shows it in a raid (`NotifierView`'s
/// default template: `Preloader UI/BottomPanel/Content/UpperPart/Notifier/
/// Content/Notification` in the preloader's scene): the template with all
/// under it (its `Content` box, the box's `Text` and `Icon`), the box's
/// row padding (its `HorizontalLayoutGroup`: left, right, top, bottom), the
/// row's own padding below it and the bottom panel's below that (their
/// `VerticalLayoutGroup`s); and the alert's icon (`notification_icon_alert_red`,
/// which `ENotificationIconType.Alert` names in `StaticIcons.NotificationSprites`).
#[derive(Debug, Clone)]
pub struct NoticeLook {
    pub panel: ScenePanel,
    pub content: usize,
    pub text: usize,
    pub icon: usize,
    pub padding: [f32; 4],
    pub below: f32,
    pub alert: Option<(String, i64)>,
    /// The plain notice's icon (`ENotificationIconType.Default`:
    /// `notification_icon_alert_green`).
    pub plain: Option<(String, i64)>,
}

pub fn notice(install: &Install) -> Result<NoticeLook> {
    const NOTICE: &str = "/BottomPanel/Content/UpperPart/Notifier/Content/Notification";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(NOTICE)).with_context(|| format!("{file} has no notice's template"))?;
    let panel = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, top) };
    let (content, text, icon) = (panel.find("Content").context("the notice's box")?, panel.find("Content/Text").context("the notice's text")?, panel.find("Content/Icon").context("the notice's icon")?);
    let padding = panel.nodes[content].group.map_or([0.0; 4], |group| group.padding);
    // Below the box: the row's own padding, and the bottom panel's
    // `Content`'s (the notifier's column ends at its foot).
    let column = |node: usize| scene.behaviour(node, "VerticalLayoutGroup").and_then(|group| row_group(scene.fields(group))).map_or(0.0, |group| group.padding[3]);
    let mut below = column(top);
    let mut at = scene.nodes[top].parent;
    while let Some(node) = at {
        if scene.path(node).ends_with("/BottomPanel/Content") {
            below += column(node);
            break;
        }
        at = scene.nodes[node].parent;
    }
    let [alert, plain] = sprites_named(install, "resources.assets", ["notification_icon_alert_red", "notification_icon_alert_green"]);
    Ok(NoticeLook { panel, content, text, icon, padding, below, alert, plain })
}

/// Sprites of one of the build's files by their names, where it has them.
pub fn sprites_named<const N: usize>(install: &Install, file: &str, names: [&str; N]) -> [Option<(String, i64)>; N] {
    let mut found = [(); N].map(|_| None);
    let Ok(mut pictures) = ScenePictures::new(install) else { return found };
    let Ok(ids) = pictures.objects(file, SPRITE) else { return found };
    for id in ids {
        let Ok(cut) = pictures.sprite(file, id) else { continue };
        if let Some(at) = names.iter().position(|name| *name == cut.name) {
            found[at] = Some((file.to_owned(), id));
        }
        if found.iter().all(Option::is_some) {
            break;
        }
    }
    found
}

/// A sprite's pixels, cut from its texture (its largest level): its width,
/// height and RGBA bytes, the top row first.
pub fn sprite_rgba(pictures: &mut ScenePictures, file: &str, path_id: i64) -> Result<(u32, u32, Vec<u8>)> {
    let cut = pictures.sprite(file, path_id)?;
    let texture = pictures.texture(&cut.texture.0, cut.texture.1)?;
    let levels = crate::unity::texture::mips(&texture)?;
    let pixels = crate::unity::texture::decode_rgba(&texture, *levels.first().context("the texture has no levels")?)?;
    let [x, y, w, h] = cut.rect.map(|v| v.round().max(0.0) as usize);
    let width = texture.width as usize;
    anyhow::ensure!(x + w <= width && y + h <= texture.height as usize, "the sprite {:?} lies outside its texture", cut.name);
    let mut out = Vec::with_capacity(w * h * 4);
    for row in (y..y + h).rev() {
        out.extend_from_slice(&pixels[(row * width + x) * 4..(row * width + x + w) * 4]);
    }
    Ok((w as u32, h as u32, out))
}

/// The pixels of a sprite of one of the build's files by its name (`sprite_rgba`).
pub fn sprite_rgba_named(install: &Install, file: &str, name: &str) -> Result<(u32, u32, Vec<u8>)> {
    let [found] = sprites_named(install, file, [name]);
    let (file, id) = found.with_context(|| format!("{file} has no sprite named {name:?}"))?;
    sprite_rgba(&mut ScenePictures::new(install)?, &file, id)
}

/// The glow a weapon's malfunction brings up at the screen's lower right
/// (`PreloaderUI._malfunctionGlow`: `BattleUIMalfunctionGlow` on
/// `Preloader UI/MalfunctionGlow` in the preloader's scene): its panel (its
/// place), and each of its kinds' (`_glows`, `GlowParams`: the kind's
/// number, `EGlowType` Examined 0, Repaired 1, TypeExamined 2; its picture;
/// the curve of its alpha over its seconds).
#[derive(Debug, Clone)]
pub struct GlowLook {
    pub panel: ScenePanel,
    pub glows: Vec<(i32, (String, i64), crate::formats::curve::Curve)>,
}

pub fn malfunction_glow(install: &Install) -> Result<GlowLook> {
    const GLOW: &str = "Preloader UI/MalfunctionGlow";
    let index = scene_paths(install)?.iter().position(|path| path.ends_with("UI/PreloaderUIScene.unity")).context("the build has no preloader's scene")?;
    let file = format!("level{index}");
    let scene = Scene::load(install, &file, true)?;
    let top = (0..scene.nodes.len()).find(|&node| scene.path(node).ends_with(GLOW)).with_context(|| format!("{file} has no malfunction glow"))?;
    let panel = ScenePanel { file: file.clone(), ..ScenePanel::of(&scene, top) };
    let fields = panel.script(0, "BattleUIMalfunctionGlow").context("the glow's script")?;
    // First its base's (`UIElement`, a `SerializedMonoBehaviour`): Odin's
    // `serializationData`, its format, bytes, objects, string, prefab, the
    // prefab's objects and changes, and its nodes (none here, which is
    // what this reads past). Then its image (`_glow`), then its kinds: each
    // its number, its sprite and its curve (keys of time, value, slopes in
    // and out, the weighting and its two weights; then the curve's ends and
    // its turn order).
    let mut r = Reader::new(fields);
    let align = |r: &mut Reader| r.pos = (r.pos + 3) & !3;
    r.i32()?;
    let bytes = r.i32()? as usize;
    r.pos += bytes;
    align(&mut r);
    for _ in 0..r.i32()? {
        pointer(&mut r)?;
    }
    let length = r.i32()? as usize;
    r.pos += length;
    align(&mut r);
    pointer(&mut r)?;
    for _ in 0..r.i32()? {
        pointer(&mut r)?;
    }
    for _ in 0..r.i32()? {
        let length = r.i32()? as usize;
        r.pos += length;
        align(&mut r);
    }
    anyhow::ensure!(r.i32()? == 0, "the glow's script keeps Odin's nodes, which are not read");
    pointer(&mut r)?;
    let mut glows = Vec::new();
    for _ in 0..r.i32()? {
        let kind = r.i32()?;
        let sprite = pointer(&mut r)?;
        let mut curve = crate::formats::curve::Curve::default();
        for _ in 0..r.i32()? {
            let (time, value, in_slope, out_slope) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
            let (_weighted, _in_weight, _out_weight) = (r.i32()?, r.f32()?, r.f32()?);
            curve.keys.push(crate::formats::curve::Key { time, value, in_slope, out_slope });
        }
        let (_before, _after, _order) = (r.i32()?, r.i32()?, r.i32()?);
        let Some(at) = panel.file_of(sprite.file) else { continue };
        glows.push((kind, (at, sprite.path), curve));
    }
    Ok(GlowLook { panel, glows })
}
