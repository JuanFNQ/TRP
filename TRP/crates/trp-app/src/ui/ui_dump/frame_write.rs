//! The dump's frames, figures, screens and looks, and one frame written.

use super::*;

/// The frames counted as the screenshot counts them.
#[derive(Resource, Default)]
pub(super) struct Frames(pub(super) u32);

pub(super) fn count_frame(mut frames: ResMut<Frames>) {
    frames.0 += 1;
}

/// An element's figures as last written, rounded as written.
#[derive(Clone, PartialEq)]
pub(super) struct Figures {
    pub(super) kind: &'static str,
    pub(super) rect: [f32; 4],
    pub(super) turn: f32,
    pub(super) color: [f32; 4],
    pub(super) depth: u64,
    pub(super) image: Option<&'static str>,
    pub(super) picture: Option<String>,
    pub(super) fill: Option<f32>,
    pub(super) words: Option<String>,
    pub(super) font: Option<(Option<String>, f32)>,
    pub(super) ink: Option<[f32; 4]>,
    pub(super) clip: Option<[f32; 4]>,
}

pub(super) struct Seen {
    pub(super) walk: u64,
    pub(super) figures: Figures,
}

/// What was last written, to write only what changed.
#[derive(Resource)]
pub(super) struct Dump {
    pub(super) out: std::io::BufWriter<std::fs::File>,
    pub(super) from: u32,
    failed: bool,
    lines: u32,
    pub(super) walk: u64,
    pub(super) elements: HashMap<u64, Seen>,
    canvas: Option<String>,
    screen: Option<&'static str>,
    pub(super) size: Option<[u32; 2]>,
    cursor: Option<bool>,
    mouse: Option<[f32; 2]>,
    buttons: Option<u32>,
    over: Option<Option<Entity>>,
    /// Each font's name as found, once its asset is there.
    font_names: HashMap<AssetId<Font>, Option<String>>,
}

impl Dump {
    pub(super) fn new(out: std::io::BufWriter<std::fs::File>, from: u32) -> Self {
        Self {
            out,
            from,
            failed: false,
            lines: 0,
            walk: 0,
            elements: HashMap::new(),
            canvas: None,
            screen: None,
            size: None,
            cursor: None,
            mouse: None,
            buttons: None,
            over: None,
            font_names: HashMap::new(),
        }
    }
}

/// The window, the screen shown and the pointer.
#[derive(SystemParam)]
pub(super) struct Screen<'w, 's> {
    frames: Res<'w, Frames>,
    time: Res<'w, Time>,
    ui_scale: Res<'w, UiScale>,
    pub(super) controller: Res<'w, Controller>,
    pub(super) selected: Option<Res<'w, Selected>>,
    buttons: Option<Res<'w, ButtonInput<MouseButton>>>,
    pub(super) window: Query<'w, 's, (Entity, &'static Window, Option<&'static CursorOptions>), With<PrimaryWindow>>,
    cameras: Query<'w, 's, (Entity, &'static Camera, &'static RenderTarget)>,
    hovered: Query<'w, 's, (Entity, &'static Interaction, &'static ComputedNode)>,
}

/// What an element's figures are read from beside its own node.
#[derive(SystemParam)]
pub(super) struct Looks<'w, 's> {
    boxes: Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform)>,
    parents: Query<'w, 's, &'static ChildOf>,
    pub(super) labels: Query<'w, 's, (Option<&'static Name>, Option<&'static ChildOf>, Has<Text>, Has<ImageNode>, Has<TmpText>)>,
    pub(super) children: Query<'w, 's, &'static Children>,
    reader: TextUiReader<'w, 's>,
    pub(super) look: Option<Res<'w, HealthLook>>,
    pub(super) fonts: Res<'w, Assets<Font>>,
    fills: Option<Res<'w, Assets<RadialFill>>>,
    glows: Option<Res<'w, Assets<GlowMaterial>>>,
}

