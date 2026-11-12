//! Tests that read a weapon's assembly, pictures, cases, ports, marker and glass.

use super::*;

#[test]
fn stock_akm_assembles_with_every_part() {
    let Some((assets, weapon)) = akm() else { return };
    assert!(weapon.skipped.is_empty(), "parts not placed: {:?}", weapon.skipped);

    let rendered: Vec<_> = weapon.hierarchy.nodes.iter().filter(|n| n.renderer.is_some()).collect();
    // Seven meshes on the base rifle and one for each of the eight parts.
    assert_eq!(rendered.len(), 15);
    assert!(rendered.iter().all(|n| n.name.ends_with("LOD0")), "a lower detail mesh was kept");

    for node in rendered {
        let renderer = node.renderer.as_ref().unwrap();
        let mesh = read_mesh(&assets, renderer.mesh).expect("mesh");
        assert!(!mesh.positions.is_empty(), "{} has no vertices", node.name);
        assert_eq!(mesh.normals.len(), mesh.positions.len());
        assert_eq!(mesh.uv0.len(), mesh.positions.len());
        let vertex_count = mesh.positions.len() as u32;
        assert!(mesh.submeshes.iter().flatten().all(|&i| i < vertex_count));
        for material in renderer.materials.iter().flatten() {
            let material = trp_data::unity::extract::read_material(&assets, *material).expect("material");
            let main = material.textures.get("_MainTex").expect("main texture");
            let texture = read_texture(&assets, *main).expect("texture");
            assert!(texture.width > 0 && !texture.data.is_empty());
        }
    }
}

/// A weapon of one's own's pictures, compressed to be drawn with
/// (`custom::Picture::compressed`): how far each channel of the largest
/// level comes back from what was read, as a root mean square in 255 and
/// the share of pixels more than eight off. Passes with a note where no
/// weapon of one's own has its files here.
#[test]
fn a_weapon_of_one_s_own_s_pictures_compress_closely() {
    use trp_data::formats::compress::{bc1_decoded, bc5_decoded};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let mut checked = 0;
    for template in db.custom.keys() {
        let Ok(weapon) = assemble(&mut assets, &db, template) else { continue };
        let Some(build) = weapon.custom.as_ref() else { continue };
        for material in &build.materials {
            let Ok(read) = trp_data::weapons::custom::pictures(material) else {
                eprintln!("skipped: the pictures of {} are not here", material.name);
                continue;
            };
            for (what, picture) in [("colour", read.colour), ("normal", read.normal), ("surface", read.surface)] {
                let Some(picture) = picture else { continue };
                let (width, height, channels) = (picture.width, picture.height, picture.channels);
                let first = width as usize * height as usize * channels;
                let original = picture.data[..first].to_vec();
                let packed = picture.compressed();
                let blocks = &packed.data[..trp_data::formats::compress::level_bytes(width, height, if channels == 4 { 8 } else { 16 })];
                let back = if channels == 4 { bc1_decoded(width, height, blocks) } else { bc5_decoded(width, height, blocks) };
                let kept = if channels == 4 { 3 } else { 2 };
                let mut line = format!("{} {what} {width}x{height}:", material.name);
                for channel in 0..kept {
                    let (mut sum, mut off) = (0.0f64, 0usize);
                    for pixel in 0..(width * height) as usize {
                        let d = original[pixel * channels + channel] as f64 - back[pixel * channels + channel] as f64;
                        sum += d * d;
                        off += (d.abs() > 8.0) as usize;
                    }
                    let rms = (sum / (width * height) as f64).sqrt();
                    line += &format!(" channel {channel} {rms:.2} ({:.1}% over 8)", off as f64 * 100.0 / (width * height) as f64);
                    assert!(rms < 6.0, "{line}");
                }
                println!("{line}");
                checked += 1;
            }
        }
    }
    println!("{checked} pictures checked");
}

/// A spent case's figures (`ShellExtractionData`, `shellport`,
/// `AmmoPoolObject._caliber`): the AKM throws its cases by its shot
/// settings from a shell port of its own; its 5.45 BP round is of the
/// 5.56 mm class.
#[test]
fn a_weapon_s_case_throw_and_a_round_s_calibre_are_read() {
    let Some((mut assets, akm)) = akm() else { return };
    let extraction = akm.extraction.expect("the AKM's shell extraction");
    let port = akm.shell_port.expect("the AKM's shell port");
    println!("extraction {extraction:?}, shell port {} at {:?}", akm.hierarchy.nodes[port].name, akm.hierarchy.nodes[port].translation);
    assert!(extraction.force.iter().any(|range| range[0] != 0.0 || range[1] != 0.0));
    // A live round's own throw (`PatronExtractionSettings`), read beside it.
    let patron = akm.patron_extraction.expect("the AKM's round's throw");
    println!("a round's throw {patron:?}");
    assert!(patron.force.iter().any(|range| range[0] != 0.0 || range[1] != 0.0));
    let files = assets.load("assets/content/items/ammo/patrons/patron_545x39_bp.bundle").expect("the round");
    assert_eq!(trp_data::weapons::weapon::round_caliber(&assets, &files), Some(2));
}

