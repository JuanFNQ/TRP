//! TextMeshPro's texts: effects, copies, rich spans and drawing.

use super::*;

/// What a font asset's material draws under its letters (TextMeshPro's
/// distance-field shader): a dark rim all round them, from its outline
/// (`_OutlineWidth`) and an underlay laid unmoved under them and dilated
/// (`UNDERLAY_ON`, `_UnderlayDilate`; Bender Outline: an outline of 0.1 and
/// an underlay of 0.6, black), or a drop shadow, an underlay moved
/// (`_UnderlayOffsetX`, `_UnderlayOffsetY`; Bender Shadowed: 0.3 right and
/// down, black at 0.33). Each as wide or as far, in pixels, for a letter of
/// one, the atlas sampled at the asset's point size: an underlay moves by
/// its offset times its scale ratio (`_ScaleRatioC`) times the gradient's
/// scale (`_GradientScale`) of the atlas's pixels; an outline and an
/// underlay grow by half their width times their ratio (`_ScaleRatioA`,
/// `_ScaleRatioC`) in the distance field's measure, whose whole spans twice
/// the atlas's padding (`_GradientScale` less one; 0.6 of Bender Outline
/// is about a pixel at a size of 11). The face's own dilation
/// (`_FaceDilate`) widens the letters as far, and the shader's soft edge
/// fades the rim over its ramp (`PROVISIONAL`: drawn as copies of the
/// words moved round, a fainter ring for the fade; the field's measure,
/// TextMeshPro's generator not read).
#[derive(Clone, Copy, Debug, Default)]
pub struct TextEffect {
    /// The rim's colour and how far out from the letters its edge lies
    /// (where it is half drawn), for a letter of one.
    pub rim: Option<(Color, f32)>,
    pub shadow: Option<(Color, [f32; 2])>,
    /// How much the letters themselves are widened each way, for a letter
    /// of one (`_FaceDilate`: bolder than the font's own).
    pub face: f32,
    /// How wide the rim's edge fades, in pixels (the shader's ramp, the
    /// same at every size).
    pub soft: f32,
}

impl TextEffect {
    /// Drawn at a share of its alpha, as its words are.
    pub fn faded(self, alpha: f32) -> Self {
        let fade = |color: Color| color.with_alpha(color.alpha() * alpha);
        Self { rim: self.rim.map(|(color, width)| (fade(color), width)), shadow: self.shadow.map(|(color, at)| (fade(color), at)), ..self }
    }

    pub(super) fn of(install: &trp_data::Install, file: &str, id: i64) -> Option<Self> {
        let font = trp_data::interface::screen::TmpFont::read(install, file, id).ok()?;
        let (material_file, material) = trp_data::interface::screen::font_material(install, file, id).ok()?;
        let props = trp_data::interface::screen::material_properties(install, &material_file, material).ok()?;
        let float = |name: &str| props.floats.get(name).copied().unwrap_or(0.0);
        let color = |name: &str| props.colors.get(name).copied().map_or(Color::BLACK, rgba);
        // An underlay is moved by so many of the atlas's pixels: its
        // offset times its ratio times the gradient's scale (the shader's
        // `_UnderlayOffset * _ScaleRatioC * _GradientScale / _TextureWidth`
        // of the texture's width).
        let per = float("_GradientScale") / font.point_size.max(1.0);
        let underlay = props.keywords.iter().any(|keyword| keyword == "UNDERLAY_ON");
        let offset = [float("_UnderlayOffsetX"), -float("_UnderlayOffsetY")].map(|o| o * float("_ScaleRatioC") * per);
        let moved = offset.iter().any(|o| o.abs() > 1e-4);
        // An outline and a dilation are in the distance field's own
        // measure, whose 0 to 1 spans twice the atlas's padding
        // (`_GradientScale` less one) in its pixels, the edge at a half.
        let padding = (float("_GradientScale") - 1.0).max(0.0);
        let spread = 2.0 * padding / font.point_size.max(1.0);
        // The letters widened by the face's dilation (the shader's
        // `weight`: `_FaceDilate * _ScaleRatioA * 0.5`), the rim's edge as
        // far again out as its own width (the underlay's bias: `weight` and
        // `_UnderlayDilate * _ScaleRatioC * 0.5`; an outline's,
        // `_OutlineWidth * _ScaleRatioA * 0.5`); both edges fade over the
        // shader's ramp, a distance of one over its scale (`_GradientScale`
        // times the screen's pixels to the atlas's), which comes to twice
        // the padding over the gradient's scale in pixels at any size.
        let weight = float("_FaceDilate") * float("_ScaleRatioA") * 0.5;
        let rim = [float("_OutlineWidth") * float("_ScaleRatioA"), if underlay && !moved { float("_UnderlayDilate") * float("_ScaleRatioC") } else { 0.0 }];
        let rim = rim[0].max(rim[1]) * 0.5;
        let effect = Self {
            rim: (rim > 1e-4).then(|| (if underlay && !moved { color("_UnderlayColor") } else { color("_OutlineColor") }, (weight + rim) * spread)),
            shadow: (underlay && moved).then(|| (color("_UnderlayColor"), offset)),
            face: weight * spread,
            soft: if float("_GradientScale") > 0.0 { 2.0 * padding / float("_GradientScale") } else { 0.0 },
        };
        (effect.rim.is_some() || effect.shadow.is_some() || effect.face > 1e-4).then_some(effect)
    }
}

