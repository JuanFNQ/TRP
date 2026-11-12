//! Binds, item grounds and looks, the pictures made and the version line.

use super::*;

/// The label of a thing bound to a key: its picture (the cut of its
/// texture) at its sprite's own size, in units, and its key's words.
pub struct BindLook {
    pub image: Handle<Image>,
    pub cut: Rect,
    pub size: [f32; 2],
    pub text_size: f32,
    pub color: Color,
    pub font: Handle<Font>,
}

/// The ground of a thing's view as the game colours it
/// (`ItemView.UpdateStaticInfo`): the database's colour of its kind
/// (`TaxonomyColorExtension.ColorMapping`) at 0.302.
pub fn item_ground(color: &str) -> Color {
    let [r, g, b] = match color {
        "blue" => [28, 65, 86],
        "yellow" => [104, 102, 40],
        "green" => [21, 45, 0],
        "red" => [109, 36, 24],
        "black" => [0, 0, 0],
        "grey" => [29, 29, 29],
        "violet" => [76, 42, 85],
        "orange" => [60, 25, 0],
        _ => [127, 127, 127],
    };
    Color::srgba_u8(r, g, b, 77)
}

/// A view's ground under the mouse (`ItemView.DefaultSelectedColor`).
pub const ITEM_SELECTED: Color = Color::srgba(1.0, 1.0, 1.0, 77.0 / 255.0);

/// The words a stack's count is in (`GridItemView.GetStackColor`):
/// rounds', money's and the rest's, by the database's names of the kind's
/// classes.
pub fn stack_color(class_names: &[String]) -> Color {
    let has = |name: &str| class_names.iter().any(|class| class == name);
    if has("Ammo") {
        Color::srgb_u8(0xda, 0xda, 0xbc)
    } else if has("Money") {
        Color::srgb_u8(0x8d, 0xb5, 0x00)
    } else {
        Color::srgb_u8(0xb6, 0xc1, 0xc7)
    }
}

/// Each item view's look made from its prefab, with its pictures and fonts.
pub(super) fn item_looks(look: &HealthLook) -> [Option<std::sync::Arc<ItemViewLook>>; 2] {
    let Some(items) = &look.items else { return [None, None] };
    [
        items.slot.as_ref().map(|panel| std::sync::Arc::new(item_look(look, panel, &look.slot_item))),
        items.grid.as_ref().map(|panel| std::sync::Arc::new(item_look(look, panel, &look.grid_item))),
    ]
}

