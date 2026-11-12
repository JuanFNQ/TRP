//! Each element's words colour, box, ink, path and font.

use super::*;

/// The colour of words drawn as TextMeshPro's, as the recorder writes the
/// game's: the text's own colour, which its colour tags change for a part
/// of its words (a figure's dim noughts, `<color=#99999933>0</color>98`).
/// The app keeps runs of colours, not the text's own: taken as the one most
/// of the letters are drawn in, the first of those where as many are (an
/// empty run before the words, which a figure's noughts fill, is not it).
pub(super) fn words_colour(runs: &[trp_data::interface::tmp_text::TmpRun]) -> [f32; 4] {
    let mut counts: Vec<([f32; 4], usize)> = Vec::new();
    for run in runs {
        let letters = run.text.chars().count();
        match counts.iter_mut().find(|(color, _)| *color == run.color) {
            Some((_, count)) => *count += letters,
            None => counts.push((run.color, letters)),
        }
    }
    counts.iter().fold(None, |best: Option<([f32; 4], usize)>, &(color, count)| match best {
        Some((_, most)) if most >= count => best,
        _ => Some((color, count)),
    })
    .map_or([1.0; 4], |(color, _)| color)
}

/// One element: whole when first seen, else what changed (a field gone
/// written as null), as the recorder writes them.
pub(super) fn element(elements: &mut HashMap<u64, Seen>, walk: u64, id: u64, figures: Figures, added: &mut Vec<(u64, String)>, changed: &mut Vec<(u64, String)>, path: impl FnOnce() -> String) {
    let depth = figures.depth;
    let Some(seen) = elements.get_mut(&id) else {
        let mut own = format!("{{\"id\":{id},\"path\":{},\"kind\":{},\"canvas\":1", text(&path()), text(figures.kind));
        let _ = write!(own, ",\"box\":{}", rect(figures.rect));
        if figures.turn != 0.0 {
            let _ = write!(own, ",\"turn\":{}", p(figures.turn));
        }
        let _ = write!(own, ",\"color\":{},\"alpha\":1,\"depth\":{}", color(figures.color), figures.depth);
        if let Some(image) = figures.image {
            let _ = write!(own, ",\"image\":{}", text(image));
        }
        if let Some(picture) = &figures.picture {
            let _ = write!(own, ",\"picture\":{}", text(picture));
        }
        if let Some(fill) = figures.fill {
            let _ = write!(own, ",\"fill\":{}", f(fill));
        }
        if let Some(words) = &figures.words {
            let _ = write!(own, ",\"words\":{}", text(words));
        }
        if let Some(font) = &figures.font {
            let _ = write!(own, ",\"font\":{}", font_of(font));
        }
        if let Some(ink) = figures.ink {
            let _ = write!(own, ",\"ink\":{}", rect(ink));
        }
        if let Some(clip) = figures.clip {
            let _ = write!(own, ",\"clip\":{}", rect(clip));
        }
        own.push('}');
        added.push((depth, own));
        elements.insert(id, Seen { walk, figures });
        return;
    };
    seen.walk = walk;
    if seen.figures == figures {
        return;
    }
    let was = &seen.figures;
    let mut own = String::new();
    if figures.rect != was.rect {
        let _ = write!(own, ",\"box\":{}", rect(figures.rect));
    }
    if figures.turn != was.turn {
        let _ = write!(own, ",\"turn\":{}", p(figures.turn));
    }
    if figures.color != was.color {
        let _ = write!(own, ",\"color\":{}", color(figures.color));
    }
    if figures.depth != was.depth {
        let _ = write!(own, ",\"depth\":{}", figures.depth);
    }
    if figures.picture != was.picture {
        let _ = write!(own, ",\"picture\":{}", figures.picture.as_deref().map_or("null".to_owned(), text));
    }
    if figures.fill != was.fill {
        let _ = write!(own, ",\"fill\":{}", figures.fill.map_or("null".to_owned(), f));
    }
    if figures.words != was.words {
        let _ = write!(own, ",\"words\":{}", figures.words.as_deref().map_or("null".to_owned(), text));
    }
    if figures.font != was.font {
        let _ = write!(own, ",\"font\":{}", figures.font.as_ref().map_or("null".to_owned(), font_of));
    }
    if figures.ink != was.ink {
        let _ = write!(own, ",\"ink\":{}", figures.ink.map_or("null".to_owned(), rect));
    }
    if figures.clip != was.clip {
        let _ = write!(own, ",\"clip\":{}", figures.clip.map_or("null".to_owned(), rect));
    }
    seen.figures = figures;
    if !own.is_empty() {
        changed.push((depth, format!("{{\"id\":{id}{own}}}")));
    }
}

/// The game's name for the screen the app shows (its list of screen types): the
/// edit build screen's, the settings' (the app's OPTIONS tab, which in the
/// game is a screen of its own), the inventory's, else the battle screen's.
pub(super) fn screen_of(controller: &Controller, selected: Option<&Selected>) -> &'static str {
    if controller.edit_build {
        "EditBuild"
    } else if controller.inventory_open && selected.is_some_and(|selected| selected.0 == crate::screens::inventory_view::OPTIONS) {
        "Settings"
    } else if controller.inventory_open {
        "Inventory"
    } else {
        "BattleUI"
    }
}

