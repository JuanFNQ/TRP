//! The command line's arguments of the world, the graphics and the run's outputs: captures, logs and the window.

use super::*;

/// Takes an argument of the world, the graphics or the run's outputs; false if it is none of them.
pub(super) fn display(options: &mut Options, arg: &str, args: &mut impl Iterator<Item = String>) -> bool {
    match arg {
        "--cloudiness" => options.cloudiness = args.next().and_then(|v| v.parse().ok()).map(|c: f32| c.clamp(-1.0, 1.0)),
        "--start" => options.start = args.next(),
        "--grid" => options.grid = args.next().as_deref() != Some("off"),
        "--alt-menu" => options.alt_menu = true,
        "--fps" => options.show_fps = args.next().as_deref() != Some("off"),
        "--hud" => {
            let given = args.next().unwrap_or_default();
            match given.as_str() {
                "always" => options.hud = Some(panels::battle_view::Shows::Always),
                "autohide" => options.hud = Some(panels::battle_view::Shows::Autohide),
                "never" => options.hud = Some(panels::battle_view::Shows::Never),
                _ => eprintln!("--hud takes always, autohide or never: {given}"),
            }
        }
        "--custom-hands" => options.custom_hands = Some(args.next().as_deref() != Some("off")),
        "--clock" => {
            let given = args.next().unwrap_or_default();
            options.clock = player::watch_view::clock_of(&given);
            if options.clock.is_none() {
                eprintln!("--clock takes a time of day as HH:MM or HH:MM:SS: {given}");
            }
        }
        "--smooth-animations" => options.smooth_animations = Some(args.next().as_deref() != Some("off")),
        "--icons" => options.icon_generation = Some(args.next().as_deref() != Some("off")),
        "--texture-quality" => options.texture_quality = args.next().and_then(|quality| quality.parse::<u8>().ok()).map(|quality| quality.min(2)),
        "--mip-streaming" => options.mip_streaming = Some(args.next().as_deref() != Some("off")),
        "--shadow-quality" => options.shadow_quality = args.next().and_then(|quality| quality.parse::<u8>().ok()).map(|quality| quality.min(3)),
        "--ssao" => {
            let given = args.next().unwrap_or_default();
            options.ssao = trp_sim::lighting::hbao::SsaoMode::from_key(&given);
            if options.ssao.is_none() {
                eprintln!("--ssao takes one of {}: {given}", trp_sim::lighting::hbao::SsaoMode::ALL.map(trp_sim::lighting::hbao::SsaoMode::key).join(", "));
            }
        }
        "--ssao-display" => options.ssao_ao_only = args.next().as_deref() == Some("ao"),
        "--fps-limit" => {
            let given = args.next().unwrap_or_default();
            options.fps_limit = match given.as_str() {
                "off" => Some(None),
                number => match number.parse::<u32>() {
                    Ok(fps) => Some(Some(fps)),
                    Err(_) => {
                        eprintln!("--fps-limit takes frames a second (30 to 144) or off: {given}");
                        None
                    }
                },
            };
        }
        "--dust-resolution" => {
            let given = args.next().unwrap_or_default();
            options.dust_resolution = effects::offscreen_dust::DustResolution::from_key(&given);
            if options.dust_resolution.is_none() {
                eprintln!("--dust-resolution takes full, half or quarter: {given}");
            }
        }
        "--msaa" => {
            let given = args.next().unwrap_or_default();
            options.msaa = match given.as_str() {
                "off" | "1" => Some(1),
                "2" | "2x" => Some(2),
                "4" | "4x" => Some(4),
                _ => {
                    eprintln!("--msaa takes off, 2 or 4: {given}");
                    None
                }
            };
        }
        "--feature" => {
            let given = args.next().unwrap_or_default();
            match given.split_once('=') {
                Some((name, on)) => options.feature_switches.push((name.to_owned(), on != "off")),
                None => eprintln!("--feature takes a switch's name and on or off: {given}"),
            }
        }
        // Taken before the rest (`setup::wanted`).
        "--setup" => {}
        "--fixed-dt" => options.fixed_dt = args.next().and_then(|v| v.parse().ok()).filter(|ms: &f32| *ms > 0.0),
        "--frame-rate" => {
            options.frame_rate = args.next().and_then(|v| v.parse().ok()).unwrap_or(trp_sim::REFERENCE_FRAME_RATE).clamp(30.0, 480.0)
        }
        "--log-frames" => options.log_frames = args.next().map(PathBuf::from),
        "--level" => options.level = args.next().map(PathBuf::from),
        "--export-level" => options.export_level = args.next().map(PathBuf::from),
        "--log-pacing" => options.log_pacing = args.next().map(PathBuf::from),
        "--turn" => options.turn = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
        "--scenario" => {
            let file = PathBuf::from(args.next().unwrap_or_default());
            match trp_sim::recordings::script::Scenario::load(&file) {
                Ok(scenario) => options.scenario = Some(scenario),
                Err(e) => eprintln!("ignoring the scenario: {e:#}"),
            }
        }
        "--scenario-at" => options.scenario_at = args.next().and_then(|v| v.parse().ok()).unwrap_or(2.0f32).max(0.0),
        "--present" => options.present = args.next().unwrap_or_default(),
        "--backend" => options.backend = args.next(),
        "--render-thread" => options.render_thread = Some(args.next().as_deref() != Some("off")),
        "--screenshot" => options.screenshot = args.next().map(PathBuf::from),
        "--screenshot-crop" => {
            options.screenshot_crop = args.next().and_then(|v| {
                let n: Vec<u32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                (n.len() == 4).then(|| [n[0], n[1], n[2], n[3]])
            })
        }
        "--sound-out" => options.sound_out = args.next().map(PathBuf::from),
        "--log-sounds" => options.log_sounds = args.next().map(PathBuf::from),
        "--dump-ui" => options.dump_ui = args.next().map(PathBuf::from),
        "--dump-ui-from" => options.dump_ui_from = args.next().and_then(|v| v.parse().ok()).unwrap_or(0),
        "--screenshot-frames" => options.screenshot_count = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
        "--screenshot-frame" => {
            options.screenshot_frame = args.next().and_then(|v| v.parse().ok()).unwrap_or(90)
        }
        "--figure-hold" => options.figure_hold = args.next().and_then(|v| v.parse().ok()).filter(|s: &f32| *s >= 0.0),
        "--figure-pick" => {
            let given = args.next().unwrap_or_default();
            match screens::figure_view::parse_pick(&given) {
                Some(pick) => options.figure_picks.push(pick),
                None => eprintln!("--figure-pick {given:?}: idle, random:<1-4> or move:<index>"),
            }
        }
        "--resolution" => {
            let given = args.next().unwrap_or_default();
            match given.split_once('x').and_then(|(width, height)| Some([width.parse::<u32>().ok()?, height.parse::<u32>().ok()?])) {
                Some([width, height]) => options.window = [width.max(640), height.max(360)],
                None => eprintln!("--resolution takes a width and a height joined by an x: {given}"),
            }
        }
        _ => return false,
    }
    true
}