/// A thing's view as the inventory draws it, from its prefab as a panel
/// and its pictures (`slot_layout`, `grid_layout`, the edit build screen's
/// `modding_selectable_layout`).
fn item_look(look: &HealthLook, panel: &trp_data::interface::screen::ScenePanel, drawn: &Drawn) -> ItemViewLook {
    {
        let caption = panel.find("Info Panel/Name").or_else(|| panel.find("Info Panel/Caption"));
        let value = (0..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Value" && panel.nodes[index].text.is_some());
        let font = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).map(|text| look.styled_font(panel, text.font, text.style)).unwrap_or_default();
        let border = panel.find("Border").filter(|&index| panel.nodes[index].active).and_then(|index| panel.nodes[index].image.map(|image| rgba(image.color)));
        let bind = (0..panel.nodes.len()).find(|&index| panel.nodes[index].scripts.iter().any(|(class, _)| class == "BindPanel")).and_then(|index| {
            let picture = drawn.pictures.get(index).copied().flatten()?;
            let image = look.images.get(picture.texture)?.clone()?;
            let text = panel.nodes.iter().skip(index + 1).find(|node| node.parent == Some(index) && node.text.is_some())?.text.clone()?;
            let size = [picture.cut.width(), picture.cut.height()];
            Some(BindLook { image, cut: picture.cut, size, text_size: text.size, color: rgba(text.color), font: look.font(panel, text.font) })
        });
        let sprite = |picture: &Picture, color: Color| -> Option<Sprite> {
            let image = look.images.get(picture.texture)?.clone()?;
            Some(Sprite { image, cut: picture.cut, size: picture.size, border: picture.border, color })
        };
        let drawn_sprite = |index: usize| -> Option<Sprite> {
            let picture = drawn.pictures.get(index).copied().flatten()?;
            sprite(&picture, panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color)))
        };
        let ends = |end: &str| (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end));
        let inscription = panel.find("Info Panel/Info Label").or_else(|| (0..panel.nodes.len()).find(|&index| panel.nodes[index].name == "Inscription" && panel.nodes[index].text.is_some()));
        let mastering = ends("MasteringIcon/Image").zip(ends("MasteringIcon/Level")).and_then(|(icon, level)| {
            let text = panel.nodes[level].text.as_ref()?;
            let mark = drawn_sprite(icon)?;
            // Laid out as `ValidateMastering` leaves the view of a weapon:
            // its slider's column on (`MasteringContainer`, first in the
            // bottom row, the bottom left column after it), at the view's
            // own size and a unit taller (the layout is linear in the
            // height, padding and anchors alone).
            let container = ends("MasteringContainer");
            let mut laid = panel.clone();
            if let Some(container) = container {
                laid.nodes[container].active = true;
            }
            let own = laid.nodes[0].layout.size_delta;
            let taller = [own[0], own[1] + 1.0];
            let stretching = |laid: &trp_data::interface::screen::ScenePanel, index: usize| {
                let (a, b) = (laid.rect(index, own), laid.rect(index, taller));
                Stretching { at: a, per: std::array::from_fn(|n| b[n] - a[n]), height: own[1] }
            };
            // The mark at its own size (the column's `Image`, sized by its
            // sprite: 22 by 29), the column's padding before it (-10) from
            // the bottom left column's left, at its bottom (the columns
            // laid from their lower left).
            let column = ends("BottomLeftLayoutGroup/MasteringIcon").and_then(|index| laid.nodes[index].parent.map(|parent| (index, parent)));
            let mark_at = column.map_or([0.0, 3.0], |(icon_row, column)| {
                let [x, y, _, h] = laid.rect(column, own);
                let pad = laid.nodes[icon_row].group.map_or(0.0, |group| group.padding[0]);
                [x + pad, own[1] - y - h]
            });
            // The slider: its track (`Background`, its whole rectangle) and
            // its fill (`Fill`, its anchors set by the slider to the share:
            // `Slider.UpdateVisuals`), laid out empty and full.
            let slider = container.and_then(|container| {
                let slider = (0..laid.nodes.len()).find(|&index| laid.nodes[index].parent == Some(container) && laid.nodes[index].scripts.iter().any(|(class, _)| class == "Slider"))?;
                let range = trp_data::interface::screen::slider_range(&laid, slider)?;
                let fill_index = range.fill?;
                let back = (0..laid.nodes.len()).find(|&index| laid.nodes[index].parent == Some(slider) && laid.nodes[index].name == "Background")?;
                let (axis, reversed) = (if range.direction >= 2 { 1 } else { 0 }, range.direction % 2 == 1);
                let mut filled = |share: f32| {
                    let layout = &mut laid.nodes[fill_index].layout;
                    // (The engine's anchors are y up: a slider from the
                    // bottom fills from anchor 0.)
                    let (mut min, mut max) = ([0.0, 0.0], [1.0, 1.0]);
                    if reversed {
                        min[axis] = 1.0 - share;
                    } else {
                        max[axis] = share;
                    }
                    (layout.anchor_min, layout.anchor_max) = (min, max);
                    stretching(&laid, fill_index)
                };
                let (empty, full) = (filled(0.0), filled(1.0));
                Some(MasteringSlider { track: drawn_sprite(back)?, fill: drawn_sprite(fill_index)?, track_at: stretching(&laid, back), empty, full, range })
            });
            Some(MasteringLook { mark, mark_at, size: text.size, color: rgba(text.color), font: look.font(panel, text.font), slider })
        });
        let durability = ends("DurabilityContaimer/Background").zip(ends("DurabilityContaimer/Fill Area/Fill")).and_then(|(track, fill)| Some((drawn_sprite(track)?, drawn_sprite(fill)?)));
        let mods = look.mod_pictures.iter().filter_map(|(key, picture)| Some((key.clone(), sprite(picture, Color::WHITE)?))).collect();
        // The repair buff's mark: its image's colour on each buff's icon,
        // its column's spacing.
        let buff_mark = ends("RepairBuffIcon").filter(|&index| panel.nodes[index].image.is_some());
        let buff_color = buff_mark.and_then(|index| panel.nodes[index].image.map(|image| rgba(image.color))).unwrap_or(Color::WHITE);
        let buff_icons = buff_mark
            .map(|_| look.attribute_icons.iter().filter(|(name, _)| name.ends_with("Buff")).filter_map(|(name, picture)| Some((name.clone(), sprite(picture, buff_color)?))).collect())
            .unwrap_or_default();
        let buff_spacing = buff_mark.map(|index| panel.nodes[index].parent.and_then(|column| panel.nodes[column].group).map_or(0.0, |group| group.spacing));
        let effect = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.text_effect(panel, text.font));
        let tmp = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.tmp_of(panel, text));
        let face = |index: Option<usize>| index.and_then(|index| panel.nodes[index].text.as_ref()).and_then(|text| look.tmp_faces.get(&(panel.file_of(text.font.0)?, text.font.1)).cloned());
        let tag = panel.find("TagPanel").zip(panel.find("TagPanel/TagName")).map(|(plate, name)| TagPlateLook {
            plate,
            name,
            name_tmp: tmp(Some(name)),
            name_font: font(Some(name)),
            name_face: face(Some(name)),
            caption_face: face(caption),
        });
        // The wish list's marks, and its column's other members' heights.
        let wish = ends("WishlistIcon").map(|icon| {
            let column = panel.nodes[icon].parent.unwrap_or(icon);
            let edges = (0..panel.nodes.len())
                .filter(|&index| panel.nodes[index].parent.is_some_and(|parent| panel.nodes[parent].name.starts_with("WishlistBorder")))
                .filter_map(|index| Some((index, drawn_sprite(index)?)))
                .collect();
            let mod_height = ends("ModIconContainer").map_or(0.0, |holder| panel.asks(holder, 1).1);
            let inscription_height = inscription
                .filter(|&index| panel.nodes[index].parent == Some(column))
                .and_then(|index| Some((panel.nodes[index].text.as_ref()?.size, face(Some(index))?)))
                .map(|(size, face)| face.font.height(size));
            let icons = look.wish_outlines.iter().filter_map(|(name, picture)| Some((*name, sprite(picture, Color::WHITE)?))).collect();
            WishLook { glow: ends("WishlistGlow").and_then(|glow| Some((glow, drawn_sprite(glow)?))), edges, icon, column, mod_height, inscription_height, icons }
        });
        // Its frame: the ground's place inside the view, read at a view of
        // a hundred units, and the border kept a pixel to a pixel.
        let frame = panel.find("Color Panel").zip(panel.find("Border")).and_then(|(ground, edge)| {
            let node = &panel.nodes[edge];
            if !node.active || !node.scripts.iter().any(|(class, _)| class == "PixelPerfectSpriteScaler") {
                return None;
            }
            let [x, y, w, h] = panel.placed(ground, [100.0, 100.0]).rect;
            Some(ViewFrame { inset: [x, y, 100.0 - x - w, 100.0 - y - h], border: drawn_sprite(edge)? })
        });
        let spin = look.spin.as_ref().map(|panel| {
            let piece = |end: &str| -> Option<(usize, SpinPicture)> {
                let index = (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end))?;
                let picture = look.spin_drawn.pictures.get(index).copied().flatten()?;
                let image = look.images.get(picture.texture)?.clone()?;
                Some((index, SpinPicture { image, cut: picture.cut, covers: picture.covers }))
            };
            SpinLook { panel: panel.clone(), back: piece("Spinner Background"), fill: piece("Spinner Fill"), loader: piece("LoadMagazineLoader"), stop: piece("CloseImage") }
        });
        let examine = look.examine.as_ref().map(|panel| {
            let piece = |end: &str| -> Option<(usize, Sprite)> {
                let index = (0..panel.nodes.len()).find(|&index| panel.nodes[index].path.ends_with(end))?;
                let picture = look.examine_drawn.pictures.get(index).copied().flatten()?;
                Some((index, sprite(&picture, panel.nodes[index].image.map_or(Color::WHITE, |image| rgba(image.color)))?))
            };
            let label = panel.find("ExamineProgressLabel").and_then(|index| {
                let text = panel.nodes[index].text.as_ref()?;
                Some((index, text.size, rgba(text.color), look.font(panel, text.font)))
            });
            ExamineLook { panel: panel.clone(), back: piece("ExamineSlider/Background"), fill: piece("Fill Area/Fill"), loader: piece("ExamineLoader"), label }
        });
        ItemViewLook {
            spin,
            examine,
            unexamined: ends("NotExaminedBackground").and_then(|index| Some((index, drawn_sprite(index)?))),
            unsearched: ends("UnsearchedBackground").and_then(|index| Some((index, drawn_sprite(index)?))),
            wish,
            tag,
            caption_tmp: tmp(caption),
            caption_face: face(caption),
            inscription_tmp: tmp(inscription),
            value_tmp: tmp(value),
            panel: panel.clone(),
            caption,
            value,
            caption_font: font(caption),
            value_font: font(value),
            border,
            bind,
            inscription,
            inscription_font: font(inscription),
            caption_effect: effect(caption),
            value_effect: effect(value),
            inscription_effect: effect(inscription),
            mastering,
            durability,
            mods,
            buff_icons,
            buff_spacing,
            loader: panel.find("Small Loader").and_then(|index| Some((drawn_sprite(index)?, panel.nodes[index].layout.size_delta, drawn.pictures.get(index).copied().flatten()?.covers))),
            drain: panel.find("Loader").and_then(|index| Some((drawn_sprite(index)?, panel.nodes[index].layout.size_delta, drawn.pictures.get(index).copied().flatten()?.covers))),
            frame,
        }
    }
}

