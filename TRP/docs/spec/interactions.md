# The hands on the world

How the character works a door, searches a container, opens a car's trunk and takes a thing up: what it can look at, which animation the left hand makes, whether the character is held for it, and what the thing itself does and when. Read from the game's code on 2026-10-12 (`EFT.AnimatedInteractionsSubsystem`, `InteractionContextHelper`, `Player`, `MovementState`, `MovementContext`, `DoorInteractState`, `PickUpState`, `ApproachState`, `WorldInteractiveObject`, `Door`, `LootableContainer`, `Trunk`, `SimpleCharacterController`) and from the install (the body's animator, the settings object), and **set beside twenty-three recordings made by hand the same day**: nine in the morning (`local\traces\byhand1`), which corrected the first reading in two places (a door pushed and a container searched take no movement state; a searched container is on screen 0.55 s after the key), and fourteen in the afternoon with recorder 0.6.1 (`local\traces\byhand2`: four doors, six containers of four kinds, a car's trunk, three pick-ups), which corrected it in three more (a door swings for 2.5 s, not 1.67; a container answers 0.55 s after the key, not 0.5; a container whose screen shuts is always told to shut).

## Whose animation it is

**The body's.** The weapon's own animator has a layer for the left hand (`LActions`, with states for hinges, keys, taking loot, a compass), and the firearm's controller still has the calls that would drive it; but for a firearm the two that matter (`SetInteract`, `SetPickup`) are empty, and nothing calls the third (`SetLooting`). The weapon stays in its resting state throughout (recorded: `IDLE` on every frame of every gesture).

What plays is a state of the body's animator, on a layer of its own: `InteractionsLayer`, the twenty-first of its twenty-two, additive, at full weight. It rests in `Idle`, and from there goes into one state for each gesture when two parameters say so: the flag `IsInteraction` and the number `InteractionIndex`. Each such state plays one clip and goes back to `Idle` by a way of its own.

**How the first-person hand follows it.** The clips are the third-person ones (`tp_...`), and each carries the curves the body's animator already hands an arm over by (`hands-on-weapon.md`): `First_Person_Curve_Weight`, `Hand_Left`, `Elbow_Left`, `Shoulder_Left`, from 0 up to 1 and back to 0 over the clip. While they are up the left arm is the body's, hand, elbow and shoulder, and the right hand keeps the weapon. Nothing else is needed: the arms' solver here already goes by those four.

## The gestures