/// A node's box from its four corners, and its turn (degrees,
/// counter-clockwise as seen), rounded as written. The engine's transform
/// takes a node's middle to the window's pixels, down counted as positive.
pub(super) fn box_of(transform: &UiGlobalTransform, size: Vec2) -> ([f32; 4], f32) {
    let half = size * 0.5;
    let corners = [Vec2::new(-half.x, -half.y), Vec2::new(half.x, -half.y), Vec2::new(half.x, half.y), Vec2::new(-half.x, half.y)].map(|corner| transform.transform_point2(corner));
    let x_axis = transform.matrix2.x_axis;
    let turn = r2((-x_axis.y).atan2(x_axis.x).to_degrees());
    (spread(&corners), if turn == 0.0 { 0.0 } else { turn })
}

/// The box round points, rounded as written.
fn spread(points: &[Vec2]) -> [f32; 4] {
    let (min, max) = points.iter().fold((Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)), |(min, max), at| (min.min(*at), max.max(*at)));
    [r2(min.x), r2(min.y), r2(max.x), r2(max.y)]
}

/// Where a text's glyphs are drawn: each glyph's quad as the engine's
/// renderer places it (its middle at its position from the node's top left
/// corner, as large as its picture in the atlas), the box round them all.
/// Glyphs drawn as nothing (a space) are left out.
pub(super) fn ink_of(transform: &UiGlobalTransform, size: Vec2, layout: &TextLayoutInfo) -> Option<[f32; 4]> {
    let corner = -0.5 * size;
    let points: Vec<Vec2> = layout
        .glyphs
        .iter()
        .filter(|glyph| glyph.size.x > 0.0 && glyph.size.y > 0.0)
        .flat_map(|glyph| {
            let half = glyph.size * 0.5;
            let middle = corner + glyph.position;
            [middle - half, middle + Vec2::new(half.x, -half.y), middle + half, middle + Vec2::new(-half.x, half.y)]
        })
        .map(|at| transform.transform_point2(at))
        .collect();
    (!points.is_empty()).then(|| spread(&points))
}

/// A node's path: the names from its root down, joined by `/`. A node
/// without a name is called by what it draws (`Text`, `TmpText`, `Image`,
/// else `Node`) and its place among its parent's children, a root without
/// one by its entity.
pub(super) fn path_of(entity: Entity, looks: &Looks) -> String {
    let mut names = Vec::new();
    let mut at = Some(entity);
    while let Some(here) = at {
        let Ok((name, parent, text, image, tmp)) = looks.labels.get(here) else { break };
        names.push(match name {
            Some(name) => name.as_str().to_owned(),
            None => {
                let kind = if tmp {
                    "TmpText"
                } else if text {
                    "Text"
                } else if image {
                    "Image"
                } else {
                    "Node"
                };
                match parent {
                    Some(parent) => {
                        let place = looks.children.get(parent.parent()).ok().and_then(|children| children.iter().position(|child| child == here)).unwrap_or(0);
                        format!("{kind}[{place}]")
                    }
                    None => format!("{kind}#{}", here.index_u32()),
                }
            }
        });
        at = parent.map(ChildOf::parent);
    }
    names.reverse();
    names.join("/")
}

/// A font's name: the game's font asset's own where the font was made from
/// one, else its file's path, else the face's family and style from the
/// font's own table of names. Kept once found, or once its asset is there.
pub(super) fn font_name(names: &mut HashMap<AssetId<Font>, Option<String>>, font: &Handle<Font>, look: Option<&HealthLook>, fonts: &Assets<Font>) -> Option<String> {
    if let Some(name) = names.get(&font.id()) {
        return name.clone();
    }
    let asset = fonts.get(font.id());
    let name = look
        .and_then(|look| look.font_name(font.id()).map(str::to_owned))
        .or_else(|| font.path().map(|path| path.to_string()))
        .or_else(|| asset.and_then(|asset| face_name(&asset.data)));
    if name.is_some() || asset.is_some() {
        names.insert(font.id(), name.clone());
    }
    name
}

/// A TrueType or OpenType face's family and style, from its `name` table
/// (names 1 and 2): Windows' Unicode names first, else the Mac's Roman.
fn face_name(data: &[u8]) -> Option<String> {
    let u16_at = |at: usize| data.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let u32_at = |at: usize| data.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let tables = usize::from(u16_at(4)?);
    let record = (0..tables).map(|n| 12 + 16 * n).find(|&at| data.get(at..at + 4) == Some(b"name"))?;
    let base = u32_at(record + 8)? as usize;
    let count = usize::from(u16_at(base + 2)?);
    let strings = base + usize::from(u16_at(base + 4)?);
    let read = |wanted: u16| -> Option<String> {
        let mut roman = None;
        for n in 0..count {
            let at = base + 6 + 12 * n;
            let (platform, encoding, id) = (u16_at(at)?, u16_at(at + 2)?, u16_at(at + 6)?);
            if id != wanted {
                continue;
            }
            let (length, offset) = (usize::from(u16_at(at + 8)?), usize::from(u16_at(at + 10)?));
            let Some(bytes) = data.get(strings + offset..strings + offset + length) else { continue };
            match (platform, encoding) {
                (3, 1) | (0, _) => {
                    let units: Vec<u16> = bytes.chunks_exact(2).map(|b| u16::from_be_bytes([b[0], b[1]])).collect();
                    if let Ok(name) = String::from_utf16(&units) {
                        return Some(name);
                    }
                }
                (1, 0) if roman.is_none() => roman = Some(bytes.iter().map(|&b| char::from(b)).collect()),
                _ => {}
            }
        }
        roman
    };
    match (read(1), read(2)) {
        (Some(family), Some(style)) => Some(format!("{family} {style}")),
        (family, _) => family,
    }
}