/// The panels' textures made images, and their fonts fonts, once, at the
/// start.
pub(super) fn make_pictures(mut look: ResMut<HealthLook>, mut images: ResMut<Assets<Image>>, mut fonts: ResMut<Assets<Font>>) {
    make_fonts(&mut look, &mut fonts);
    let textures = std::mem::take(&mut look.textures);
    look.images = textures.into_iter().map(|texture| make_image(texture, &mut images)).collect();
    finish_looks(&mut look);
}

/// The fonts read and not yet made (`font_data`), made.
pub(super) fn make_fonts(look: &mut HealthLook, fonts: &mut Assets<Font>) {
    for (asset, data) in std::mem::take(&mut look.font_data) {
        match Font::try_from_bytes(data) {
            Ok(font) => {
                let handle = fonts.add(font);
                look.fonts.insert(asset, handle);
            }
            Err(e) => eprintln!("a font of the game's could not be made; the app's is used: {e}"),
        }
    }
}

/// A picture read, made into the engine's (`images`).
pub(super) fn make_image(texture: Option<trp_data::unity::extract::TextureData>, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let texture = texture?;
    match crate::assets::render_data::build_image(&texture, crate::assets::render_data::Role::Color) {
        // Sampled as the engine samples it: a texture set to point
        // filtering at its nearest pixel (the frames of a pixel or
        // two, whose middles are clear); and clamped at its edges
        // where it is set so (`TextureWrapMode.Clamp`), as the
        // interface's mostly are: repeated, a picture's edge took
        // in its far side's (a faint line along the top of the left
        // panel's foot, its bottom row's light).
        Ok(mut image) => {
            use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
            let wrap = |clamp: bool| if clamp { ImageAddressMode::ClampToEdge } else { ImageAddressMode::Repeat };
            let filter = if texture.point { ImageFilterMode::Nearest } else { ImageFilterMode::Linear };
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: wrap(texture.clamp[0]),
                address_mode_v: wrap(texture.clamp[1]),
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: ImageFilterMode::Linear,
                ..default()
            });
            Some(images.add(image))
        }
        Err(e) => {
            eprintln!("the health panel's picture {} could not be made: {e:#}", texture.name);
            None
        }
    }
}

