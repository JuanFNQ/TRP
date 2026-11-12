//! The frame's systems: the start-up's, and the Update chain from the input to the warm-up.

use super::*;

/// Adds the start-up systems and the frame's chain of systems, in their order.
pub(super) fn add(mut app: App) -> App {
    app
        .add_systems(Startup, (setup_world, world::range::spawn_labels, (weapons::weapon::spawn_weapon, player::body_view::spawn_body).chain(), platform::run_log::graphics_card))
        .add_systems(PreStartup, platform::run_log::startup_begun)
        .add_systems(PostStartup, platform::run_log::startup_ended)
        .add_systems(First, platform::run_log::note_long_frames)
        .add_systems(Last, platform::memory::note_memory)
        .add_systems(First, weapons::weapon::attach_sounds)
        .add_systems(
            Update,
            (
                // The skills' buffs put on again when a level changes.
                (mark_frame, platform::keys::hear, weapons::weapon::prepare_rigs, weapons::weapon::free_rigs, player::skills::host_levels, player::skills::stimulated, player::skills::apply, player::skills::mastery_drawn, player::skills::mastery_apply).chain(),
                ((switch_view, platform::cursor::hold, platform::cursor::keep_alt, platform::cursor::point).chain(), platform::window_icon::set),
                // The magazine picker hears its keys before the hands do.
                (screens::ammo_picker::hear, player::controller::gather_input).chain().run_if(running),
                (player::controller::step.run_if(running), player::feet_down::ground_feet).chain(),
                fly_camera,
                player::controller::apply,
                (show_things, world::range::place_labels),
                player::animation::play_clips,
                player::animation::pose_rig,
                player::hands_view::apply_hands,
                player::arms::place_hands,
                (player::body_view::show_outfit, player::body_view::show_torso, player::body_view::show_upper_body).chain(),
                (player::body_view::pose_body, (player::slung_view::hang, player::slung_view::show).chain().run_if(resource_exists::<player::slung_view::Slung>)).chain(),
                (player::body_view::see_collarbones, player::body_view::keep_collarbones, player::body_view::see_outfit, player::body_view::pose_outfit).chain(),
                // A death, after the body is drawn for the frame; and
                // things thrown and let fall, stepped with the corpse.
                (player::death::die, player::death::follow, player::death::fade, player::death::begin_again, world::loose::set_going, world::loose::read_models, world::loose::fly, weapons::magazine_rounds::keep_chamber_kinds, weapons::cases::fly_cases, (weapons::bullets::fly_bullets, effects::explosion_view::impacts.run_if(resource_exists::<effects::explosion_view::Explosions>), effects::decal_view::lay_holes.run_if(resource_exists::<effects::decal_view::BulletHoles>), audio::impact_sound::play_impacts.run_if(resource_exists::<audio::impact_sound::ImpactSounds>)).chain().run_if(resource_exists::<weapons::bullets::Bullets>), effects::blast_view::grenade_blasts, audio::blast_sound::play_blasts.run_if(resource_exists::<audio::blast_sound::BlastSounds>), audio::tinnitus::ring.run_if(resource_exists::<audio::tinnitus::Tinnitus>), (effects::contusion_view::hits, effects::contusion_view::update, effects::contusion_view::eye_burn, effects::blood_view::blood_frame).chain().run_if(resource_exists::<effects::contusion_view::EyeBurnState>),(weapons::muzzle_view::muzzle_flash, weapons::muzzle_view::muzzle_light).chain().run_if(resource_exists::<weapons::muzzle_view::MuzzleViews>), (weapons::magazine_rounds::find_hands_magazine, weapons::magazine_rounds::show_rounds, weapons::magazine_rounds::find_hands_chamber, weapons::magazine_rounds::show_chambered).chain().run_if(resource_exists::<weapons::magazine_rounds::NextParity>)).chain(),
                panels::hud::debug,
                log_frame.run_if(running),
                log_pacing.run_if(running),
                capture.run_if(running),
                warm_up,
            )
                .chain(),
        );
    app
}