pub(super) type Drawn = (
    (Entity, &'static ComputedNode, &'static UiGlobalTransform, &'static InheritedVisibility, &'static ComputedUiTargetCamera, Option<&'static CalculatedClip>),
    (Option<&'static BackgroundColor>, Option<&'static BorderColor>, Option<&'static ImageNode>, Option<&'static MaterialNode<RadialFill>>, Option<&'static MaterialNode<GlowMaterial>>),
    (Has<Text>, Option<&'static TextLayoutInfo>, Option<&'static TextFont>, Option<&'static TextColor>, Option<&'static TmpText>, Has<WordsCopy>),
);

/// The parts of a node, each an element of its own.
const GROUND: u64 = 0;

const BORDER: u64 = 1;

const PICTURE: u64 = 2;

const MATERIAL: u64 = 3;

const WORDS: u64 = 4;

pub(super) fn write_frame(mut dump: ResMut<Dump>, screen: Screen, mut looks: Looks, nodes: Query<Drawn>, mut heard: ResMut<Heard>) {
    let frame = screen.frames.0;
    let sounds = std::mem::take(&mut heard.0);
    if dump.failed || frame == 0 || frame < dump.from {
        return;
    }
    let Ok((window_entity, window, cursor)) = screen.window.single() else { return };
    let dump = &mut *dump;
    let mut line = String::with_capacity(1024);
    let _ = write!(line, "{{\"frame\":{frame},\"t\":{}", f(screen.time.elapsed_secs()));
    let bare = line.len();
    if dump.lines == 0 {
        let _ = write!(line, ",\"interface\":{}", text(&format!("TRP {}", env!("TRP_VERSION"))));
    }

    // The screen shown, the window, the cursor and the mouse.
    let shown = screen_of(&screen.controller, screen.selected.as_deref());
    if dump.screen != Some(shown) {
        dump.screen = Some(shown);
        let _ = write!(line, ",\"screen\":{}", text(shown));
    }
    let size = [window.physical_width(), window.physical_height()];
    if dump.size != Some(size) {
        dump.size = Some(size);
        let _ = write!(line, ",\"size\":[{},{}]", size[0], size[1]);
    }
    let visible = cursor.is_none_or(|cursor| cursor.visible);
    if dump.cursor != Some(visible) {
        dump.cursor = Some(visible);
        let _ = write!(line, ",\"cursor\":{visible}");
    }
    // To a tenth of a pixel, as the recorder takes it; kept where the
    // pointer is outside the window.
    if let Some(at) = window.physical_cursor_position() {
        let at = [(at.x * 10.0).round() / 10.0, (at.y * 10.0).round() / 10.0];
        if dump.mouse != Some(at) {
            dump.mouse = Some(at);
            let _ = write!(line, ",\"mouse\":[{},{}]", p(at[0]), p(at[1]));
        }
    }
    let held = screen.buttons.as_deref().map_or(0, |buttons| {
        [(MouseButton::Left, 1), (MouseButton::Right, 2), (MouseButton::Middle, 4)].iter().filter(|(button, _)| buttons.pressed(*button)).map(|(_, bit)| bit).sum()
    });
    if dump.buttons != Some(held) {
        dump.buttons = Some(held);
        let _ = write!(line, ",\"buttons\":{held}");
    }
    // What the pointer is over: the topmost node the engine finds it over.
    let over = screen
        .hovered
        .iter()
        .filter(|(_, interaction, _)| matches!(interaction, Interaction::Hovered | Interaction::Pressed))
        .max_by_key(|(_, _, node)| node.stack_index)
        .map(|(entity, ..)| entity);
    if dump.over != Some(over) {
        dump.over = Some(over);
        let _ = write!(line, ",\"over\":{}", over.map_or("null".to_owned(), |entity| text(&path_of(entity, &looks))));
    }

    // The app's one canvas.
    let scale = window.scale_factor() * screen.ui_scale.0;
    let canvas = format!("{{\"id\":1,\"path\":\"app\",\"root\":1,\"mode\":\"ScreenSpaceOverlay\",\"order\":0,\"layer\":\"Default\",\"scale\":{},\"on\":true}}", f(r4(scale)));
    if dump.canvas.as_ref() != Some(&canvas) {
        let _ = write!(line, ",\"canvases\":[{canvas}]");
        dump.canvas = Some(canvas);
    }

    // The elements: those the cameras drawing to the window draw.
    let to_window: Vec<Entity> = screen
        .cameras
        .iter()
        .filter(|(_, camera, target)| {
            camera.is_active && (matches!(target, RenderTarget::Window(WindowRef::Primary)) || matches!(target, RenderTarget::Window(WindowRef::Entity(at)) if *at == window_entity))
        })
        .map(|(entity, ..)| entity)
        .collect();
    dump.walk += 1;
    let walk = dump.walk;
    let mut added: Vec<(u64, String)> = Vec::new();
    let mut changed: Vec<(u64, String)> = Vec::new();
    for ((entity, node, transform, visibility, camera, clip), (ground, border, picture, fill, glow), (has_text, layout, font, color, tmp, copy)) in &nodes {
        if !visibility.get() || node.is_empty() || copy || !camera.get().is_some_and(|camera| to_window.contains(&camera)) {
            continue;
        }
        let (rect, turn) = box_of(transform, node.size());
        let clip = clip.map(|clip| [r2(clip.clip.min.x), r2(clip.clip.min.y), r2(clip.clip.max.x), r2(clip.clip.max.y)]);
        let depth = |part: u64| u64::from(node.stack_index) * 8 + part;
        let id = |part: u64| entity.to_bits().wrapping_mul(8).wrapping_add(part);
        let plain = |kind: &'static str, image: Option<&'static str>, color: Color, part: u64| Figures {
            kind,
            rect,
            turn,
            color: rgba(color),
            depth: depth(part),
            image,
            picture: None,
            fill: None,
            words: None,
            font: None,
            ink: None,
            clip,
        };
        let mut found: Vec<(u64, Figures)> = Vec::new();
        if let Some(ground) = ground.filter(|ground| !ground.0.is_fully_transparent()) {
            found.push((id(GROUND), plain("Image", Some("Simple"), ground.0, GROUND)));
        }
        if let Some(border) = border.filter(|_| node.border.min_inset != Vec2::ZERO || node.border.max_inset != Vec2::ZERO) {
            if let Some(color) = [border.top, border.right, border.bottom, border.left].into_iter().find(|color| !color.is_fully_transparent()) {
                found.push((id(BORDER), plain("Image", Some("Sliced"), color, BORDER)));
            }
        }
        // A picture, unless it is the words drawn as the game draws them.
        if let Some(picture) = picture.filter(|picture| tmp.is_none() && !picture.color.is_fully_transparent() && picture.image.id() != bevy::image::TRANSPARENT_IMAGE_HANDLE.id()) {
            let mode = match picture.image_mode {
                NodeImageMode::Sliced(_) => "Sliced",
                NodeImageMode::Tiled { .. } => "Tiled",
                NodeImageMode::Auto | NodeImageMode::Stretch => "Simple",
            };
            let mut figures = plain("Image", Some(mode), picture.color, PICTURE);
            figures.picture = picture.image.path().map(|path| path.to_string());
            found.push((id(PICTURE), figures));
        }
        if let Some(material) = fill.and_then(|fill| looks.fills.as_ref()?.get(&fill.0)) {
            let c = material.color;
            let mut figures = plain("Image", Some("Filled"), Color::linear_rgba(c.x, c.y, c.z, c.w), MATERIAL);
            figures.fill = Some(r4(material.fill.x));
            figures.picture = material.texture.path().map(|path| path.to_string());
            found.push((id(MATERIAL), figures));
        }
        if let Some(material) = glow.and_then(|glow| looks.glows.as_ref()?.get(&glow.0)) {
            let mut figures = plain("Image", Some("Simple"), Color::WHITE.with_alpha(material.alpha.x), MATERIAL);
            figures.picture = material.texture.path().map(|path| path.to_string());
            found.push((id(MATERIAL), figures));
        }
        if has_text {
            let words: String = looks.reader.iter(entity).map(|(_, _, said, ..)| said).collect();
            let name = font.and_then(|font| font_name(&mut dump.font_names, &font.font, looks.look.as_deref(), &looks.fonts));
            let mut figures = plain("TextMeshProUGUI", None, color.map_or(Color::WHITE, |color| color.0), WORDS);
            figures.ink = layout.and_then(|layout| ink_of(transform, node.size(), layout));
            figures.words = Some(words);
            figures.font = Some((name, font.map_or(0.0, |font| r2(font.font_size))));
            found.push((id(WORDS), figures));
        }
        if let Some(tmp) = tmp {
            // Its box is the node it was laid out in (`tmp_words`), its ink
            // the box TextMeshPro aligns the words by, which the picture
            // hangs from: TextMeshPro's own text bounds run from the first
            // letter's origin to the last's advance and from the descender
            // to the ascender, as that box does.
            let aligned = looks.parents.get(entity).ok().map(ChildOf::parent);
            let laid = aligned.and_then(|aligned| looks.parents.get(aligned).ok()).map(ChildOf::parent);
            let rect_of = |of: Option<Entity>| of.and_then(|of| looks.boxes.get(of).ok()).map(|(node, transform)| box_of(transform, node.size()));
            let (rect, turn) = rect_of(laid).unwrap_or((rect, turn));
            let tint = picture.map_or(Color::WHITE, |picture| picture.color).to_srgba();
            let run = words_colour(&tmp.runs);
            let color = Color::srgba(run[0] * tint.red, run[1] * tint.green, run[2] * tint.blue, run[3] * tint.alpha);
            let name = looks
                .look
                .as_deref()
                .and_then(|look| look.asset_name(&tmp.asset).map(str::to_owned).or_else(|| look.tmp_faces.get(&tmp.asset).map(|face| format!("{} {}", face.font.family, face.font.style))));
            let mut figures = plain("TextMeshProUGUI", None, color, WORDS);
            figures.rect = rect;
            figures.turn = turn;
            figures.ink = rect_of(aligned).map(|(rect, _)| rect);
            figures.words = Some(tmp.runs.iter().map(|run| run.text.as_str()).collect());
            figures.font = Some((name, r2(tmp.layout.em)));
            found.push((id(WORDS), figures));
        }
        for (id, figures) in found {
            element(&mut dump.elements, walk, id, figures, &mut added, &mut changed, || path_of(entity, &looks));
        }
    }
    let mut gone: Vec<u64> = dump.elements.iter().filter(|(_, seen)| seen.walk != walk).map(|(id, _)| *id).collect();
    gone.sort_unstable();
    for id in &gone {
        dump.elements.remove(id);
    }
    // In the order they are drawn in.
    added.sort_by_key(|(depth, _)| *depth);
    changed.sort_by_key(|(depth, _)| *depth);
    let join = |list: &[(u64, String)]| list.iter().map(|(_, own)| own.as_str()).collect::<Vec<_>>().join(",");
    if !added.is_empty() {
        let _ = write!(line, ",\"add\":[{}]", join(&added));
    }
    if !changed.is_empty() {
        let _ = write!(line, ",\"set\":[{}]", join(&changed));
    }
    if !gone.is_empty() {
        let _ = write!(line, ",\"gone\":[{}]", gone.iter().map(u64::to_string).collect::<Vec<_>>().join(","));
    }
    if !sounds.is_empty() {
        let _ = write!(line, ",\"sounds\":[{}]", sounds.iter().map(told).collect::<Vec<_>>().join(","));
    }
    if line.len() == bare {
        return;
    }
    line.push_str("}\n");
    // Each line flushed as written, so that the file is whole however the
    // run ends.
    if let Err(e) = dump.out.write_all(line.as_bytes()).and_then(|()| dump.out.flush()) {
        warn!("the interface is no longer written: {e}");
        dump.failed = true;
        return;
    }
    dump.lines += 1;
}