/// What is made of the look once its pictures are: a thing's views, the
/// edit build screen's view of a part.
pub(super) fn finish_looks(look: &mut HealthLook) {
    let looks = item_looks(look);
    look.item_looks = looks;
    look.quick_value = quick_value_look(look);
    // The edit build screen's view of a part.
    let part_look = look.edit_build.as_ref().and_then(|eb| eb.part.as_ref()).map(|(panel, drawn)| std::sync::Arc::new(item_look(look, panel, drawn)));
    if let Some(eb) = look.edit_build.as_mut() {
        eb.part_look = part_look;
    }
}

/// The game's line of its version at the screen's foot.
#[derive(Component)]
pub(super) struct VersionLine(u32);

/// Draws the game's line of its version (`TARKOV UI`) along the screen's
/// foot, over every screen as the game's preloader draws it: its text's
/// look and place as the scene has them (Bender at 16, its colour at a
/// third, 12 in from the left, in a band 31.5 high just over the foot), the
/// words the app's own (`TRP` and its version, where the game's say its
/// build and SPT's its own: the app's choice). Made afresh
/// as the window's height changes.
pub(super) fn show_version(mut commands: Commands, look: Res<HealthLook>, window: Query<&Window, With<bevy::window::PrimaryWindow>>, shown: Query<(Entity, &VersionLine)>) {
    let Some((label, below)) = look.version.as_ref().filter(|_| look.tarkov_ui) else { return };
    let Ok(window) = window.single() else { return };
    let height = window.height().round() as u32;
    if height == 0 || shown.iter().any(|(_, line)| line.0 == height) || look.fonts.is_empty() {
        return;
    }
    for (entity, _) in &shown {
        commands.entity(entity).despawn();
    }
    let Some(text) = label.nodes.first().and_then(|node| node.text.as_ref()) else { return };
    let s = height as f32 / 1080.0;
    let band = label.nodes[0].layout.size_delta[1];
    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        bottom: Val::Px(below * s),
        width: Val::Percent(100.0),
        height: Val::Px(band * s),
        padding: UiRect { left: Val::Px(text.margin[0] * s), ..default() },
        align_items: AlignItems::Center,
        ..default()
    };
    let font = TextFont { font: look.styled_font(label, text.font, text.style), font_size: text.size * s, weight: weight_of(text.style), ..default() };
    let words = format!("TRP {}", env!("TRP_VERSION"));
    commands.spawn((VersionLine(height), node, GlobalZIndex(30))).with_children(|line| {
        line.spawn((Text::new(words), font, TextColor(rgba(text.color)), TextLayout::new_with_no_wrap()));
    });
}