/// A copy of words drawn under them as their font's rim or shadow.
#[derive(Component)]
pub struct EffectCopy;

/// Any copy of words drawn round them (a rim's, a shadow's, the face
/// widened), not the words themselves: the game draws these in the words'
/// own shader, so the interface's dump leaves them out (`ui_dump`).
#[derive(Component)]
pub struct WordsCopy;

/// Words drawn as the game draws them (`trp_data::tmp_text`): their font
/// asset and material (each by its file and number), their runs (sizes in
/// the window's units, colours as the screen's numbers) and how they are
/// laid out (a width to break lines at in the window's units). Drawn into
/// an image of their own by `draw_tmp_texts` as they are made and as they
/// change.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TmpText {
    pub asset: (String, i64),
    pub material: (String, i64),
    pub runs: Vec<trp_data::interface::tmp_text::TmpRun>,
    pub layout: trp_data::interface::tmp_text::TmpLayout,
}

/// A colour as the screen's numbers (gamma), for a text drawn as the
/// game's.
pub fn screen_numbers(color: Color) -> [f32; 4] {
    let c = color.to_srgba();
    [c.red, c.green, c.blue, c.alpha]
}

/// The box a text drawn as the game's is aligned by, which its image
/// hangs from.
#[derive(Component)]
pub struct TmpBox;

/// Words drawn as the game draws them: a box laid out as `node` holding a
/// box as large as TextMeshPro aligns the words by (as wide as their
/// advances, as high as the font's ascender to its descender), and from
/// it their image, what the shader draws round them outside it. `mark`
/// puts the caller's markers on the image. Drawn once its image is made
/// (`draw_tmp_texts`), on the same frame. The box laid out as `node`.
pub fn tmp_words(parent: &mut ChildSpawnerCommands, node: Node, text: TmpText, mark: &mut dyn FnMut(&mut EntityCommands)) -> Entity {
    parent.spawn(node).with_children(|frame| {
        let empty = Node { width: Val::Px(0.0), height: Val::Px(0.0), ..default() };
        frame.spawn((TmpBox, empty.clone())).with_children(|aligned| {
            let hung = Node { position_type: PositionType::Absolute, ..empty };
            let mut words = aligned.spawn((text, hung, ImageNode { color: Color::WHITE, ..default() }));
            mark(&mut words);
        });
    })
    .id()
}

