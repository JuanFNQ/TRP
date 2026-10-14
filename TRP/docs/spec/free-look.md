# The head's free look

The camera's own turn while a key is held, apart from the body's and the weapon's, and its way back when the key comes up. Written from a reading of the game's code (0.16.9.5, SPT 4.1.6 assembly), its settings object and the install's `Control.ini`; nothing here is copied from them. Class and field names are the game's. Read and built on 2026-10-13.

## How the mouse comes to the head (`InputAxisCombination`, `InputPreset.UpdateBindings`)

The input manager has seven axes. Four of them are bound to the mouse: `TurnX` and `TurnY`, which turn the view, and `LookX` and `LookY`, which turn the head. The look's two are made with a **modifier**, the binding `Nidnod` (the game's word for it is "Free look"; the middle mouse button in the install read, bound to its release): for each variant of that binding that has keys, a pair of look axes is made with those keys as the keys that give them the mouse. A binding with no keys has no free look.

Each of the look's two axes goes through two states by itself:

- **Idle.** On a frame the mouse moves along the axis with the modifier's keys held (going down or held), the axis takes the mouse's axis for itself and **gives nothing on that frame**.
- **Has the mouse.** On each frame the mouse moves along the axis and the keys are not up, it gives the travel. On a frame the mouse does not move along that axis, or the keys are up or coming up, it lets the mouse's axis go and is idle again.

The turn's axis is given the travel only on a frame the look has not taken the mouse's axis (the look, of two inputs, comes before the turn, of one, and a combination gives way to one of as many inputs or more). So:

- **the first frame of each run of travel along an axis is given to neither the turn nor the look**, and a run ends at every frame with no travel along that axis: a slow hand loses a good part of its travel, and each axis loses its own frames;
- on the frame the key comes up the travel is the turn's again.

The look's axes give way, for their modifier's keys, to any combination of as many inputs or more that has one of those keys, but never to the look's own binding nor to blind fire's (`IsLowerPriority`).

**The scale.** A look axis reads the mouse as the turn's does: the engine's halving of a raw count, the engine axis's own factor (0.05) and the binding's (`LookX`, `LookY`: 5 in the install read), so **0.125 degrees a count**. The player's sensitivity is not in it (`MoveInputTranslator.TranslateAxes` hands the two axes to `Player.Look` as they come, where the turn goes through `Player.Rotate` and its multiplier). The two switches that turn an axis of the mouse about apply to the look as to the turn. As bound in the install, across is positive to the right and up and down is positive downward.

**The key coming up** gives the binding's own command, `ResetLookDirection` (the binding is one of those whose way of being pressed cannot be chosen: `Release`).

## The head's turn (`Player.Look`)

Called on every frame the keys reach the player (not while a screen has them), with what the two axes gave, nothing included. Two figures are kept, across and down, in degrees.

1. **The limits** (the settings object): across from -40 to 40, down from -50 (up) to 20 (down); with the sights up (a player's, not an AI's) the limits across are times 0.7. While the sprint is on and the camera points between 50 and 90 degrees below level, the limit down is 0.
2. `across = clamp(across - travel across)`, `down = clamp(down + travel down)`: so across is negative to the right.
3. **What is shown of the turn down** is worked out here, before the way back: `down * (1 - (across / the limit across)^2)` while down is positive, and `down` itself while it is not. Looking down is cut back as the head turns aside, to nothing at the limit; looking up is not.
4. If the key came up since the last frame: **the mouse no longer has the head**, the way back is under way, and this frame's travel counts as none.
5. **The mouse has the head from the first frame whose travel is not nothing on both axes at once**, and keeps it until the key comes up. (So a turn made along one axis only never takes it.)
6. **While the mouse does not have the head, both figures go back**: each to `lerp(figure, 0, 15 * dt)` while it is more than 0.01, and to 0 below that. Thirty degrees are back in half a second. This runs against a turn along one axis only, too: a degree a frame across at 120 frames a second holds the head at 7 degrees.
7. The head counts as looking while both figures are other than 0 and it is not on its way back; the way back ends on the frame after both are 0.
8. The head's rotation is `(what is shown of down, across, 0)`.

The frame's length is the engine's.

## What takes it