| Number | State | Clip, length | Leaves for `Idle` at | Blend |
|---|---|---|---|---|
| 8 | `8 Take` | `tp_item_take`, 1.40 s | 0.9 | 0.2 s |
| 9 | `9 Drop` | `tp_item_throw`, 1.57 s | 0.64 (0.55 into the inventory's pose) | 0.25 s |
| 10 | `10 Backpack Drop` | `tp_backpack` | 0.9 (0.65 into the inventory's pose) | 0.1 of it |
| 16 to 21 | the doors' six: push and pull, hinge left and right, forward and backward | `tp_hinge_...`, 1.73 s | 0.9 | 0.1 of it |
| 22, 35 | `Door Key Open`, `Door Card Open` | `tp_key_use`, `tp_card_use` | 0.75 | 0.1 of it |
| 23 to 28 | the containers' six: open up, close down, open pull, close push, open default, close default | the doors' clips over again (open default is `tp_hinge_left_close_forward`) | 0.75 | 0.25 s |
| 29, 30 | `Container Hinge Up Open`, `Close` | `tp_cover_up_open`, `tp_cover_up_close` | 0.856 | 0.25 s |

Besides these the layer has the player's hand signs (1 to 7, 36 to 40), what is done to headgear (11 to 15) and the bipod's (31 to 34); none is built. The ways in are blended over a tenth of the state, a fifth of a second for `8 Take`, a quarter for the two drops.

**The game's own numbers are not the animator's.** A thing names its gestures by the game's `EInteraction` (the doors' six are 23 to 28, the containers' 30 to 37, `TakeItem` 8, `DropBackpack` 22, `DoorCardOpen` 38), and the model turns them into the layer's by a table of its own. For the doors' and the containers' that comes to seven less; recorded for nine pairs (the recorder writes the thing's number as `thing_gesture` and the animator's as `p_InteractionIndex`).

**A thing names its own gesture.** A door gives one of its six by where the character stands to it and which way it swings ("A door's gesture", below). A container or a trunk gives the two its prefab names (`_openInteraction`, `_closeInteraction`), or the default two if it names none, and they need not be containers' gestures. Recorded:

| Thing (the user's name) | Kind | Opened with | Shut with | What moves |
|---|---|---|---|---|
| technical supply crate | container | 29 `Container Hinge Up Open` | 30 | a lid, 0 to 4.78 degrees |
| wooden crate on the floor (twice) | container | 29 | 30 | a lid, 0 to -30 degrees |
| dead Scav | container | 21 `Door Pull Backward` | 28 `Container Close Default` | nothing |
| drawer | container | 25 `Container Open Pull` | 20 `Door Push Forward` | the drawer slides (its angles 0 and 0) |
| car's trunk | trunk | 27 `Container Open Default` | 28 | the lid, 0 to 60 degrees |
| the morning's two containers | container | 19 (a door's), 29 | 30 | |

**`9 Drop` is never asked for.** The game's list has `DropItem` and `ThrowInventory`, and nothing in its code starts either: a thing thrown out of the inventory makes no gesture. Only a backpack taken off does (`DropBackpack`, asked for by the hands' own operation for it).

## One at a time (`AnimatedInteractionsModel`)

A small model stands between what asks for a gesture and the animator. One gesture plays; the rest wait in a queue. Each gesture comes with an instruction:

| Gesture | Instruction |
|---|---|
| a container's | what is waiting is thrown out; the layer is put back to rest outright (`Idle` asked for at its start) before it plays |
| a door's | what is waiting is thrown out; nothing further is taken until the queue has run empty |
| taking a thing | the same as a door's, and it counts as an operation |
| a backpack dropped | as a container's, and it counts as an operation |

An operation is not thrown out of the queue and is not cut short by a forced stop. A gesture is over when its state has been left (the state carries a script that says so); the flag goes down in that frame and the next one waiting plays. A forced stop empties the queue of all but operations and puts the layer to rest unless an operation is playing. **The inventory opening is a forced stop** (`Player.SetInventoryOpened`).

**What starts one** (`MovementContext.SetInteractInHands`). It is refused in a sprint, with a mounted or a stationary weapon, and while the animator of the thing in hand is anywhere but in its resting state (a backpack's dropping excepted); so a gesture never begins in a reload or a shot.

## What is looked at (`Player.InteractionRaycast`)

Once a frame a line is cast from 0.15 m behind the point the player looks from (`BEHIND_CAST`) along the view, and the first thing it meets is what is looked at: a thing to take up, or a door, container or trunk. The install's reaches, counted from the eye: **1.3 m** for anything (`LOOT_RAYCAST_DISTANCE`), and **1 m** for a door (`DOOR_RAYCAST_DISTANCE`; the class's default is 0.75). A line that meets something else first finds nothing.

**Nothing is looked at at all** unless two things allow it:

- **The movement state.** Each state of the body's animator that names a movement state carries a flag for it on its script (`CanInteract`, read with the turning limits). In the install it is up in standing (and its turns on the spot), walking, lying still (`Prone_Idle_Aim`) and crawling, and blind fire; and down in a sprint and its stop, a jump and its landing, a sidestep, going down and getting up, a reload lying down and a turn lying down, every vault and climb, and the states of the hands on the world themselves (`probe bodystates - Player_anim_controller` prints `interacts`).
- **The thing in hand.** A rifle allows it only while its logic idles or fires (`FirearmController.CanInteract`: `Idling` or a fire operation): not in a draw, a reload, a look at it or a put-away. Empty hands always allow it. (Grenades, knives and usable items go by their animator resting; medicine and quick-use items never allow it; none of these is built.)

What is then offered for the thing is a menu ("The menu", below). **For anything being worked, nothing.** A door's own action also asks the movement context (`MovementContext.CanInteract`): refused while the hands are in an interaction of their own, or lying down where there is no room to sit up.

## The menu (`InteractionContextHelper.GetAvailableActions`, `AvailableInteractionState`, `ActionPanel`)

Read from the game's code on 2026-10-12. What the game offers for the thing looked at is a list of lines, each with a name and whether it can be chosen, and one of them chosen:

| The thing | Its lines, in order |
|---|---|
| a door, shut | `OpenDoor`; `Breach`; `Bang & clear`; `Flash & clear`; `Move in` |
| a door, locked | `Unlock`, if the character carries its key; `Breach`; `Bang & clear`; `Flash & clear`; `Move in` |
| a door, open | `CloseDoor` |
| a container | shut: `Search`; open: `CloseDoor` |
| a trunk | shut: `OpenDoor`; open: `CloseDoor` |

- **The last three of a door's are written into the code as never to be chosen.** They are shown all the same.
- **`Breach` can be chosen only where the door lets a kick be made** (`Door.IsBreachAngle`, and the door's own `CanInteractWithBreach`), and not lying down, crawling or getting up. The angle: the way from the character to the door's aiming point, flat on the ground, must not lie along the shut leaf by more than the settings' `DOOR_BREACH_THRESHOLD` (0.8 in the install, as the dot product of the two directions: so within some 53 degrees of square on). The menu is made afresh whenever the character crosses that limit (`GamePlayerOwner.DoorBreachMenuHandler`).
- **A door's key** is an item: a lock names one by its item id (`KeyId`; a key's own is its template's id), and the line is there if the character carries it anywhere.
- **The line chosen** is the first that can be, whenever the menu is made (`InitSelected`). **The wheel moves the choice** over the lines that can be chosen, round and round: a notch down to the next line down the list, a notch up to the next line up (`ActionPanel.TranslateCommand`: the game's `ScrollPrevious` and `ScrollNext`, which are the wheel down and up). **While a menu is up the wheel does not set the walking speed**: the panel takes both commands and passes neither on, even for a menu of one line.
- **The interact key does the chosen line**, on the frame it goes down; not while the inventory screen is up. With nothing that can be chosen (a locked door with no key at a bad angle) the key does nothing.
- **Drawn** (`InteractionButton`): every line; the chosen one on a grey ground (0.514) with black words, the others on black with grey words, and the words of one that cannot be chosen dark grey (0.25). The words are the game's texts for the names (`OPEN`, `CLOSE`, `BREACH`, `UNLOCK`, `SEARCH` and the three as they stand, in the install's English).

Opening, shutting and searching go the ordinary way (next section). `Breach` and `Unlock` go the game's other way, through the approach ("A kick", "Unlocking").

## Working a door, a container or a trunk

The game has two ways to a thing, and the ordinary one holds the character to nothing.

**Opened, shut, searched** (`Player.ExecuteInteraction`, which every plain "open", "close" and "search" of the menu calls; `MovementState.ExecuteDoorInteraction`), in this order:

1. A character lying down is no longer lying, and its stance goes to its lowest (recorded: from lying still the key puts the body into getting up, 0.834 s, and it stands at stance 0).
2. The gesture is worked out from the thing as it stands and where the player stands (`GetInteractionParameters`).
3. The thing is held for the work (`LockForInteraction`): it notes what it was and is now "being worked", which offers nothing.
4. The thing is told what is asked of it (`Interact`); what it does and when is its own ("A thing's own part").
5. The gesture is asked of the left hand, unless the thing says it is worked without one (`interactWithoutAnimation`). A gesture that is refused (a sprint, a weapon that is busy) changes nothing else: the thing is worked all the same.
6. **No movement state is taken.** The character walks, turns, crouches and leans as it likes while the hand is out. No approach, no crouch to a low thing, no lean let go.

**A search** adds to that (`InteractionContextHelper`, the container's "Search"): how far the character stands from the container is noted, and a timer of the player's is started for the settings' `DelayToOpenContainer`, **0.55 s in the install** (the class's default is 0.5). The timer is over on the first frame at least that long after the key's frame, and runs after that frame's update of the player, so it goes by where the character is then. The screen is shown if the character is not in a jump and is no more than a quarter of a metre further from the container than it was at the key; otherwise the container is told to shut again. The screen opening stops the gesture where it is (the forced stop above), so of a search's gesture only the first 0.55 s is ever seen. **When the screen shuts the container is always told to shut**, with no gesture (`TryInteractionCallback`, the screen's closing action; recorded four times, and the fifth recording ended first). It stays open through its wait and its swing and is shut 2.22 s after the screen.

## A kick (`MovementContext.StartDoorBreak`, `ApproachState`, `BreachDoorState`, `Door`)

Read from the game's code on 2026-10-12 (evening), after two recordings made by hand that afternoon had shown it was not the unlocking's path; built from that reading and then set beside the same two recordings.

**As it begins** (the menu's `Breach`):

1. **Where it is made from** (`Door.GetBreakInPosition`, `GetBreakInParameters`): the place 1.3 m square off the leaf, half a metre along it from the hinge, on the character's side of the door, at the height of its feet; and what it is aimed at, the point of the leaf half a metre along it.
2. **Whether the door puts the character there first** is the door's own (`Snap`, a set of the door's states, asked of the state it is in at the key). Recorded: a shut door did, a locked one did not.
3. **Whether it will open is decided now, and is no roll** (`Door.BreachSuccessRoll`): the door has to be one that can be breached (`CanBeBreached`, `Operatable`), and the way from that place to the door's aiming point has to run with the sum of the leaf's two lines, shut and open: **a kick opens a door only from the side it swings away from.** The body's animator is told (`KickSucceed`), which picks its clip.
4. The body's animator is told of the kick (`Kick`), and that the character already stands where it is to if the door puts it nowhere (`Approached`). The door is held for it from now ("being worked").

**The approach** (`Base Layer.Interactions.Approach`, the game's movement state 17). Standing and the walk each have a way into it on `Kick`, blended over a quarter of a second, and it gives way to the kick only when that blend is over and the character counts as there: **so it lasts a quarter of a second and a frame** (recorded 0.261 s, twice), unless the walk to the place takes longer. In it the view does not turn (the state lets no turn through), the lean is let go, the prone key does nothing and the inventory cannot be opened.

- For a door that puts the character nowhere, nothing else happens in it.
- **For one that does**, the character is walked to the place and its view turned to the door's point, levelled, both by one eased share of the way (a curve flat at both ends), and the body turned to face the same. How long that takes is the longest of: the distance at 2.5 m/s (less overweight, down to 0.01), the turn of the view at 90 degrees a second, and its levelling at the same rate; and no more than a second and a half. The frame the state is entered on has run none of it.

**The kick** (`Stand_Kick`, or `Stand_Kick_Fail` for one that will not open; 1.333 s each; the game's movement state 9, `BreachDoor`). It gives way to standing 0.9 of the way through, so the state lasts **1.2 s**. In it the view turns a tenth as fast and no more than a degree a frame, the hands take no input and the sights cannot be up (the settings' two lists of states have it), movement keys, jump and sprint do nothing.

- The body does not turn after the view. **From 0.15 of the way through it moves by the clip's own motion.** A kick that opens the door carries the character 0.65 m on through the doorway; one that fails takes it a quarter of a metre in and back out to a third of a metre behind where it stood.
- **0.45 of the way through, once, the kick lands** (`KickTime`): the door is thrown open (`Door.KickOpen`) or holds (`Door.FailBreach`), and the legs pay what a jump costs (`Physical.OnBreach`).
- As the state is left the animator's flags go down.

**The door.** Thrown open, it swings from its shut angle to its open one **in half a second, with no wait**, along a curve of its own (`KickCurve`: from nothing to all of the turn, at a slope of one at both ends), is open when that is over, and counts as broken. A kick that fails leaves it "being worked" for 0.3 s more and then puts it back as it was, locked if it was locked.

## Unlocking

(`Player.StartInteraction`: a door or a container unlocked with a key or a card in the hand; `MovementContext.StartInteraction`, `ApproachState`, `DoorInteractState`.) The menu's `Unlock` first counts a use of the key, which is thrown away when its uses are spent (`UnlockOperation`), and then holds the character:

1. **The approach.** The body's base layer goes through its approach state first, so the lean is let go and a thing more than 0.65 m under the ribcage is crouched to; a thing that puts the character at a place of its own (`Snap`) has it walked and turned there.
2. **The state for it** (the movement state numbered 16; the base layer plays its standing clips through it). It takes no walk and no jump; the view turns, and a stance or a lean asked for is taken. The inventory cannot be opened. It has two parts:
   - **The hand is laid.** The gesture begins. This part ends when the hands' animator says the left hand has arrived (`LeftHandProgress` over 0.99) or 1.6 s after the state began; a firearm's animator plays nothing for it, so by the code it is always the 1.6 s. As it ends the thing is told to do its part.
   - **The hand comes away.** Where the thing neither snaps nor has a grip, this part ends on the next frame. Otherwise it lasts until 3 s after the state began.
3. **The end.** The state is left; a container that is open is then shown, and the stance held before the approach is taken again when its screen shuts.

A door unlocked is shut, at once, when it is told (`Unlock`): it is then opened like any other.

No recording has a lock opened with a key: this path is as read. (The approach itself is on record in the two kicks.)

## A thing's own part (`WorldInteractiveObject`, `Door`, `LootableContainer`, `Trunk`)

Read on 2026-10-12 and **set beside every recorded swing frame by frame** (`cargo run -p trp-sim --bin byhand -- local\traces\byhand2`; `local/swing.sh`).

**States.** Shut, open, and being worked (also locked and breached, not built). Being worked lasts from the key until the swing is over; what the thing was is kept, and it goes back to that if the order it was given is refused while it is not swinging.

**A door's gesture** (`Door.CalculateInteractionIndex`). Two lines through the door's place: the line its leaf lies along now, and the line it will lie along when it has swung (its own forward axis, which turns with it, turned by its shut and open angles, or back by them if it is open). The character's place is dropped square onto the first line; whether the way from the character to that foot runs with the side the door will swing to says whether it swings **away** (pushed) or **towards** the character (pulled), and the upward part of the cross product of the first line with that way says on which hand the hinge is. By the game's numbers:

| The door is | It will swing | Hinge to one hand | Hinge to the other |
|---|---|---|---|
| shut | away | 27 `Door Push Forward` | 27 |
| shut | toward the character | 26 `Pull Hinge Right` | 24 `Pull Hinge Left` |
| open | away | 25 `Push Hinge Right` | 23 `Push Hinge Left` |
| open | toward the character | 28 `Door Pull Backward` | 28 |

(The animator's are seven less.) Being worked, the two lines are nothing and the number comes out 28. A locked door asks for `Door Key Open`. Recorded: the rule gives the game's own number on all 349 frames the four doors were looked at while shut or open, with the door's forward axis its own y (the only one of the four possible that does), and it was 28 on every frame of their swings.

**How long it waits.** A door waits **half a second of its own**, in its routine (`Door.SmoothDoorOpenCoroutine`: a fixed 0.5 s, `DelayToOpenContainer` not used). A container and a trunk wait the settings' **`DelayToOpenContainer`, 0.55 s** in the install (`LootableContainer.Interact`, `Trunk.Interact`, a timer). Either is over on the first frame at least that long after the frame it was told on. A door is given its new state at the key (and so refuses or takes the order then); a container or a trunk when its wait is over.

**How it swings.** One curve for every thing, the settings' `DoorCurve`:

| Time on the curve (s) | 0 | 0.568 | 1.718 | 2.5 |
|---|---|---|---|---|
| Share of the turn | 0 | 0.546 | 0.976 | 1 |

(Slopes at the four keys: 1.11, 0.61, 0.10, 0.01.) The turn is the shortest way from the angle it stands at to its open or shut angle (`OpenAngle`, `CloseAngle`; recorded: doors 0 and 110). **A door runs the curve at its own pace**: its routine calls the base one without the speed the base asks for, **so 2.5 s**. **A container and a trunk run it half as fast again (1.5), so 1.67 s.** On the swing's first frame the thing stands where the curve begins, and its time is moved on by that frame's length times the pace; each frame after, it stands where the curve has it at the time so far, and the time is moved on again; the first frame that finds the time at or past 2.5 puts it at its new angle and in its new state. A drawer is moved the same way between its shut and open places (out by the curve opening, back by it shutting). A door or a container with a handle plays the handle first and waits for it.

So, with the key on a frame of its own: **a door stands at its new angle and state 3.0 s after the key** (recorded 3.005 and 3.009), **a container or a trunk 2.22 s after** (recorded 2.220 to 2.225, seven times), and a container told to shut by its screen 2.22 s after the screen shuts (recorded 2.220 to 2.225, four times). The hand's gesture (1.56 to 1.94 s) is over before any of them stands still.

**An order it refuses** (`CanStartInteraction`): to the state it is in; to the state its last order was for (whatever its progress); and, for a door or a trunk, to the state its last swing ended in until a swing's length has passed since. Recorded twice: the drawer and the dead Scav were shut by hand (F) while their screens' shutting was still under way; the key made the gesture and held the thing as being worked, the order was refused, and the thing was shut on the frame the screen's order had it.

**The door and the character** (`Door.CollisionColliders`, `LocationScene.DoorsCollisionColliders`, `SimpleCharacterController`). A door's colliders on the player's collision layers are solid to the character as any wall is, and turn with the leaf. Every door's are handed to the player's controller as colliders "with no speed limit": while the capsule overlaps one, the controller does not cut the move to the state's hard cap on speed (`IsOverlappedWithNoSpeedLimitCollider`), so a leaf that swings into the character pushes it out of its way at whatever pace that takes, by the ordinary way out (movement spec, "The character's own collision"). The settings' `DisableDoorColliderOnInteraction` (on in the install) is read by nothing.

**Recorded** (2026-10-12, `local\traces\byhand3`, "Door swinging onto player": the character stands still against a shut door that opens towards it, a step from the hinge, and works it). The door's own part is the model's on every frame (its angle to 0.000 degrees, its state on all 381). **The character is pushed, standing:** it begins to move 0.58 s after the key, as the leaf does, and is moved **1.06 m in 1.11 s, at up to 1.83 m/s**, until the leaf has swung some 104 of its 124 degrees; its movement state is the standing one throughout, its hands' animator at rest, and nothing is asked of the player. That is the push with no cap on its speed, as read. **Not compared with the app's own push:** the path depends on the leaf's collider (where it lies off the hinge, how thick it is), which the recorder does not write; a flat leaf through the hinge's axis does not fit the path (the character's axis would have to be 0.64 m off it, and is not kept at one distance: 6 cm rms).

Not read: what `Snap` does at a door, a handle's own animation.

## Taking a thing up

Only out of the standing state (`IdlePlayerState`): the same call in any other movement state, a walk among them, does nothing. It puts the pick-up state over the standing one (`PickUpState`):

- the lean is let go, and the gesture `TakeItem` asked for;
- with the view more than 60 degrees down, the character goes down to a third of its stance;
- **the view does not turn, the stance and the lean take nothing, and no walk begins** (the animator is put back into its standing state whenever it would leave it);
- **0.6 s in, the thing is taken** (the inventory's own move is made then), and that ends the state: the stance it began with is taken again. Left to itself the state would end after 5 s.

The gesture is 1.4 s long, so the hand is still out when the thing has gone and the character can move again. Recorded at three heights: from the floor with the view 85 degrees down (the stance at 0.33 for the while), at waist height 46 degrees down (the stance kept at 1), and higher up crouched, the view 18 degrees up (the stance kept at 0); the state lasted 0.608 to 0.611 s and the flag 1.462 to 1.467 s each time.

## A backpack dropped (`Player.DropBackpack`, the hands' `DropBackpackOperation`, `DropBackpackEventConsumer`; read and built 2026-10-12, night)

The backpack has a key of its own (`DropBackpack`; in the install `Z` twice within the double click's time), and its slot is one the game animates: a pack leaving it for the ground goes by a gesture, by the key and from the inventory screen alike.

- **The key** does nothing with no pack on the back. Otherwise the pack is thrown (`TryThrowItem`) if it may leave its slot, the character is not sprinting and not in a kick, no gesture of the hands is playing (`AnimatedInteractions.CanInteract`), and what is in hand is not at a gesture of its own left hand. (Also, each for something not built here: a mounted weapon is first let go, and a compass in the hand is put away and nothing else happens.)
- **The throw is the hands' to carry out**, as an operation of whatever is in hand. **Only the resting operation takes it**; in a shot, a reload, a look at the weapon, a draw or a put-away the game refuses the throw and the pack stays on. A weapon's operation for it (`DropBackpackOperation`) takes the trigger off the animator, tells it the inventory is shut, puts the sights down, and asks the body for the gesture; through it the trigger, the reload, the looks and a put-away are not taken at all (a put-away asked for meanwhile is lost).
- **The gesture** is state 10 of the interactions' layer (`10 Backpack Drop`, the clip `tp_backpack`, 1.333 s): entered from rest over a quarter of a second, or from the open inventory's pose 0.15 of the way in; left for rest 0.9 of the way through, or for the inventory's pose 0.65 of the way through with the screen open. It is asked for whatever the weapon's animator is doing (the one gesture that is), and refused in a sprint. It is an operation's gesture: nothing throws it out of the queue or cuts it short.
- **The pack leaves the shoulders at the state's own mark** (`OnBackpackDrop`, 0.25 of the way through: 0.333 s in). The hands' operation ends with it (the animator is told how the inventory stands, the weapon idles) and the throw is carried out: the pack is out of the inventory and in the world. An operation whose gesture is cut off before the mark is run on to its end (`FastForward`).

**Built:** `Movement::can_drop_backpack`, `drop_backpack`, `Acted::BackpackDropped` (the body's animator says when the state passes its mark, `Body::take_backpack_dropped`); `Firearm::drop_backpack`, `backpack_dropped`; `EmptyHands::idling`. In the app the key, and the inventory screen's key for putting a thing down when the thing is the pack on the back, begin it (`inventory_view::toggle`), and at the mark the pack lies on the ground a step ahead, as anything put down does (`ESTIMATE`: the game throws it). **Not built:** a pack dragged off its slot to anywhere else, which the game animates the same way; the pack's own model leaving the back.

**Compared with the game: nothing**; no recording has a pack dropped. Tried against the install: the mark comes 0.358 s after the gesture is asked for (the blend in and the frame included) and the gesture is over at 1.35 s; in a capture of the app the gesture begins three drawn frames after the key's second press comes up and plays 1.2 s.

## The body's pose for an open inventory

On the same layer: `Idle` goes to `Inventory` (`tp_inventory`, looping) while the body's `Inventory` flag is up and `InventoryOperation` is down, blended over 0.3 s; `Inventory` gives way to `Inventory End` when the flag goes, and stands back for `Idle` while `InventoryOperation` is up. (A second way out of `Idle`, into `Inventory Start`, has the same conditions and comes later in the controller's list, so that state is never entered.)

The flags are the player's (`WaitWeaponInventory`; `inventory.md`, "The inventory open"): `Inventory` goes up once the hands' animator is in `OPEN INVENTORY` or `IN INVENTORY`, and from then on `InventoryOperation` is up for as long as the screen is open and that animator is doing something else. Empty hands have neither state, so their body never takes the pose; and a reload begun before the screen opened keeps both flags down until it has ended in the hold.

`tp_inventory` carries the same four curves at 1, so **with the inventory open the left arm in first person is the body's**, not the weapon's: the weapon's own states for it take the hand off the weapon, and the body's pose says where it goes.

## Built

- `trp-sim/src/interactions.rs`: the gestures by the game's names, the animator's numbers and the game's own (`Interaction::index`, `game_number`, `of_game_number`), and the model in front of the animator (`AnimatedInteractions`).
- `trp-sim/src/things.rs` (2026-10-12): a door, a container, a trunk (`Thing`): its states, what the game offers for it (`action`), the gesture it asks (`gesture`, a door's by `door_gesture`), being worked (`work`) or told with no hand (`tell`), and its wait and swing on the install's curve frame by frame (`step`, `ThingSettings`: `DoorCurve`, `DelayToOpenContainer`), with the game's refusals. **Since the evening of 2026-10-12:** the menu and its wheel (`Menu`, `Thing::menu`); a lock and its key (`lock`, `unlock`, `ThingState::Locked`); a kick (`start_breach` with `break_in`, `breach_angle` and `breach_succeeds`; `kick_open`; `fail_breach`).
- `trp-sim/src/movement.rs`, `body.rs`: `State::Approach` and `State::Breach`, entered by the body's animator, whose three states for them are now read; `Movement::kick` (with the place a door puts the character at, if it does), `Acted::Kicked`, `kicking`, `locks_inventory`.
- The app: the menu drawn for whatever is looked at, the wheel choosing and `F` doing the chosen line; the range's right-hand door locked, its key in the crate.
- `trp-sim/src/body.rs`: the layer's states are read with the rest; the two parameters and the inventory's two are set each frame; a gesture's end is taken from its state being left; `Body::reset_interactions`, `interaction_state`; each base state's `CanInteract`.
- `trp-sim/src/movement.rs`: `Movement::set_interaction` (with the refusals above); `work` (the gesture, nothing held, a lying character got up); `search` (the same, and `Acted::Searched` or `Acted::LeftUnsearched` when the install's wait is over, the key's frame not counted, the distance taken after the frame's move); `unlock`; `pick_up`; `can_interact` (the movement state's flag); the inventory opening stops the gestures; the body's inventory flags. `World::lifts_cap`: a host says the capsule is in a door's leaf, and the hard cap on speed is not applied to that move. `Firearm::can_interact`.
- `trp-sim/src/collision.rs`: a block may be turned about the upright (`Block::turned`), for a door's leaf.
- `trp-data`: the settings object read on to `AnimatorCullDistance` (195 fields), so `DelayToOpenContainer` and the reaches come from the install; each body state's `CanInteract`.
- **The app's range** (`crates/trp-app/src/range.rs`): two doors in the wall behind the start, hinged at the two ends of a doorway and turning 110 degrees towards the start, solid to the character and to the weapon's line, worked from either side; beyond them a wooden crate on the floor and a technical supply crate at waist height (gestures 29 and 30, their lids turning as recorded), a dead Scav (21 and 28), a drawer that slides out (25 and 20), a table and a shelf with things lying on them and a stack of rounds on the floor, and a car whose trunk opens and shuts (27 and 28, 60 degrees), in a row behind the door wall as the user drew it that evening; and the airdrop crate with the rifle's kit. Each container is the install's item of that sort (empty), shown in the screen when searched. `F` does what the game offers for what is looked at, and nothing is looked at when the movement state or the rifle does not allow it. The things move on the simulation's frames, after the character. `--start` puts the character before each of them, turned to it.
- `trp-sim/src/bin/byhand.rs`: sets a recording made by hand beside the logic of what the hands hold, lists what the body did, and **sets the door, container or trunk worked beside the model of its kind on every frame** (recorder 0.6.1 on), with the door's gesture by side tried on every frame it was looked at.

**PROVISIONAL or not built:**

- The unlocking state and the pick-up's are not movement states of their own here. While either is in hand the keys are noted and not heard, as under a screen: a walk under way runs out as with the keys let go, where the game's body stops for the approach; and a stance or a lean asked for while a thing is unlocked, which the game's state takes, is dropped. The whole unlocking path is compared with nothing.
- The range's things are the app's shapes and places, modelled on the recorded ones for what they ask of the hand and how they turn. The airdrop crate names no gesture of its own here (so the default, 27 and 28; its prefab is not read) and nothing of it moves. How far the drawer comes out is an ESTIMATE (0.35 m; its prefab's open place is not read). The dead Scav is 0.4 m high so that it is in reach standing, as the recorded one was.
- A door's leaf is not among what the camera's box is kept out of (`World::push_out`); it is among what the character and the weapon's line meet. What the line of sight meets of a container is its whole body; of the trunk, the car's back metre.
- **Of the kick:** where a door's own aiming point is (`viewTarget1`: taken as half a metre along the leaf, a metre up) and which of its states a door puts the character at its place in (taken as shut and not locked, as the two recorded doors were) are each door's own and not read; `PROVISIONAL`. The broken door's pieces, the kick's sound and dust, and letting the kicking character through the leaf (`IgnoreInteractionCollision`) are not built. A stance changed or the prone key pressed in the kick is taken as the game's base state takes it, untried.
- **Of the lock:** the key is not put in the hand (the unlocking state is not a movement state of its own here: above), and its uses are not counted. A lock on a container, a card.
- A handle, the hand put on a thing's grip (`SetIKInteraction`), the approach's walk for an unlocking. Lying down, the door's own check for room to sit up.
- Of the menu: the lines for a thing on the ground other than taking it; the game's art for it.
- A backpack dropped; hand signs; headgear; the bipod.

## Compared with the running game

**Nine recordings made by hand, the morning of 2026-10-12** (recorder 0.6.0, `local\traces\byhand1`; `local/gesture_layer.sh <trace>` follows the layer through one). The character stood at 120 frames a second. The simulation's figures are `the_hands_work_a_container_and_take_a_thing`, at the same 120:

| What was done | Recorded | The simulation |
|---|---|---|
| a door's gesture (17, `Door Pull Hinge Left`) | the movement state stays the standing one, and the walk when the character walks off 0.64 s in; `Interact` and `Approached` never rise; the flag up 1.737 s | no state; 1.733 s |
| another door's (20, `Door Push Forward`) | the same; 1.737 s | 1.733 s |
| a container searched (19 once, 29 four times) | no state, stance unchanged at 1, the view free; **the screen open 0.550 to 0.553 s after the key**, and the gesture's flag down on that frame with the layer put back to `Idle` outright | `Searched` 0.550 s after the key's frame; the gesture stopped as the screen opens |
| that container's lid shut by hand (30) | the flag up 1.771 s | 1.775 s |
| a thing taken up, the view 74 degrees down | the pick-up state for 0.614 s; the view held; the stance at 0.33 for the while and back to 1; the flag up 1.466 s | 0.608 s; held; 0.33; 1.467 s |
| the body's pose for the screen | `Inventory` up as the weapon's animator enters `OPEN INVENTORY` (two frames after the screen opens), the layer into its `Inventory` state over 0.3 s with the whole left hand; with a reload under way, not until it has ended in the hold (4.8 s after the screen opened); `InventoryOperation` up 0.365 s after the weapon was taken off its slot (when `IDLE TO OUT` has taken over from the hold) and the layer back to `Idle`; never with empty hands | the same rule, read from the same routine; not set beside the recording frame for frame |

Seen in the layer itself (`gesture_layer.sh`): a door's and the pick-up's gesture begin to blend in on the frame of the key, a container's one frame later, the layer having been put to rest first, as the model's instructions say; the state is entered 0.100 s after (0.2 s for the pick-up's); it starts to give way at 0.9 of itself (0.86 for the lid's) and the flag falls on the frame `Idle` is the state again.

**Fourteen recordings made by hand, the afternoon of 2026-10-12** (recorder 0.6.1, `local\traces\byhand2`, the file names the user's; `cargo run -p trp-sim --bin byhand -- local\traces\byhand2`, kept as `local\byhand2.txt`). In every one the weapon's animator is the game's on every frame.

| What was done | Recorded | The model |
|---|---|---|
| a shut door pulled open, an open one pulled shut, a shut one pushed open, an open one pushed shut (two doors, one from each side) | gestures 17, 21, 20, 16; the door at its new state 3.005 and 3.009 s after the key | the same four gestures from the door's geometry and the player's place; the angle **the game's to 0.000 degrees on every frame** (1,748 frames), the state never different |
| the door's gesture by side, on every frame a door stood shut or open and was looked at | the game's number for where the player stood | the same on all 349 frames (the door's forward axis its own y) |
| a technical supply crate, a wooden crate twice (once lying down), a dead Scav, a drawer searched; the screen shut | the screen open 0.550 to 0.553 s after the key; the container open 2.220 to 2.224 s after the key; shut 2.220 to 2.225 s after the screen | the lid's angle the game's to 0.000 degrees on every frame (4,566 frames), the state never different |
| the dead Scav and the drawer shut by hand while closing | gestures 28 and 20; the thing being worked from the key and shut on the screen's schedule | the same |
| a car's trunk opened, then shut | gestures 27 and 28; no screen; the lid 0 to 60 degrees, at rest 2.223 and 2.225 s after the key | the angle the game's to 0.000 degrees on every frame (816), the state never different |
| the gestures' lengths | 16: 1.738 s; 20: 1.737 and 1.739; 21: 1.939; 27: 1.563; 28: 1.561 and 1.565 | 1.733, 1.733, 1.933, 1.558, 1.558 (`the_hands_work_a_container_and_take_a_thing`; the recorded frames run a little long) |
| a container searched lying down | the key puts the body into getting up (`Prone2Stand`) for 0.834 s; the gesture begins with the key; stance 0 after | 0.833 s; stance 0 (`a_search_lying_down_gets_the_character_up`) |
| things taken up from the floor, at waist height, higher up crouched | as under "Taking a thing up" | 0.608 s and the stance as recorded, at the morning's figures |

**Recorded on 2026-10-12, afternoon** (`local\traces\byhand3`): a door swung into the character ("The door and the character"), a kick at a locked door and at a shut one, and the inventory opened with its own key (open on the frame `Tab` goes down, shut on the frame it goes down again: `inventory.md`).

**The two kicks, set beside what was built from the code that evening** (`cargo run -p trp-sim --bin byhand -- local\traces\byhand3 Breach`; `cargo test -p trp-sim --test install a_door_is_kicked -- --nocapture`):

| | Recorded | The model |
|---|---|---|
| the shut door: thrown open | its angle and its state on 444 frames | **the angle the game's to 0.000 degrees and the state the game's on every frame**, the kick's curve and its half second with it; that it opens from where the player stood is the model's own answer (the leaf's axis is on record: 281 frames of its gesture by side) |
| where that door put the player | where the player stood as the kick's state began | **0.0 mm** from the place the model has the kick made from |
| the locked door: held | its state on 333 frames, locked again 0.904 s into the kick's state | the state the game's on every frame. Which side that door opens to is not on record (it was never looked at shut or open), so that the kick fails is taken from the recording, not decided by the model |
| the approach | 0.261 s, twice; with the shut door the view from 9.8 degrees down to level and 2.4 degrees round in 0.110 s, the player walked 0.209 m | 0.258 s at an even 120 frames a second; the same place to under a millimetre and the same view |
| the kick's state | 1.202 and 1.205 s; the kick lands 0.603 s in (the legs' stamina falls on that frame, by a jump's cost) | 1.200 s; 0.608 s |
| the door open after the kick lands | 0.506 s | 0.508 s |
| the body through a kick that opens | 0.5905 m on as the state is left, 0.6489 m at rest | 0.5907 and 0.6500 m |
| the body through a kick that fails | 0.2492 m in, 0.2305 m behind as the state is left, 0.3242 m behind at rest | 0.2568, 0.2264 and 0.3226 m |

The body's figures are the simulation's own run with the recorded doors' places and the recorded character's Strength, on even ground; the two differ by up to 8 mm in the failed kick and by 1 mm at rest. **Not compared frame by frame:** the body's place through the kick (no script replays a recording made by hand), and the menu, which no recording shows.

**Not recorded yet:** a lock opened with a key; a container walked away from as it is searched; the leaf's own collider, for the push.
