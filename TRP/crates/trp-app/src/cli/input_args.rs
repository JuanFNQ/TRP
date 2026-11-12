//! The command line's arguments of scripted input and of the view: keys, mouse, drags, the camera and the look.

use super::*;

/// Takes an argument of scripted input or of the view; false if it is none of them.
pub(super) fn input(options: &mut Options, arg: &str, args: &mut impl Iterator<Item = String>) -> bool {
    match arg {
        "--wheel" => {
            let given = args.next().unwrap_or_default();
            match given.split_once(':').and_then(|(frame, notches)| Some((frame.parse().ok()?, notches.parse().ok()?))) {
                Some(turn) => options.wheel.push(turn),
                None => eprintln!("ignoring --wheel {given}: wanted <frame>:<notches>"),
            }
        }
        "--scroll" => {
            let given = args.next().unwrap_or_default();
            match given.split_once(':').and_then(|(frame, notches)| Some((frame.parse().ok()?, notches.parse().ok()?))) {
                Some(turn) => options.scroll.push(turn),
                None => eprintln!("ignoring --scroll {given}: wanted <frame>:<notches>"),
            }
        }
        "--zoom" => {
            let given = args.next().unwrap_or_default();
            match given.split_once(':').and_then(|(frame, notches)| Some((frame.parse().ok()?, notches.parse().ok()?))) {
                Some(turn) => options.zoom.push(turn),
                None => eprintln!("ignoring --zoom {given}: wanted <frame>:<notches>"),
            }
        }
        "--damage" => {
            let given = args.next().unwrap_or_default();
            let mut parts = given.split(':');
            let frame = parts.next().and_then(|v| v.parse().ok());
            let part = parts.next().and_then(part_named);
            match (frame, part, parts.next().and_then(|v| v.parse().ok())) {
                // And, as a round would, the stamina it burns a point
                // (`StaminaBurnPerDamage`), nought unless given.
                (Some(frame), Some(part), Some(amount)) => {
                    options.damage.push((frame, part, amount, parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0)))
                }
                _ => eprintln!("--damage takes a frame, a part and an amount: {given}"),
            }
        }
        "--poison" => {
            let given = args.next().unwrap_or_default();
            let mut parts = given.split(':');
            let frame = parts.next().and_then(|v| v.parse().ok());
            let part = parts.next().and_then(part_named);
            match (frame, part, parts.next().and_then(|v| v.parse().ok())) {
                (Some(frame), Some(part), Some(amount)) => options.poison.push((frame, part, amount)),
                _ => eprintln!("--poison takes a frame, a part and an amount: {given}"),
            }
        }
        "--tap" => {
            let given = args.next().unwrap_or_default();
            let mut parts = given.split(':');
            let frame = parts.next().and_then(|v| v.parse().ok());
            let key = parts.next().and_then(platform::keys::tapped);
            match (frame, key) {
                (Some(frame), Some(key)) => options.taps.push((frame, key, parts.next().and_then(|v| v.parse().ok()).unwrap_or(2))),
                _ => eprintln!("--tap takes a frame and a key joined by a colon: a letter, a digit, num0 to num9, numenter, tab, delete, backspace, space, ctrl, alt, shift, caps, enter, escape, backquote, f1, f7 to f9, pageup, pagedown, home, end, left, right, up, down, lmb, rmb, mmb, mouse3 or mouse4 (docs/manual.md)"),
            }
        }
        "--mouse" => {
            let given = args.next().unwrap_or_default();
            let numbers: Vec<f32> = given.split(':').filter_map(|v| v.parse().ok()).collect();
            match numbers[..] {
                [frame, frames, across, down] => options.mouse.push((frame as u32, frames as u32, Vec2::new(across, down))),
                _ => eprintln!("ignoring --mouse {given}: wanted <frame>:<frames>:<counts across a frame>:<counts down a frame>"),
            }
        }
        "--drag-at" => options.drag_at = args.next().and_then(|frame| frame.parse().ok()).unwrap_or(options.drag_at),
        "--move" => {
            let numbers: Vec<f32> = args.next().unwrap_or_default().split(':').filter_map(|v| v.parse().ok()).collect();
            match numbers[..] {
                [frame, x, y] => options.moves.push((frame as u32, x, y)),
                _ => eprintln!("--move takes three numbers joined by colons: the drawn frame, x, y"),
            }
        }
        "--drag" => {
            let numbers: Vec<f32> = args.next().unwrap_or_default().split(':').filter_map(|v| v.parse().ok()).collect();
            match numbers[..] {
                [x, y, to_x, to_y] => options.drags.push([x, y, to_x, to_y]),
                _ => eprintln!("--drag takes four numbers joined by colons: from x, from y, to x, to y"),
            }
        }
        "--view" => {
            options.view = match args.next().as_deref() {
                Some("side") => View::Free,
                // The whole figure from three metres off, for captures.
                Some("body") => {
                    options.whole = true;
                    View::Free
                }
                // The whole figure likewise, with the keys the
                // character's: a capture that presses keys of its own
                // (`--tap`) is heard as a played run's.
                Some(watch) if watch == "watch" || watch.starts_with("watch:") => {
                    options.whole = true;
                    options.watch_turn = watch.strip_prefix("watch:").and_then(|d| d.parse().ok()).unwrap_or(0.0);
                    View::Watched
                }
                // The range from overhead, ahead at the top; `map:<metres>`
                // from that height.
                Some(map) if map.starts_with("map") => {
                    options.map = Some(map.strip_prefix("map:").and_then(|h| h.parse().ok()).unwrap_or(30.0));
                    View::Free
                }
                _ => View::FirstPerson,
            }
        }
        // The pose and weapon motion of the character's own eyes or of
        // another's, whatever the view: `--pov first --view side` is
        // the first-person rig looked at from outside.
        "--pov" => {
            options.pov = match args.next().as_deref() {
                Some("first") => Some(trp_sim::PointOfView::FirstPerson),
                Some("third") => Some(trp_sim::PointOfView::ThirdPerson),
                other => {
                    eprintln!("--pov takes first or third, not {other:?}");
                    None
                }
            }
        }
        "--play" => options.play.extend(args.next()),
        "--at" => options.hold_at = args.next().and_then(|v| v.parse().ok()),
        "--hold" => options.hold = args.next(),
        "--fov" => {
            options.field_of_view = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[0]).clamp(50.0, 75.0);
            options.fov_given = true;
        }
        "--head-bobbing" => {
            options.head_bobbing = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[1]).clamp(0.2, 1.0)
        }
        "--sensitivity" => {
            options.sensitivity = args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[2]).clamp(0.1, 5.0);
            options.sensitivity_given[0] = true;
        }
        "--aiming-sensitivity" => {
            options.aiming_sensitivity =
                args.next().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_VIEW[3]).clamp(0.1, 5.0);
            options.sensitivity_given[1] = true;
        }
        "--pitch" => {
            options.pitch = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32).clamp(-90.0, 90.0);
            options.view_given = true;
        }
        "--yaw" => {
            options.yaw = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0f32);
            options.view_given = true;
        }
        "--hour" => options.hour = args.next().and_then(|v| v.parse().ok()).map(|h: f32| h.rem_euclid(24.0)),
        _ => return false,
    }
    true
}