- **The camera** (`ProceduralWeaponAnimation.LerpCamera`): the head's rotation is added to the camera's rotation spring's figure before the two are made a rotation (`weapon-handling.md`, "The camera"), so the camera turns on top of the bob, the lean and the poses. **While the head is turned at all the camera does not follow the weapon's climb** (`AddHandRecoilRotateToCamera` returns at once); its own kick is still added. The weapon, the arms and the view's own turn are untouched: the weapon stays where the body points.
- **The field of view with the sights up.** The sights going up or down change the field of view only while the mouse does not have the head (`OnAimOrPoseChanged`). And in `Player.Look`, with the sights up, the mouse taking the head puts the field of view to the hip's, and its letting go puts the aimed one back (each over the usual second). The flag that notes this is only brought up to date while the sights are up.
- **The neck and the head** (`PlayerBones.RotateHead`, from `Player.VisualPass`, for the character's own eyes and for anyone else's): after the animator has posed them, the neck's rotation in its parent is multiplied by `Euler(turn * NeckChain)` and the head's by `Euler(turn * HeadChain)`, where the turn is `(across, 0, -(what is shown of down))` and both shares are 0.5 on the player prefab. Only while the turn is a tenth of a degree or more. The function's first argument, which would blend in a set turn of the neck toward the sights, is always 0 here.
- Nothing else reads it but the game's dialogue with a trader, which holds the character still while the head is turned.

## Built

- `trp_sim::head_look::HeadLook` (`Player.Look` with its state), kept by `Movement` and run each frame the keys reach the character (`Input::head_look`, `head_reset`, `camera_pitch`); the limits from the settings object (`MovementSettings::head_look`).
- `Hands::set_head_rotation`: the camera's rotation takes it, the climb is not followed meanwhile, and the aimed field of view gives way.
- `Body::set_head_turn`, `drawn_bones`: the neck and the head among the bones a host asks for are turned by their shares (`trp_data::player::head_chain` reads them).
- `bindings::Preset::mouse`: the mouse's travel given to the turn or to the look, by the rule above.
- In the app: the mouse goes through that (`keys::hear`), `Nidnod` is among the bindings the app acts on (lit on the options tab, on the guide as "Look around with the head"), and the head's bones turn in the drawn body, its shadow in first person and the figure from outside. For a capture: `--tap <frame>:mmb:<frames>` holds the key and `--mouse <frame>:<frames>:<across>:<down>` moves the mouse so many counts a frame.

## Compared with what

**With the game: nothing.** No recording has the free look (recorder 0.7.0, built the same day and not yet run, writes the head's rotation and the engine's two mouse axes on every frame). All of the above is the code as read. Tried against the install (`crates/trp-sim/tests/free_look.rs`, and the unit tests of `head_look.rs` and `bindings.rs`):

- the install's limits are the code's own: -40 to 40, -50 to 20, 0.7;
- thirty degrees across and ten up: the head's rotation is exactly that, held for a second with the mouse still, and the neck's and the head's bones are each turned 15.81 degrees (half of the turn's 31.62); with the key up it is back to nothing in 0.48 s;
- the camera looks 30.00 degrees to the right for a head 30 to the right, and 10 down for 10 down;
- the field of view: 50 from the hip, 35 aimed, 50 aimed with the mouse on the head, 35 again once it lets go;
- the mouse: the first frame of travel with the key down goes to neither, the next to the look, a frame with no travel along an axis lets that axis go, and the key coming up gives the travel back to the turn.

The regression over the sixteen kept recordings is unchanged by it (`after44`).

**Seen** in captures (`local\shots\fl_*_s43.png`): in first person the camera turned to the right and up of a rifle that stays where the body points, and back with the key up; from outside the head turned toward the camera for a look to the left and away for one to the right.

**What a recording would settle** (`docs/manual.md`, item 12 of the "By hand" list; the trace has `mouse_axes`, `head_rot`, `mouse_look` and the view's own turn): that the first frame of each run of travel is lost, which is the one thing here a hand would feel and a reader of the code could have wrong; the rate; and the way back's half second.

## Not built, and open

- The dialogue's hold on the character.
- `PROVISIONAL`: the app hears the mouse once a drawn frame and hands the travel to the simulation's frames by their share of the time, as it does the turn. The game's manager and `Player.Look` both run once a game frame, so the lost first frame and the frame that needs travel on both axes are the app's drawn frames here, not the simulation's.
- An AI's use of the same function (a bot's head, turned by its own code with no way back) is not built.
