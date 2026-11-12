//! The range built: its hall, props and own boxes.

use super::*;

impl Range {
    /// The range: its own boxes, or a level file's in their place, and its
    /// props where the file stands them (`level`).
    pub fn of(level: Option<std::sync::Arc<crate::world::level::Level>>) -> Self {
        let mut solids = Vec::new();
        let block = |centre: Vec3, size: Vec3, color: Color| Solid { name: "block", centre, size, yaw: 0.0, color, surface: trp_sim::sound::steps::CONCRETE, overhead: false };
        match &level {
            // A level file's boxes, each as the file has it.
            Some(level) => {
                for found in &level.boxes {
                    let name: &'static str = Box::leak(found.name.clone().into_boxed_str());
                    let overhead = crate::world::level::is_roof(&found.name);
                    solids.push(Solid { name, centre: found.centre, size: found.size, yaw: found.yaw, color: found.color, surface: found.surface, overhead });
                }
            }
            None => Self::own_boxes(&mut solids),
        }
        let stand_of = |name: &str, anchor: Vec3| level.as_ref().and_then(|level| level.props.get(name)).copied().unwrap_or(crate::world::level::Stand::made(anchor));
        let mut props: Vec<Prop> = Vec::new();
        // Each prop: what it is made of is made, then put where it stands.
        let end = |name: &str, anchor: Vec3, at: (usize, usize), solids: &mut Vec<Solid>, things: &mut Vec<Placed>, props: &mut Vec<Prop>| {
            let stand = stand_of(name, anchor);
            if stand != crate::world::level::Stand::made(anchor) {
                for solid in &mut solids[at.0..] {
                    solid.centre = stand.point(anchor, solid.centre);
                    solid.yaw += stand.yaw;
                }
                for placed in &mut things[at.1..] {
                    placed.stand(anchor, stand);
                }
            }
            props.push(Prop { name: name.to_owned(), stand, solids: at.0..solids.len(), things: at.1..things.len() });
        };
        // The things for the hands. Each container's or trunk's body is a
        // solid; what moves is drawn apart (`Part`).
        let mut things = Vec::new();
        let body = |solids: &mut Vec<Solid>, at: [f32; 2], size: Vec3, color: Color| {
            solids.push(block(Vec3::new(at[0], size.y / 2.0, at[1]), size, color));
            *solids.last().unwrap()
        };
        let container = |name, thing, solid: Solid, part, holds| Placed {
            name,
            thing,
            part,
            hit: solid.block(),
            place: in_sim(solid.centre),
            holds: Some(holds),
            leaf: None,
        };
        // A lid on a body of `size` standing at `at`, hinged along the
        // body's far edge from the door wall, turning about `axis`.
        let lid = |at: [f32; 2], size: Vec3, axis: Vec3, color| Part::Moving {
            centre: Vec3::new(at[0], size.y + 0.02, at[1]),
            size: Vec3::new(size.x, 0.04, size.z),
            yaw: 0.0,
            pivot: Vec3::new(at[0], size.y + 0.02, at[1] + size.z / 2.0),
            axis,
            slide: Vec3::ZERO,
            color,
        };
        // The benches (`BenchKind`): each table's boxes, not drawn (its
        // model is, `workbench`).
        for kind in BenchKind::ALL.into_iter().filter(|kind| kind.on()) {
            let at = (solids.len(), things.len());
            let name = match kind {
                BenchKind::Gun => "gun bench",
                BenchKind::Armor => "armor bench",
            };
            for (low, high) in kind.solids() {
                let (centre, size) = kind.boxed(low, high);
                solids.push(Solid { name, centre, size, yaw: 0.0, color: UNSEEN, surface: METAL, overhead: false });
            }
            // The boxes of repair kits under the gun bench, opened as the
            // tables' magazine boxes (the gestures recorded for a lid hinged
            // up), their fronts to the lanes; the armour's on the right seen
            // from the lane, as under the workbench before.
            if kind == BenchKind::Gun {
                let table = kind.table();
                let box_olive = Color::srgb(0.22, 0.23, 0.18);
                let inside = BENCH_LEG + MAGAZINE_BOX.x / 2.0 + 0.01;
                for (index, label) in REPAIR_BOXES.into_iter().enumerate() {
                    let along = if index == 0 { table.length() - inside } else { inside };
                    let floor = kind.point(table.at(BENCH_LEG + MAGAZINE_BOX.z / 2.0 + 0.01, table.low[1], along));
                    let body = block(floor + Vec3::Y * MAGAZINE_BOX.y / 2.0, MAGAZINE_BOX, box_olive);
                    solids.push(Solid { surface: PLASTIC, ..body });
                    let mut opens = Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -30.0);
                    opens.open_angle = -MAGAZINE_BOX_OPENS;
                    let lid = lid_of(floor + Vec3::Y * MAGAZINE_BOX.y, MAGAZINE_BOX, Vec3::Z, box_olive.lighter(0.04));
                    let label: &'static str = Box::leak(format!("{} box", label.to_lowercase()).into_boxed_str());
                    things.push(container(label, opens, body, lid, REPAIR_BOX_HOLDS + index));
                }
            }
            end(kind.prop(), kind.anchor(), at, &mut solids, &mut things, &mut props);
        }
        // Behind the door wall, from left to right. A technical supply crate,
        // its lid at waist height (recorded: gestures 29 and 30, the lid from
        // 0 to 4.78 degrees).
        let wood = Color::srgb(0.47, 0.36, 0.22);
        let green = Color::srgb(0.28, 0.34, 0.26);
        let hinged_up = || Thing::container(Some(Interaction::ContainerHingeUpOpen), Some(Interaction::ContainerHingeUpClose), 0.0, -30.0);
        let at = (solids.len(), things.len());
        let supply_size = Vec3::new(0.55, 0.84, 0.9);
        let supply = body(&mut solids, SUPPLY_CRATE, supply_size, green);
        let mut tech = hinged_up();
        tech.open_angle = 4.78;
        things.push(container("technical supply crate", tech, supply, lid(SUPPLY_CRATE, supply_size, Vec3::X, green.darker(0.05)), 2));
        end("supply crate", Vec3::new(SUPPLY_CRATE[0], 0.0, SUPPLY_CRATE[1]), at, &mut solids, &mut things, &mut props);
        // A drawer in a cabinet, its front to the door wall (recorded:
        // pulled open with 25 and pushed shut with a door's 20; it slides).
        let at = (solids.len(), things.len());
        let cabinet = body(&mut solids, DRAWER, Vec3::new(0.8, 0.8, 0.5), Color::srgb(0.40, 0.38, 0.33));
        let front = Vec3::new(DRAWER[0], 0.64, DRAWER[1] - 0.03);
        let drawer_front = Part::Moving {
            centre: front,
            size: Vec3::new(0.7, 0.15, 0.5),
            yaw: 0.0,
            pivot: front,
            axis: Vec3::Y,
            // ESTIMATE: how far the drawer comes out (its prefab's open
            // place is not read).
            slide: Vec3::new(0.0, 0.0, -0.35),
            color: Color::srgb(0.46, 0.43, 0.37),
        };
        let drawer = Thing::container(Some(Interaction::ContainerOpenPull), Some(Interaction::DoorPushForward), 0.0, 0.0);
        things.push(container("drawer", drawer, cabinet, drawer_front, 4));
        end("drawer", Vec3::new(DRAWER[0], 0.0, DRAWER[1]), at, &mut solids, &mut things, &mut props);
        // The storage against the staircase's side, its fronts to the lane
        // (`CABINETS`): cabinets and a dresser with a drawer as the row's
        // (pulled open with 25, pushed shut with a door's 20; it slides), and
        // a box with a lid hinged along its back by the stairs (the gestures
        // recorded for a lid hinged up, 29 and 30), lifted towards them.
        for (index, name) in CABINETS.into_iter().enumerate() {
            let begun = (solids.len(), things.len());
            let (at, furniture) = (cabinet_at(index), Furniture::of(name));
            let size = furniture.size();
            let anchor = Vec3::new(at[0], 0.0, at[1]);
            let (body_color, front_color) = match furniture {
                Furniture::Box => (Color::srgb(0.27, 0.31, 0.21), Color::srgb(0.25, 0.29, 0.19)),
                _ => (Color::srgb(0.40, 0.38, 0.33), Color::srgb(0.46, 0.43, 0.37)),
            };
            let body = Solid { surface: METAL, ..block(Vec3::new(at[0], size.y / 2.0, at[1]), Vec3::new(size.z, size.y, size.x), body_color) };
            solids.push(body);
            let named: &'static str = Box::leak(format!("{} {}", name.to_lowercase(), furniture.word()).into_boxed_str());
            if furniture == Furniture::Box {
                let mut opens = hinged_up();
                opens.open_angle = -GUN_CASE_OPENS;
                let lid = lid_of(Vec3::new(at[0], size.y, at[1]), size, Vec3::X, front_color);
                things.push(container(named, opens, body, lid, CABINET_HOLDS + index));
                end(named, anchor, begun, &mut solids, &mut things, &mut props);
                continue;
            }
            let front = Vec3::new(at[0] + 0.03, furniture.drawer_height(), at[1]);
            let drawer_front = Part::Moving {
                centre: front,
                size: Vec3::new(size.z, 0.15, size.x - 0.1),
                yaw: 0.0,
                pivot: front,
                axis: Vec3::Y,
                // ESTIMATE: as the row's drawer's.
                slide: Vec3::new(0.35, 0.0, 0.0),
                color: front_color,
            };
            let drawer = Thing::container(Some(Interaction::ContainerOpenPull), Some(Interaction::DoorPushForward), 0.0, 0.0);
            things.push(container(named, drawer, body, drawer_front, CABINET_HOLDS + index));
            end(named, anchor, begun, &mut solids, &mut things, &mut props);
        }
        // A dead Scav lying along the row (recorded: opened with a door's
        // gesture, 21, and shut with the default, 28; nothing moves). It is
        // 40 cm high so that it is in reach standing, as the recorded one
        // was (the game's reach is 1.3 m from the eye).
        let at = (solids.len(), things.len());
        let scav = body(&mut solids, SCAV, Vec3::new(1.7, 0.4, 0.5), Color::srgb(0.30, 0.31, 0.27));
        let dead = Thing::container(Some(Interaction::DoorPullBackward), Some(Interaction::ContainerCloseDefault), 0.0, 0.0);
        things.push(container("dead Scav", dead, scav, Part::None, 3));
        end("scav", Vec3::new(SCAV[0], 0.0, SCAV[1]), at, &mut solids, &mut things, &mut props);
        // A wooden crate on the floor, its lid hinged along its far edge
        // (recorded on the floor: gestures 29 and 30, the lid from 0 to -30
        // degrees).
        let at = (solids.len(), things.len());
        let crate_size = Vec3::new(1.0, 0.44, 0.6);
        let crate_body = body(&mut solids, WOODEN_CRATE, crate_size, wood);
        made_of(&mut solids, WOOD);
        things.push(container("wooden crate", hinged_up(), crate_body, lid(WOODEN_CRATE, crate_size, Vec3::NEG_X, wood.darker(0.05)), 1));
        end("wooden crate", Vec3::new(WOODEN_CRATE[0], 0.0, WOODEN_CRATE[1]), at, &mut solids, &mut things, &mut props);
        // The tables of weapons, against the knee-high wall to the airdrop
        // crate's right: on each a green gun case with the class's weapons,
        // under it a black box with what loads them, its front with the
        // table's. Each opens as the range's crates do (the gestures
        // recorded for a lid hinged up, 29 and 30), its lid hinged along
        // its back edge, by the wall, and lifted towards it.
        let (case_green, box_black) = (Color::srgb(0.24, 0.33, 0.19), Color::srgb(0.05, 0.05, 0.06));
        for (index, name) in WEAPON_CRATES.into_iter().enumerate() {
            let begun = (solids.len(), things.len());
            // The medical table's case is light, and its two hold no
            // weapons: their names say what they do hold.
            let medical = name == MEDICAL;
            let case_green = if medical { Color::srgb(0.74, 0.73, 0.68) } else { case_green };
            let (case_word, box_word) = if of_its_own(name) { ("case", "box") } else { ("weapon case", "magazine box") };
            let box_name = box_name(name);
            let ([x, z], front) = weapon_table(index);
            let (along, middle) = (Vec3::Y.cross(front), Vec3::new(x, 0.0, z));
            // A box's size in the range's axes, from its length along the
            // table, its height and its depth from back to front.
            let sized = |size: Vec3| along.abs() * size.x + Vec3::Y * size.y + front.abs() * size.z;
            // The table: its top, and a leg at each corner.
            solids.push(block(middle + Vec3::Y * (WEAPON_TABLE.y - TABLE_SLAB / 2.0), sized(Vec3::new(WEAPON_TABLE.x, TABLE_SLAB, WEAPON_TABLE.z)), wood));
            made_of(&mut solids, WOOD);
            for (across, out) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let leg = middle + along * across * (WEAPON_TABLE.x - TABLE_LEG) / 2.0 + front * out * (WEAPON_TABLE.z - TABLE_LEG) / 2.0 + Vec3::Y * (WEAPON_TABLE.y - TABLE_SLAB) / 2.0;
                solids.push(block(leg, sized(Vec3::new(TABLE_LEG, WEAPON_TABLE.y - TABLE_SLAB, TABLE_LEG)), wood));
                made_of(&mut solids, WOOD);
            }
            // (Each thing's name is the prompt's, made once for the run.)
            let named = |of: &str, what: &str| -> &'static str { Box::leak(format!("{} {what}", of.to_lowercase()).into_boxed_str()) };
            let case = Solid { surface: PLASTIC, ..block(middle + Vec3::Y * (WEAPON_TABLE.y + GUN_CASE.y / 2.0), sized(GUN_CASE), case_green) };
            solids.push(case);
            let mut opens = hinged_up();
            opens.open_angle = -GUN_CASE_OPENS;
            let lid = lid_of(middle + Vec3::Y * (WEAPON_TABLE.y + GUN_CASE.y), GUN_CASE, front, case_green.darker(0.04));
            things.push(container(named(name, case_word), opens, case, lid, GUN_CASE_HOLDS + index));
            // The box under it, its front with the table's; a table of
            // weapons has its box of rounds beside it (the magazines' on
            // the right as one faces the table, the rounds' on the left).
            let rounds_too = index < AMMO_TABLES;
            let below = middle + front * (WEAPON_TABLE.z - MAGAZINE_BOX.z) / 2.0;
            let magazines_at = if rounds_too { below + along * BOX_ASIDE } else { below };
            let magazines = block(magazines_at + Vec3::Y * MAGAZINE_BOX.y / 2.0, sized(MAGAZINE_BOX), box_black);
            solids.push(magazines);
            let mut opens = hinged_up();
            opens.open_angle = -MAGAZINE_BOX_OPENS;
            let lid = lid_of(magazines_at + Vec3::Y * MAGAZINE_BOX.y, MAGAZINE_BOX, front, box_black.lighter(0.04));
            things.push(container(named(box_name, box_word), opens, magazines, lid, MAGAZINE_BOX_HOLDS + index));
            if rounds_too {
                // An olive box of rounds, the size of the magazines'.
                let rounds_olive = Color::srgb(0.20, 0.22, 0.13);
                let rounds_at = below - along * BOX_ASIDE;
                let rounds = block(rounds_at + Vec3::Y * MAGAZINE_BOX.y / 2.0, sized(MAGAZINE_BOX), rounds_olive);
                solids.push(rounds);
                let mut opens = hinged_up();
                opens.open_angle = -MAGAZINE_BOX_OPENS;
                let lid = lid_of(rounds_at + Vec3::Y * MAGAZINE_BOX.y, MAGAZINE_BOX, front, rounds_olive.lighter(0.04));
                things.push(container(named(name, "ammunition box"), opens, rounds, lid, AMMO_BOX_HOLDS + index));
            }
            end(&weapon_table_prop(index), middle, begun, &mut solids, &mut things, &mut props);
        }
        // A table and a shelf with things lying on them (`lying`), and
        // nothing more; and the rounds on the floor, which stand on nothing.
        let at = (solids.len(), things.len());
        body(&mut solids, TABLE, Vec3::new(1.0, TABLE_TOP, 0.6), wood);
        made_of(&mut solids, WOOD);
        end("table", Vec3::new(TABLE[0], 0.0, TABLE[1]), at, &mut solids, &mut things, &mut props);
        let at = (solids.len(), things.len());
        body(&mut solids, HIGH_SHELF, Vec3::new(0.9, SHELF_TOP, 0.35), wood);
        made_of(&mut solids, WOOD);
        end("high shelf", Vec3::new(HIGH_SHELF[0], 0.0, HIGH_SHELF[1]), at, &mut solids, &mut things, &mut props);
        let at = (solids.len(), things.len());
        end(ROUNDS_PROP, Vec3::new(ROUNDS[0], 0.0, ROUNDS[1]), at, &mut solids, &mut things, &mut props);
        // The small white table by the doorway, the yard door's key on it
        // (`key_lies`): its top, and a leg at each corner.
        let at = (solids.len(), things.len());
        let white = Color::srgb(0.9, 0.9, 0.88);
        let ([x, z], size) = (KEY_TABLE, KEY_TABLE_SIZE);
        solids.push(Solid { surface: WOOD, ..block(Vec3::new(x, size.y - TABLE_SLAB / 2.0, z), Vec3::new(size.x, TABLE_SLAB, size.z), white) });
        for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let leg = Vec3::new(x + dx * (size.x - TABLE_LEG) / 2.0, (size.y - TABLE_SLAB) / 2.0, z + dz * (size.z - TABLE_LEG) / 2.0);
            solids.push(Solid { surface: WOOD, ..block(leg, Vec3::new(TABLE_LEG, size.y - TABLE_SLAB, TABLE_LEG), white) });
        }
        end(KEY_TABLE_PROP, Vec3::new(x, 0.0, z), at, &mut solids, &mut things, &mut props);
        // A car at the row's right end, its back to the row: the trunk opens
        // with the default gestures, its lid from 0 to 60 degrees (recorded).
        let at = (solids.len(), things.len());
        let red = Color::srgb(0.45, 0.16, 0.14);
        let car = body(&mut solids, CAR, Vec3::new(4.2, 1.0, 1.8), red);
        made_of(&mut solids, METAL);
        solids.push(block(Vec3::new(CAR[0] + 0.1, 1.225, CAR[1]), Vec3::new(2.0, 0.45, 1.6), red.darker(0.1)));
        made_of(&mut solids, METAL);
        let trunk_lid = Part::Moving {
            centre: Vec3::new(CAR[0] - 1.6, 1.02, CAR[1]),
            size: Vec3::new(1.0, 0.04, 1.6),
            yaw: 0.0,
            pivot: Vec3::new(CAR[0] - 1.1, 1.02, CAR[1]),
            axis: Vec3::NEG_Z,
            slide: Vec3::ZERO,
            color: red.darker(0.05),
        };
        let trunk_hit = Solid { surface: METAL, ..block(Vec3::new(CAR[0] - 1.6, 0.52, CAR[1]), Vec3::new(1.0, 1.04, 1.8), red) };
        things.push(Placed {
            name: "trunk",
            thing: Thing::trunk(None, None, 0.0, 60.0),
            part: trunk_lid,
            hit: trunk_hit.block(),
            place: in_sim(car.centre),
            holds: None,
            leaf: None,
        });
        end("car", Vec3::new(CAR[0], 0.0, CAR[1]), at, &mut solids, &mut things, &mut props);
        // Past the car's front, the four of `courses10`'s takes, each facing
        // the door wall. A weapon box, a plastic case whose lid lifts 15
        // degrees, and a toolbox whose lid lifts 25 (their Customs prefabs'
        // angles, `plastic_suitcase_140_open_cover` and `scontainer_toolbox_
        // 01_Cup`; the weapon box's take turned it 0 to -15): each opened
        // with a lid's gesture, 29, as both takes recorded, and shut with
        // 30, the wooden crate's (`PROVISIONAL`: no take shut either).
        let at = (solids.len(), things.len());
        let plastic = Color::srgb(0.17, 0.18, 0.19);
        let weapon_box = body(&mut solids, WEAPON_BOX, WEAPON_BOX_SIZE - Vec3::Y * 0.04, plastic);
        made_of(&mut solids, PLASTIC);
        let mut opens = hinged_up();
        opens.open_angle = -15.0;
        things.push(container("weapon box", opens, weapon_box, lid(WEAPON_BOX, WEAPON_BOX_SIZE - Vec3::Y * 0.04, Vec3::NEG_X, plastic.lighter(0.03)), MORE_HOLDS));
        end("weapon box", Vec3::new(WEAPON_BOX[0], 0.0, WEAPON_BOX[1]), at, &mut solids, &mut things, &mut props);
        let at = (solids.len(), things.len());
        let tool_red = Color::srgb(0.55, 0.13, 0.10);
        let toolbox = body(&mut solids, TOOLBOX, TOOLBOX_SIZE - Vec3::Y * 0.04, tool_red);
        made_of(&mut solids, PLASTIC);
        let mut opens = hinged_up();
        opens.open_angle = -25.0;
        things.push(container("toolbox", opens, toolbox, lid(TOOLBOX, TOOLBOX_SIZE - Vec3::Y * 0.04, Vec3::NEG_X, tool_red.darker(0.05)), MORE_HOLDS + 1));
        end("toolbox", Vec3::new(TOOLBOX[0], 0.0, TOOLBOX[1]), at, &mut solids, &mut things, &mut props);
        // A safe, its door on its front hinged on the right as one faces
        // it and pulled 40 degrees open (its Customs prefab's `safe_01_
        // boor`; the take's door 0 to 40), opened with the gesture the take
        // recorded, 19, and shut with the default (`PROVISIONAL`: the take
        // never shut it).
        let at = (solids.len(), things.len());
        let steel = Color::srgb(0.26, 0.28, 0.27);
        let safe = body(&mut solids, SAFE, SAFE_SIZE, steel);
        made_of(&mut solids, METAL);
        let front = SAFE[1] - SAFE_SIZE.z / 2.0 - SAFE_DOOR.z / 2.0;
        let door = Part::Moving {
            centre: Vec3::new(SAFE[0], 0.01 + SAFE_DOOR.y / 2.0, front),
            size: SAFE_DOOR,
            yaw: 0.0,
            pivot: Vec3::new(SAFE[0] - SAFE_DOOR.x / 2.0, 0.01 + SAFE_DOOR.y / 2.0, front),
            axis: Vec3::Y,
            slide: Vec3::ZERO,
            color: steel.lighter(0.04),
        };
        things.push(container("safe", Thing::container(Some(Interaction::PullHingeRight), None, 0.0, 40.0), safe, door, MORE_HOLDS + 2));
        end("safe", Vec3::new(SAFE[0], 0.0, SAFE[1]), at, &mut solids, &mut things, &mut props);
        // An airdrop weapon crate, which nothing of moves; its gestures the
        // defaults, 27 and 28, as the range's old airdrop crate's
        // (`PROVISIONAL`: its take began open).
        let at = (solids.len(), things.len());
        let olive = Color::srgb(0.29, 0.31, 0.21);
        let airdrop = body(&mut solids, AIRDROP_CRATE, AIRDROP_CRATE_SIZE, olive);
        made_of(&mut solids, WOOD);
        things.push(container("airdrop weapon crate", Thing::container(None, None, 0.0, 0.0), airdrop, Part::None, MORE_HOLDS + 3));
        end("airdrop crate", Vec3::new(AIRDROP_CRATE[0], 0.0, AIRDROP_CRATE[1]), at, &mut solids, &mut things, &mut props);
        // Two doors in the doorway, hinged at its two ends and meeting in
        // its middle, each turning 110 degrees towards the start (recorded:
        // the doors' angles were 0 and 110). Their leaves lie along the
        // wall's face on the start's side. (In the simulation's axes the
        // start's side is +z, and a door turns towards it about straight
        // down from the left hinge and straight up from the right.)
        let at = (solids.len(), things.len());
        let face = DOOR_WALL_Z - WALL_THICK / 2.0 + LEAF[2] / 2.0;
        let doors = [
            (DOORWAY[0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            (DOORWAY[1], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ];
        for (side, forward, axis) in doors {
            let hinge = Hinge { at: in_sim(Vec3::new(side, 0.0, face)), axis, forward };
            let thing = Thing::door(hinge, 0.0, 110.0);
            let leaf = leaf_block(&thing);
            things.push(Placed {
                name: "door",
                part: Part::Leaf { size: Vec3::from(LEAF), color: Color::srgb(0.36, 0.30, 0.24) },
                thing,
                hit: leaf,
                place: hinge.at,
                holds: None,
                leaf: None,
            });
        }
        end("doors", Vec3::new((DOORWAY[0] + DOORWAY[1]) / 2.0, 0.0, DOOR_WALL_Z), at, &mut solids, &mut things, &mut props);
        // The barbed wire and the sampler, which have no solids.
        for (name, anchor) in [(WIRE_PROP, wire_anchor()), (SAMPLER_PROP, sampler_anchor())] {
            let at = (solids.len(), things.len());
            end(name, anchor, at, &mut solids, &mut things, &mut props);
        }

        // The character's collision: the solids, then the doors' leaves as
        // they stand.
        let mut blocks: Vec<Block> = solids.iter().filter(|solid| !solid.overhead).map(Solid::block).collect();
        for placed in things.iter_mut().filter(|placed| matches!(placed.part, Part::Leaf { .. })) {
            placed.hit = leaf_block(&placed.thing);
            blocks.push(placed.hit);
            placed.leaf = Some(blocks.len() - 1);
        }
        // The workbench where its prop stands: its model's top and its
        // triggers.
        let benches = BenchKind::ALL.into_iter().filter(|kind| kind.on()).map(|kind| {
            let anchor = kind.anchor();
            let stand = props.iter().find(|prop| prop.name == kind.prop()).map_or(crate::world::level::Stand::made(anchor), |prop| prop.stand);
            let made = kind.made();
            let place = Transform::from_translation(stand.point(anchor, made.translation)).with_rotation(Quat::from_rotation_y(stand.yaw) * made.rotation);
            let (low, high) = kind.trigger();
            let corners: Vec<Vec3> = (0..8).map(|k| stand.point(anchor, kind.point([if k & 1 == 0 { low[0] } else { high[0] }, if k & 2 == 0 { low[1] } else { high[1] }, if k & 4 == 0 { low[2] } else { high[2] }]))).collect();
            let trigger = [corners.iter().copied().fold(Vec3::INFINITY, Vec3::min), corners.iter().copied().fold(Vec3::NEG_INFINITY, Vec3::max)];
            Workbench { kind, place, trigger }
        });
        let benches = benches.collect();
        // The game's figure, until `main` has read it from the install.
        Self { solids, props, level, radius: 0.366, site: Site { ground: 0.0, blocks, others: Vec::new() }, things, thing_settings: None, benches }
    }

    /// The range's own boxes, without a level file: the ring of blocks,
    /// the walls, the cover in the lanes, the stairs, the platforms and the ledge.
    fn own_boxes(solids: &mut Vec<Solid>) {
        let block = |name: &'static str, centre: Vec3, size: Vec3, color: Color| Solid { name, centre, size, yaw: 0.0, color, surface: trp_sim::sound::steps::CONCRETE, overhead: false };
        // A ring of blocks all round, to judge turning against. The eighth
        // stands 15 degrees on from its place, out of the second flight's
        // way to the ledge.
        for i in 0..12 {
            let angle = (i as f32 * 30.0 + if i == 7 { 15.0 } else { 0.0 }).to_radians();
            solids.push(block(
                "ring block",
                Vec3::new(angle.sin() * RING, 0.5, angle.cos() * RING),
                Vec3::ONE,
                Color::hsl(i as f32 * 30.0, 0.5, 0.45),
            ));
        }
        // The two walls across the lane: behind the start the door wall,
        // with its doorway and a lintel over it and a window to its right;
        // ahead the knee-high one, to vault and to lie at. (No block a metre
        // high off to the right, to climb: the ring's blocks are as high.)
        // Light concrete greys for the walls, the stairs and the platforms
        //: the faces turned from
        // the sun are what read darkest under the camera's exposure.
        let grey = HALL_GREY;
        let span = |name: &'static str, z: f32, left: f32, right: f32, low: f32, high: f32| {
            block(name, Vec3::new((left + right) / 2.0, (low + high) / 2.0, z), Vec3::new(right - left, high - low, WALL_THICK), grey)
        };
        solids.push(span("door wall", DOOR_WALL_Z, -WALL_HALF, DOORWAY[0], 0.0, WALL_HIGH));
        solids.push(span("door wall", DOOR_WALL_Z, DOORWAY[1], WINDOW[0], 0.0, WALL_HIGH));
        solids.push(span("window sill", DOOR_WALL_Z, WINDOW[0], WINDOW[1], 0.0, SILL));
        solids.push(span("window lintel", DOOR_WALL_Z, WINDOW[0], WINDOW[1], LINTEL, WALL_HIGH));
        solids.push(span("door wall", DOOR_WALL_Z, WINDOW[1], WALL_HALF, 0.0, WALL_HIGH));
        solids.push(span("doorway lintel", DOOR_WALL_Z, DOORWAY[0], DOORWAY[1], LEAF[1] + 0.05, WALL_HIGH));
        solids.push(span("knee-high wall", KNEE_WALL_Z, KNEE_LEFT, KNEE_RIGHT, 0.0, KNEE_HIGH));
        // Cover down the lanes (no hall about them).
        for (name, centre, size, turn) in COVER {
            solids.push(Solid { yaw: turn.to_radians(), ..block(name, centre, size, HALL_GREY) });
        }
        // Somewhere to fall from, to the left of the start: stairs going the
        // lane's way up to a platform, and a lower platform on its right,
        // toward the knee-high wall (the stairs on the platform's left half
        // and the lower platform on its right, as TRP's range has them).
        let floor = |name: &'static str, low: Vec3, high: Vec3, color: Color| block(name, (low + high) / 2.0, high - low, color);
        let (left, stair_right, tread) = (STAIRS_LEFT, STAIRS_LEFT + 2.5, TREAD);
        for i in 0..STAIRS {
            let top = PLATFORM * (i + 1) as f32 / (STAIRS + 1) as f32;
            let near = STAIRS_FOOT - (i as f32) * tread;
            solids.push(floor(
                "stairs step",
                Vec3::new(left, 0.0, near - tread),
                Vec3::new(stair_right, top, near),
                Color::srgb(0.56, 0.56, 0.56 + 0.02 * (i % 2) as f32),
            ));
        }
        let far = STAIRS_FOOT - (STAIRS as f32) * tread;
        solids.push(floor("platform", Vec3::new(left, 0.0, far - 5.0), Vec3::new(left + 5.0, PLATFORM, far), Color::srgb(0.52, 0.52, 0.54)));
        solids.push(floor("lower platform", Vec3::new(left + 5.0, 0.0, far - 5.0), Vec3::new(left + 8.0, SHELF, far), Color::srgb(0.52, 0.54, 0.52)));
        // From the platform's far edge, on the half the first flight comes up
        // on, a second flight on the way the first goes, to the ledge beyond:
        // straight on from the start's side, no turning back.
        let rise = (LEDGE - PLATFORM) / LEDGE_STEPS as f32;
        let (flight_left, flight_right, edge) = (left, stair_right, far - 5.0);
        for i in 0..LEDGE_STEPS {
            let top = PLATFORM + rise * (i + 1) as f32;
            let near = edge - i as f32 * tread;
            solids.push(floor("flight step", Vec3::new(flight_left, 0.0, near - tread), Vec3::new(flight_right, top, near), Color::srgb(0.56, 0.56, 0.56 + 0.02 * (i % 2) as f32)));
        }
        let ledge = edge - LEDGE_STEPS as f32 * tread;
        solids.push(floor("ledge", Vec3::new(flight_left, 0.0, ledge - 3.0), Vec3::new(flight_right, LEDGE, ledge), Color::srgb(0.50, 0.50, 0.52)));
    }
}