/// The shell ports of weapons of several chambers, by the chambers'
/// numbers (more-weapons spec, "The rounds and cases seen in chambers of
/// their own"): the MP-43-1C's two in its barrels before the weapon's own
/// plain one, the Rhino 50DS's six in its cylinder; the MP-18, of one
/// chamber, has its one `shellport`. Each throws by its shot settings.
#[test]
fn the_ports_of_weapons_of_several_chambers_are_read() {
    let Ok(install) = Install::locate() else { return };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let names = |weapon: &Weapon| weapon.shell_ports.iter().map(|&index| weapon.hierarchy.nodes[index].name.clone()).collect::<Vec<_>>();
    let mp43 = assemble(&mut assets, &db, "5580223e4bdc2d1c128b457f").expect("the MP-43-1C");
    assert_eq!(names(&mp43), ["shellport_000", "shellport_001", "shellport"]);
    assert!(mp43.extraction.is_some());
    let rhino = assemble(&mut assets, &db, "61a4c8884f95bc3b2c5dc96f").expect("the Rhino 50DS");
    assert_eq!(names(&rhino), (1..=6).map(|n| format!("shellport_00{n}")).collect::<Vec<_>>());
    assert!(rhino.extraction.is_some());
    let mp18 = assemble(&mut assets, &db, "61f7c9e189e6fb1a5e3ea78d").expect("the MP-18");
    assert!(mp18.shell_ports.is_empty());
    assert!(mp18.shell_port.is_some() && mp18.extraction.is_some());
    // Each throws a live round from its first port by its own figures.
    for (name, weapon) in [("MP-43-1C", &mp43), ("Rhino", &rhino), ("MP-18", &mp18)] {
        println!("the {name}'s case's throw {:?}, a round's {:?}", weapon.extraction, weapon.patron_extraction);
        assert!(weapon.patron_extraction.is_some());
    }
}

/// Whether a rig links its vest's marker for the player (`TransformLinks`,
/// entry 18; third-person spec, layer 2): the AKM's and the empty hands' do,
/// so a character seen from outside reaches for the vest with them; the
/// Slickers' hands for eating do not (`courses8`'s eating: the layer at
/// nought from the item's first frame).
#[test]
fn the_vest_marker_is_linked_on_a_weapon_not_on_an_items_hands() {
    let Some((mut assets, akm)) = akm() else { return };
    assert!(akm.vest_linked, "the AKM's rig links its vest marker");
    let empty = trp_data::weapons::weapon::assemble_prefab(&mut assets, trp_data::weapons::weapon::EMPTY_HANDS).expect("the empty hands");
    assert!(empty.vest_linked, "the empty hands' rig links its vest marker");
    let slickers = trp_data::weapons::weapon::assemble_prefab(&mut assets, "assets/content/weapons/usable_items/item_energy_bar/item_slickers_container.bundle").expect("the Slickers' hands");
    assert!(!slickers.vest_linked, "an item's hands link no vest marker");
}

/// The window magazine's glass (icons spec, "The game's surface in a
/// picture"; body-animator spec, "The shield's glass"): the PMAG GEN M3
/// Window's second material is the dithered glass with the figures as read
/// (`local\glass_icon_brief_s240.md`), its specular picture and
/// its cube; its colour picture is the body's own, whose alpha is never
/// over 107 (0.42): in the world always the dithered half, in its picture
/// (its `_Icon` twin, which takes no `_OpacityScale`) blended at that alpha
/// or less, the rounds seen through it.
#[test]
fn the_window_magazine_s_glass_is_read() {
    use trp_data::character::dress::{load, Under, Worn};
    use trp_data::drawing::glass::GlassFigures;
    use trp_data::unity::extract::read_material;
    use trp_data::unity::texture;
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    assets.load("cubemaps").expect("the cubemaps");
    let key = db.items["55802d5f4bdc2dac148b458e"].prefab_key().expect("its prefab").to_owned();
    let Worn::Rigid(model) = load(&mut assets, &key, Under::default()).expect("the model") else { panic!("a model") };
    let mut glass = None;
    let mut others = 0;
    for material in model.nodes.iter().filter_map(|node| node.renderer.as_ref()).flat_map(|renderer| renderer.materials.iter().flatten()) {
        let data = read_material(&assets, *material).expect("its material");
        match GlassFigures::of(&data) {
            Some(figures) => glass = Some((data, figures)),
            None => others += 1,
        }
    }
    let (data, figures) = glass.expect("its glass");
    assert!(others > 0, "its body is not the glass");
    assert_eq!(data.name, "mag_stanag_magpul_pmag_gen_m3_window_556x45_30_LOD0_glass");
    let near = |a: f32, b: f32| (a - b).abs() < 0.0006;
    assert!(near(figures.color[0], 0.8962) && near(figures.color[3], 1.0), "{:?}", figures.color);
    assert!(near(figures.spec_color[0], 0.9412) && near(figures.reflect_color[0], 1.0) && near(figures.reflect_color[3], 0.5), "{figures:?}");
    assert!(near(figures.opacity_scale, 1.0) && near(figures.spec_power, 0.17) && near(figures.glossness, 1.5) && near(figures.global_reflection_strength, 1.0) && figures.cull == 2, "{figures:?}");
    assert!(data.textures.contains_key("_SpecTex") && data.textures.contains_key("_Cube"), "its specular picture and its cube");
    let colour = read_texture(&assets, data.textures["_MainTex"]).expect("its colour picture");
    let level = texture::mips(&colour).expect("its levels")[0];
    let rgba = texture::decode_rgba(&colour, level).expect("decoded");
    let most = rgba.chunks_exact(4).map(|px| px[3]).max().expect("pixels");
    println!("{}: alpha at most {most}", colour.name);
    assert!(most <= 107, "its alpha at most 107, the dithered half everywhere: {most}");
}