/// The game's views of a thing handed to the inventory, which draws every
/// thing by them with `TARKOV UI` on.
pub(super) fn share_item_looks(look: Res<HealthLook>, mut carried: ResMut<crate::screens::inventory_view::Carried>) {
    share_looks(&look, &mut carried);
}

fn share_looks(look: &HealthLook, carried: &mut crate::screens::inventory_view::Carried) {
    carried.item_looks = look.item_looks.clone();
    carried.info_size = info_places(look).map(|places| Vec2::from(places.size));
    carried.game_grid_window = look.grid_window.is_some();
    carried.tag_size = tag_places(look).map(|places| Vec2::from(places.size));
    carried.tag_limit = if look.tag_window.is_some() { look.tag_limit } else { TAG_LIMIT };
}

/// The rest of the screens being read on their thread (`HealthLook::read`,
/// main's "the screens"), after the battle screen's part the start was
/// given: its look, and the install's words for its texts' keys
/// (`HealthLook::keys`). The warm-up waits while it is here, so that the
/// inventory, the windows over it and the edit build screen are whole from
/// the run's first frame, as they were when the start read them all, and
/// their pictures go up to the graphics card under the curtain.
#[derive(Resource)]
pub struct ScreensComing(pub Option<std::thread::JoinHandle<(HealthLook, HashMap<String, String>)>>);

