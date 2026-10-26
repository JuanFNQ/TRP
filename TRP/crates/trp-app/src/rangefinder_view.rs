//! The handheld rangefinder's readout (rangefinder spec, "The readout"):
//! the ray it measures with, cast when its controller asks, from its
//! `optic_camera` object along that object's forward, and the number drawn
//! into the optic camera's picture as the prefab's canvas draws it.

use bevy::camera::visibility::Visibility;
use bevy::prelude::*;
use bevy::ui::UiTargetCamera;

use crate::optic_view::{OpticCamera, Pip};
use crate::range::Range;
use crate::weapon::{HandsRig, WeaponRig};

/// The readout's canvas (`RangeFinder`: screen space, camera, scaled to a
/// 1024-pixel square, which the optic picture is) and its text (`Text
/// (TMP)`: anchored at the middle, moved 35 right and 194 up, a box of 200
/// by 50, centred both ways).
const CANVAS: f32 = 1024.0;
const TEXT_OFFSET: [f32; 2] = [35.0, 194.0];
const TEXT_BOX: [f32; 2] = [200.0, 50.0];
/// Its size and colour (`m_fontSize` 125, `m_fontColor`).
const TEXT_SIZE: f32 = 125.0;
const TEXT_COLOUR: [f32; 3] = [1.0, 0.0, 0.036];
/// Its font: `Bender Overlay SDF`, italic (style 2). `PROVISIONAL`: drawn
/// with the build's Bender Italic file; TextMeshPro may slant the regular
/// face instead (its asset's weight table not read for this one).
const FONT: &str = "Jovanny Lemonad - Bender Italic";

pub struct RangefinderViewPlugin;

impl Plugin for RangefinderViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (measure, show_readout).chain().after(crate::hands_view::apply_hands));
    }
}

/// The readout's text node, once made.
#[derive(Component)]
struct ReadoutText;

/// Casts the readout's ray where the rangefinder asks for one: from its
/// optic object, placed in the pivot's frame as the optic camera is
/// (`optic_view::place_camera`), along its forward, as far as
/// `READOUT_REACH`, against the range's boxes, doors and ground, the
/// dummies' hit colliders and the poppers (the mask's layers 9, 11, 12, 16
/// and 23; `PROVISIONAL`: the app's own things stand in for them, the
/// poppers among them).
#[allow(clippy::too_many_arguments)]
fn measure(
    rig: Option<Res<WeaponRig>>,
    mut hands: Option<ResMut<HandsRig>>,
    range: Res<Range>,
    bullets: Res<crate::bullets::Bullets>,
    dummies: Option<Res<crate::dummy::Dummies>>,
    poppers: Option<Res<crate::poppers::Poppers>>,
    transforms: Query<&Transform>,
    globals: Query<&GlobalTransform>,
    parents: Query<&ChildOf>,
) {
    let (Some(rig), Some(hands)) = (rig, hands.as_deref_mut()) else { return };
    if !hands.rangefinder.as_ref().is_some_and(|r| r.wants_ray()) {
        return;
    }
    let place = hands.hands.sights().and_then(|sights| {
        let (part, mode) = sights.live_optic()?;
        let node = sights.parts().get(part)?.modes.get(mode)?.optic.as_ref()?.camera?;
        let object = *rig.bones.get(node)?;
        let root = transforms.get(rig.root).ok()?;
        let within = crate::hands_view::in_root(object, rig.root, |e| transforms.get(e).ok().copied(), &parents)?;
        let place = Transform { scale: Vec3::ONE, ..*root } * within;
        let pivot = globals.get(rig.pivot).ok()?;
        let world = pivot.mul_transform(Transform { scale: Vec3::ONE, ..place });
        Some((world.translation(), world.forward().as_vec3()))
    });
    let distance = place.and_then(|(from, way)| {
        let to = from + way * trp_sim::rangefinder::READOUT_REACH;
        let (from, to) = ([from.x, from.y, -from.z], [to.x, to.y, -to.z]);
        let mut hits = Vec::new();
        bullets.sight_hits(&range, from, to, &mut hits);
        if let Some(dummies) = &dummies {
            dummies.ray(from, to, &mut hits);
        }
        if let Some(poppers) = &poppers {
            poppers.ray(from, to, &mut hits);
        }
        hits.iter().map(|hit| hit.distance).min_by(f32::total_cmp)
    });
    if let Some(rangefinder) = hands.rangefinder.as_mut() {
        rangefinder.take_distance(distance);
    }
}

/// The readout drawn into the optic camera's picture while its optic is on
/// (`RangeFinder` active only then): made the first time, on that camera,
/// with the build's font. With `PIP` off there is no such picture, and no
/// readout (`PROVISIONAL`).
#[allow(clippy::too_many_arguments)]
fn show_readout(
    mut commands: Commands,
    pip: Res<Pip>,
    game: Res<crate::GameData>,
    hands: Option<Res<HandsRig>>,
    camera: Query<Entity, With<OpticCamera>>,
    mut text: Query<(&mut Text, &mut Visibility), With<ReadoutText>>,
    mut fonts: ResMut<Assets<Font>>,
    mut made: Local<bool>,
) {
    if !pip.0 {
        return;
    }
    let shown = hands.as_ref().and_then(|hands| hands.rangefinder.as_ref()).filter(|r| r.optic_on());
    if let Ok((mut words, mut visibility)) = text.single_mut() {
        let seen = if shown.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != seen {
            *visibility = seen;
        }
        if let Some(rangefinder) = shown.filter(|r| r.readout.text != words.0) {
            words.0 = rangefinder.readout.text.clone();
        }
        return;
    }
    if *made || shown.is_none() {
        return;
    }
    *made = true;
    let Ok(camera) = camera.single() else { return };
    let font = trp_data::screen::build_fonts(&game.unity.install, "resources.assets")
        .ok()
        .and_then(|read| read.into_iter().find(|(name, _)| name == FONT))
        .and_then(|(_, bytes)| Font::try_from_bytes(bytes).ok())
        .map(|font| fonts.add(font))
        .unwrap_or_default();
    // The box's corner in the picture, down from its top (UI's axes).
    let left = CANVAS / 2.0 + TEXT_OFFSET[0] - TEXT_BOX[0] / 2.0;
    let top = CANVAS / 2.0 - TEXT_OFFSET[1] - TEXT_BOX[1] / 2.0;
    commands
        .spawn((
            Name::new("Rangefinder readout"),
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            UiTargetCamera(camera),
        ))
        .with_children(|canvas| {
            canvas
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(left),
                        top: Val::Px(top),
                        width: Val::Px(TEXT_BOX[0]),
                        height: Val::Px(TEXT_BOX[1]),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        overflow: Overflow::visible(),
                        ..default()
                    },
                ))
                .with_children(|holder| {
                    holder.spawn((
                        ReadoutText,
                        Text::new(trp_sim::rangefinder::NO_DISTANCE),
                        TextFont { font, font_size: TEXT_SIZE, ..default() },
                        TextColor(Color::srgb(TEXT_COLOUR[0], TEXT_COLOUR[1], TEXT_COLOUR[2])),
                        TextLayout::new(Justify::Center, LineBreak::NoWrap),
                        Visibility::Inherited,
                    ));
                });
        });
}