/// The images of the texts drawn as the game draws them, each made as its
/// words are first drawn or change and kept by what it is (the same words
/// alike drawn again take the same image), at the window's own pixels:
/// the image as large as it comes, its words' box where the layout puts
/// the node, by margins as wide as what lies outside it.
#[allow(clippy::type_complexity)]
pub(crate) fn draw_tmp_texts(
    look: Res<HealthLook>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut texts: Query<(&TmpText, &mut ImageNode, &mut Node, &ChildOf), Changed<TmpText>>,
    mut boxes: Query<&mut Node, (With<TmpBox>, Without<TmpText>)>,
    mut made: Local<HashMap<String, (Handle<Image>, [f32; 2], [f32; 4])>>,
) {
    if texts.is_empty() {
        return;
    }
    let _clock = crate::platform::render_timing::Clock::new("texts drawn as the game's");
    let scale = window.single().map_or(1.0, |window| window.scale_factor());
    // Kept while the screens draw the same texts; let go when it has grown
    // large (each image is kept as long as a text shows it).
    if made.len() > 4096 {
        made.clear();
    }
    for (text, mut image, mut node, parent) in &mut texts {
        let key = format!("{scale}|{text:?}");
        let (handle, size, layout) = match made.get(&key) {
            Some(kept) => kept.clone(),
            None => {
                let (Some(face), Some(material)) = (look.tmp_faces.get(&text.asset), look.tmp_materials.get(&text.material)) else { continue };
                let runs: Vec<trp_data::interface::tmp_text::TmpRun> = text.runs.iter().map(|run| trp_data::interface::tmp_text::TmpRun { size: run.size * scale, ..run.clone() }).collect();
                let layout = trp_data::interface::tmp_text::TmpLayout { em: text.layout.em * scale, wrap: text.layout.wrap.map(|width| width * scale), lead: text.layout.lead * scale, ..text.layout };
                let drawn = trp_data::interface::tmp_text::draw(face, material, &runs, &layout);
                let mut picture = Image::new(
                    bevy::render::render_resource::Extent3d { width: drawn.width as u32, height: drawn.height as u32, depth_or_array_layers: 1 },
                    bevy::render::render_resource::TextureDimension::D2,
                    drawn.rgba,
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::RENDER_WORLD,
                );
                picture.sampler = bevy::image::ImageSampler::nearest();
                let kept = (images.add(picture), [drawn.width as f32 / scale, drawn.height as f32 / scale], drawn.layout.map(|v| v / scale));
                made.insert(key, kept.clone());
                kept
            }
        };
        let [lx, ly, lw, lh] = layout;
        let [w, h] = size;
        image.image = handle;
        (node.left, node.top, node.width, node.height) = (Val::Px(-lx), Val::Px(-ly), Val::Px(w), Val::Px(h));
        if let Ok(mut aligned) = boxes.get_mut(parent.parent()) {
            (aligned.width, aligned.height) = (Val::Px(lw), Val::Px(lh));
        }
    }
}

/// Words with what their font asset draws under them: a box laid out as
/// `node` holding, in one cell of a grid, copies of the words in the
/// effect's colour moved twelve ways round for the rim (a whole ring and a
/// fainter one outside it, for its soft edge) or moved for the shadow,
/// copies in their own colour moved four ways by the face's dilation, and
/// the words themselves over them. `runs` are the words
/// and each one's font and colour; `size` the letters' height in pixels the
/// effect is measured by. `mark` puts the caller's markers on each text
/// (`None`) and each run (its place), and says whether it is a copy.
pub fn effect_words(parent: &mut ChildSpawnerCommands, node: Node, runs: &[(String, TextFont, Color)], effect: Option<TextEffect>, size: f32, mark: &mut dyn FnMut(&mut EntityCommands, Option<usize>, bool)) {
    let Some((_, first_font, first_color)) = runs.first() else { return };
    // Words in a box of a set width are broken into lines at it (a
    // window's description), each copy as wide; others run on one line.
    let wrapped = matches!(node.width, Val::Px(_));
    let layout = if wrapped { TextLayout::new_with_linebreak(LineBreak::WordBoundary) } else { TextLayout::new_with_no_wrap() };
    // Each copy: its colour (none: the words' own) and where it is moved.
    let mut copies: Vec<(Option<Color>, Vec2)> = Vec::new();
    // Twelve ways round, so that the rim is round.
    let round = |r: f32| (0..12).map(move |k| Vec2::from_angle(k as f32 * std::f32::consts::TAU / 12.0) * r);
    if let Some((color, width)) = effect.and_then(|effect| effect.rim) {
        // The rim's edge fades over the shader's ramp: whole to a quarter
        // of it inside its edge, then a fainter ring a quarter of it out.
        let (edge, soft) = (width * size, effect.map_or(0.0, |effect| effect.soft));
        if soft > 0.5 {
            copies.extend(round(edge + soft / 4.0).map(|at| (Some(color.with_alpha(color.alpha() * 0.4)), at)));
        }
        copies.extend(round((edge - soft / 4.0).max(0.5)).map(|at| (Some(color), at)));
    }
    if let Some((color, [x, y])) = effect.and_then(|effect| effect.shadow) {
        copies.push((Some(color), Vec2::new(x, y) * size));
    }
    // The letters widened: the words again in their own colours, moved
    // that far each way.
    let face = effect.map_or(0.0, |effect| effect.face * size);
    if face >= 0.15 {
        copies.extend([Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y].map(|way| (None, way * face)));
    }
    let cell = || Node { grid_row: GridPlacement::start(1), grid_column: GridPlacement::start(1), width: if wrapped { Val::Percent(100.0) } else { Val::Auto }, ..default() };
    parent.spawn(Node { display: Display::Grid, ..node }).with_children(|grid| {
        for (color, at) in copies.iter().copied().chain(std::iter::once((None, Vec2::ZERO))) {
            let copy = color.is_some();
            let moved = UiTransform { translation: Val2::px(at.x, at.y), ..UiTransform::IDENTITY };
            let mut text = grid.spawn((cell(), Text::new(""), first_font.clone(), TextColor(color.unwrap_or(*first_color)), layout, moved));
            if copy {
                text.insert(EffectCopy);
            }
            if copy || at != Vec2::ZERO {
                text.insert(WordsCopy);
            }
            mark(&mut text, None, copy);
            text.with_children(|words| {
                for (at, (said, font, run_color)) in runs.iter().enumerate() {
                    let mut span = words.spawn((TextSpan::new(said.clone()), font.clone(), TextColor(color.unwrap_or(*run_color))));
                    if copy {
                        span.insert(EffectCopy);
                    }
                    mark(&mut span, Some(at), copy);
                }
            });
        }
    });
}