/// Merges the rest of the screens in once their thread is done
/// (`HealthLook::merge_rest`): its pictures and fonts made, the things'
/// views shared with what is carried, its words added.
#[allow(clippy::too_many_arguments)]
pub(super) fn receive_screens(
    mut commands: Commands,
    coming: Option<ResMut<ScreensComing>>,
    mut look: ResMut<HealthLook>,
    mut words: ResMut<Words>,
    mut carried: ResMut<crate::screens::inventory_view::Carried>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
) {
    let Some(mut coming) = coming else { return };
    if coming.0.as_ref().is_some_and(|thread| !thread.is_finished()) {
        return;
    }
    commands.remove_resource::<ScreensComing>();
    let Some(thread) = coming.0.take() else { return };
    let began = std::time::Instant::now();
    match thread.join() {
        Ok((rest, texts)) => {
            look.merge_rest(rest, &mut images, &mut fonts);
            words.0.extend(texts);
            share_looks(&look, &mut carried);
            info!("the inventory's screens: read away from the frame, merged in {:.1} ms", began.elapsed().as_secs_f32() * 1000.0);
        }
        Err(_) => eprintln!("the screens' thread failed after the battle screen's part; the inventory is drawn the app's way"),
    }
}

/// What the screens' read has read of fonts so far, so that its second
/// part reads none again (`HealthLook::read_fonts`): the font assets and
/// the materials tried, and each file's fonts.
#[derive(Default)]
pub(super) struct FontsRead {
    pub(super) tried: std::collections::HashSet<(String, i64)>,
    pub(super) materials: std::collections::HashSet<(String, i64)>,
    pub(super) kept: HashMap<String, Vec<(String, Vec<u8>)>>,
}

/// The font assets of the texts in these panels, and the materials the
/// texts name for themselves, where they do (`HealthLook::read_fonts`).
pub(super) type FontAssets = (std::collections::BTreeSet<(String, i64)>, std::collections::BTreeSet<(String, i64)>);

pub(super) fn font_assets(panels: &[&trp_data::interface::screen::ScenePanel]) -> FontAssets {
    let assets = panels.iter().flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| Some((scene.file_of(text.font.0)?, text.font.1))))).collect();
    let materials = panels.iter().flat_map(|scene| scene.nodes.iter().filter_map(|node| node.text.as_ref().and_then(|text| text.material).and_then(|(file, id)| Some((scene.file_of(file)?, id))))).collect();
    (assets, materials)
}

/// The colours a tag may be (`EditTagWindow.GetColor`: the game's own
/// seven, a number past them the first).
pub const TAG_COLOURS: [[f32; 3]; 7] = [[0.4, 0.0, 1.0 / 51.0], [0.4, 0.227_450_98, 3.0 / 85.0], [24.0 / 85.0, 0.4, 1.0 / 17.0], [1.0 / 85.0, 0.4, 28.0 / 85.0], [0.168_627_46, 0.0, 20.0 / 51.0], [0.4, 0.0, 0.227_450_98], [0.4, 0.4, 0.4]];

pub(super) const TAG_COLORS: usize = TAG_COLOURS.len();