/// The weight a text is laid out at by its style (TextMeshPro's
/// `FontStyles.Bold`): Bevy finds a face by its family and the weight
/// asked, so a bold typeface of the same family as the regular one is only
/// had by asking for it bold.
pub fn weight_of(style: u32) -> FontWeight {
    if style & 1 != 0 { FontWeight::BOLD } else { FontWeight::NORMAL }
}

/// Words with TextMeshPro's rich text in them as runs, each with the colour
/// its `<color=#RRGGBB>` or `<color=#RRGGBBAA>` gives it (none: the text's
/// own) up to its `</color>`, the tags of the bold, the italic and the size
/// left out; none where the words have no colour tag, which are drawn as
/// they are.
pub(crate) fn rich_spans(words: &str) -> Option<Vec<(String, Option<[f32; 4]>)>> {
    if !words.contains("<color") {
        return None;
    }
    let mut spans: Vec<(String, Option<[f32; 4]>)> = Vec::new();
    for (said, colour, _) in rich_runs(words) {
        match spans.last_mut() {
            Some((last, was)) if *was == colour => last.push_str(&said),
            _ => spans.push((said, colour)),
        }
    }
    Some(spans)
}

/// Words with TextMeshPro's rich text in them as runs, each with the colour
/// its `<color=…>` gives it (none: the text's own) and whether a `<b>` makes
/// it bold, up to their closing tags; the tags of the italic, the underline
/// and the size left out, any other kept as words (a `<sprite>` among them).
pub(crate) fn rich_runs(words: &str) -> Vec<(String, Option<[f32; 4]>, bool)> {
    let hex = |digits: &str| -> Option<[f32; 4]> {
        let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok().map(|b| b as f32 / 255.0);
        match digits.len() {
            6 => Some([byte(0)?, byte(2)?, byte(4)?, 1.0]),
            8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
            _ => None,
        }
    };
    let mut spans: Vec<(String, Option<[f32; 4]>, bool)> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut bold = 0u32;
    let mut current = String::new();
    let mut rest = words;
    while let Some(open) = rest.find('<') {
        current.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>').map(|at| open + at) else {
            current.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let tag = &rest[open + 1..close];
        // A colour by its figures, or by one of TextMeshPro's names for one
        // (`TMP_Text`'s `<color=white>`: black, blue, green, orange, purple,
        // red, white, yellow).
        let named = |name: &str| -> Option<[f32; 4]> {
            let figures = match name {
                "black" => "000000",
                "blue" => "0000FF",
                "green" => "00FF00",
                "orange" => "FF8000",
                "purple" => "A020F0",
                "red" => "FF0000",
                "white" => "FFFFFF",
                "yellow" => "FFFF00",
                _ => return None,
            };
            hex(figures)
        };
        let colour = tag.strip_prefix("color=").map(|value| value.trim_matches('"')).and_then(|value| value.strip_prefix('#').map_or_else(|| named(value).or_else(|| hex(value)), hex));
        let known = colour.is_some() || matches!(tag, "/color" | "b" | "/b" | "i" | "/i" | "u" | "/u" | "/size") || tag.starts_with("size=");
        if !known {
            current.push_str(&rest[open..=close]);
        } else if colour.is_some() || matches!(tag, "/color" | "b" | "/b") {
            if !current.is_empty() {
                spans.push((std::mem::take(&mut current), colours.last().copied(), bold > 0));
            }
            match (colour, tag) {
                (Some(colour), _) => colours.push(colour),
                (None, "b") => bold += 1,
                (None, "/b") => bold = bold.saturating_sub(1),
                _ => {
                    colours.pop();
                }
            }
        }
        rest = &rest[close + 1..];
    }
    current.push_str(rest);
    if !current.is_empty() {
        spans.push((current, colours.last().copied(), bold > 0));
    }
    spans
}
