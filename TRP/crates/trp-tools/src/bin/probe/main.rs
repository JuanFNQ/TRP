//! Inspection tool for the install's bundles.
//!
//! ```text
//! probe find <text>                 manifest keys containing <text>
//! probe info <key>                  files, type-tree flag and object counts
//! probe names <key> [class]         objects with their names
//! probe dump <key> <path_id> [depth] [items]
//! probe tree <key>                  GameObject hierarchy with components
//! probe scripts <key>               MonoBehaviours with script class and owner
//! probe preset <weapon template id> stock build of a weapon with prefab bundles
//! probe fbx <file> [records|view|slice ...] [--from <node>] [--turn x,y,z]   a binary FBX file: its models, a drawing, a slab (in a node's axes, turned)
//! probe fbx <file> motion [every] [bone ...]   its take: where the bones named are on every so many frames
//! probe overlay <weapon> <fbx> <trigger model> <out.png>   a weapon with such a model over it
//! probe texture <key> <path_id> <out.png>   one of a bundle's pictures, its largest level
//! probe custom [id]                 the weapons that are not the install's, and one as built
//! probe muzzles [calibre]           every weapon's muzzle effects, where they are from its fireport
//! ```

use anyhow::{bail, Result};
use trp_data::unity::Assets;
use trp_data::Install;

mod printing;
mod animators;
mod body;
mod bundles;
mod clips;
mod custom;
mod fonts;
mod icons;
mod level;
mod other_formats;
mod panels;
mod particles;
mod player_settings;
mod prefabs;
mod scene_objects;
mod shaders_textures;
mod sounds;
mod voices_mixer;
mod weapon_parts;
mod weapons;
mod windows;


fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--from <node>` anywhere: an FBX file's scene in that node's axes
    // and units (`fbx::Scene::seen_from`).
    let mut option = |name: &str| {
        args.iter().position(|a| a == name).filter(|&at| at + 1 < args.len()).map(|at| {
            let value = args.remove(at + 1);
            args.remove(at);
            value
        })
    };
    let fbx_from = option("--from");
    // `--turn x,y,z`: and then turned by those degrees (`fbx::Scene::turned`).
    let fbx_turn: Option<[f64; 3]> = option("--turn").and_then(|t| {
        let v: Vec<f64> = t.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    });
    let arg = |i: usize| args.get(i).map(String::as_str);
    let install = Install::locate()?;
    let mut assets = Assets::new(install)?;

    match (arg(0), arg(1)) {
        (Some("file"), Some(file)) => scene_objects::file(&mut assets, file)?,
        (Some("staticicons"), _) => icons::staticicons(&mut assets)?,
        (Some("infowindow"), width) => windows::infowindow(&mut assets, width)?,
        (Some("fontscale"), _) => fonts::fontscale(&mut assets)?,
        (Some("captionfit"), Some(cells)) => fonts::captionfit(&mut assets, &args, cells)?,
        (Some("tmptext"), Some(words)) => fonts::tmptext(&mut assets, &args, words)?,
        (Some("healthpanel"), width) => panels::healthpanel(&mut assets, width)?,
        (Some("sceneclips"), Some(file)) => scene_objects::sceneclips(&mut assets, &args, file)?,
        (Some("classfind"), Some(class)) => scene_objects::classfind(&mut assets, class)?,
        (Some("scenepanel"), Some(file)) => panels::scenepanel(&mut assets, &args, file)?,
        (Some("battle"), _) => panels::battle(&mut assets)?,
        (Some("textwidth"), _) => fonts::textwidth(&mut assets, &args)?,
        (Some("gridwindow"), _) => windows::gridwindow(&mut assets)?,
        (Some("quick"), _) => panels::quick(&mut assets)?,
        (Some("stash"), _) => panels::stash(&mut assets)?,
        (Some("points"), Some(file)) => scene_objects::points(&mut assets, &args, file)?,
        (Some("parts"), Some(file)) => scene_objects::parts(&mut assets, &args, file)?,
        (Some("editbuild"), _) => panels::editbuild(&mut assets)?,
        (Some("sceneprops"), Some(scene)) => level::sceneprops(&mut assets, &args, scene)?,
        (Some("uisounds"), _) => sounds::uisounds(&mut assets)?,
        (Some("thingsounds"), first) => sounds::thingsounds(&mut assets, &args, first)?,
        (Some("dropsounds"), read) => sounds::dropsounds(&mut assets, read)?,
        (Some("stepsounds"), read) => sounds::stepsounds(&mut assets, &args, read)?,
        (Some("partstates"), Some(template)) => weapons::partstates(&mut assets, template)?,
        (Some("partbinds"), Some(template)) => weapons::partbinds(&mut assets, template)?,
        (Some("bipodaudio"), Some(template)) => sounds::bipodaudio(&mut assets, &args, template)?,
        (Some("handsounds"), Some(what)) => sounds::handsounds(&mut assets, &args, what)?,
        (Some("voicecost"), _) => voices_mixer::voicecost(&mut assets)?,
        (Some("voice"), _) => voices_mixer::voice(&mut assets, &args)?,
        (Some("vorbis"), _) => voices_mixer::vorbis(&mut assets, &args)?,
        (Some("sounds"), Some(folder)) => sounds::sounds(&mut assets, &args, folder)?,
        (Some("scriptptrs"), Some(file)) => scene_objects::scriptptrs(&mut assets, &args, file)?,
        (Some("textmats"), _) => fonts::textmats(&mut assets)?,
        (Some("fontmaterial"), Some(file)) => fonts::fontmaterial(&mut assets, &args, file)?,
        (Some("build-objects"), Some(file)) => scene_objects::build_objects(&mut assets, &args, file)?,
        (Some("typed"), Some(file)) => scene_objects::typed(&mut assets, &args, file)?,
        (Some("externals"), Some(file)) => scene_objects::externals(&mut assets, file)?,
        (Some("unpack"), Some(file)) => scene_objects::unpack(&mut assets, &args, file)?,
        (Some("raw"), Some(file)) => scene_objects::raw(&mut assets, &args, file)?,
        (Some("textwidths"), Some(file)) => fonts::textwidths(&mut assets, file)?,
        (Some("rsprite"), Some(path)) => prefabs::rsprite(&mut assets, &args, path)?,
        (Some("nsprite"), Some(file)) => prefabs::nsprite(&mut assets, &args, file)?,
        (Some("skillspage"), _) => windows::skillspage(&mut assets)?,
        (Some("model"), Some(key)) => prefabs::model(&mut assets, key)?,
        (Some("masteringpage"), _) => windows::masteringpage(&mut assets)?,
        (Some("tabs"), _) => panels::tabs(&mut assets)?,
        (Some("sprite"), Some(file)) => prefabs::sprite(&mut assets, &args, file)?,
        (Some("prefab"), Some(wanted)) => prefabs::prefab(&mut assets, &args, wanted)?,
        (Some("panelimages"), Some(which)) => prefabs::panelimages(&mut assets, &args, which)?,
        (Some("containers"), layout) => windows::containers(&mut assets, layout)?,
        (Some("resources-paths"), filter) => prefabs::resources_paths(&mut assets, filter)?,
        (Some("fonts"), Some(file)) => fonts::fonts(&mut assets, file)?,
        (Some("clouds"), cloudiness) => level::clouds(&mut assets, cloudiness)?,
        (Some("level-colliders"), Some(x)) => level::level_colliders(&mut assets, &args, x)?,
        (Some("level-ballistics"), filter) => level::level_ballistics(&mut assets, &args, filter)?,
        (Some("level-light"), scene) => level::level_light(&mut assets, &args, scene)?,
        (Some("scenes"), filter) => level::scenes(&mut assets, filter)?,
        (Some("keys"), filter) => player_settings::keys(&mut assets, filter)?,
        (Some("screen"), Some(file)) => scene_objects::screen(&mut assets, &args, file)?,
        (Some("fields"), Some(file)) => scene_objects::fields(&mut assets, &args, file)?,
        (Some("pointers"), Some(file)) => scene_objects::pointers(&mut assets, &args, file)?,
        (Some("popper"), _) => windows::popper(&mut assets)?,
        (Some("player-script-values"), Some(wanted)) => player_settings::player_script_values(&mut assets, wanted)?,
        (Some("player-scripts"), Some(wanted)) => player_settings::player_scripts(&mut assets, wanted)?,
        (Some("bundle-layers"), key) => level::bundle_layers(&mut assets, key)?,
        (Some("layers"), _) => level::layers(&mut assets)?,
        (Some("ragdoll"), _) => player_settings::ragdoll(&mut assets)?,
        (Some("object"), Some(id)) => prefabs::object(&mut assets, id)?,
        (Some("resources"), Some(wanted)) => prefabs::resources(&mut assets, &args, wanted)?,
        (Some("studio"), _) => windows::studio(&mut assets)?,
        (Some("tagwindow"), Some("placed")) => windows::tagwindow_placed(&mut assets)?,
        (Some("tagwindow"), _) => windows::tagwindow(&mut assets)?,
        (Some("prefabfields"), Some(wanted)) => prefabs::prefabfields(&mut assets, &args, wanted)?,
        (Some("scenescript"), Some(file)) => scene_objects::scenescript(&mut assets, &args, file)?,
        (Some("scriptbytes"), Some(window @ ("tagwindow" | "splitdialog"))) => prefabs::scriptbytes(&mut assets, &args, window)?,
        (Some("prefabdicts"), Some(wanted)) => prefabs::prefabdicts(&mut assets, &args, wanted)?,
        (Some("contextmenu"), _) => windows::contextmenu(&mut assets)?,
        (Some("icon"), Some(wanted)) => icons::icon(&mut assets, wanted)?,
        (Some("iconmaterials"), Some(wanted)) => icons::iconmaterials(&mut assets, wanted)?,
        (Some("iconbounds"), Some(wanted)) => icons::iconbounds(&mut assets, wanted)?,
        (Some("with"), Some(prop)) => level::with(&mut assets, prop)?,
        (Some("props"), Some(wanted)) => level::props(&mut assets, &args, wanted)?,
        (Some("item"), Some(wanted)) => icons::item(&mut assets, wanted)?,
        (Some("json"), Some(file)) => bundles::json(&mut assets, &args, file)?,
        (Some("find"), Some(text)) => bundles::find(&mut assets, text)?,
        (Some("info"), Some(key)) => bundles::info(&mut assets, key)?,
        (Some("names"), Some(key)) => bundles::names(&mut assets, &args, key)?,
        (Some("shaders"), wanted) => shaders_textures::shaders(&mut assets, &args, wanted)?,
        (Some("compute"), Some(file)) => shaders_textures::compute(&mut assets, &args, file)?,
        (Some("dump"), Some(key)) => bundles::dump(&mut assets, &args, key)?,
        (Some("foldsights"), _) => weapon_parts::foldsights(&mut assets)?,
        (Some("script"), Some(key)) => bundles::script(&mut assets, &args, key)?,
        (Some("slotbones"), word) => weapon_parts::slotbones(&mut assets, word)?,
        (Some("preset"), Some(weapon)) => weapons::preset(&mut assets, weapon)?,
        (Some("notice"), _) => windows::notice(&mut assets)?,
        (Some("usingpanel"), _) => windows::usingpanel(&mut assets)?,
        (Some("hot"), Some(key)) => player_settings::hot(&mut assets, key)?,
        (Some("repairwindow"), _) => windows::repairwindow(&mut assets)?,
        (Some("splitdialog"), Some("laid")) => windows::splitdialog_laid(&mut assets, &args)?,
        (Some("splitdialog"), _) => windows::splitdialog(&mut assets)?,
        (Some("glow"), _) => windows::glow(&mut assets)?,
        (Some("inspectpreview"), _) => windows::inspectpreview(&mut assets)?,
        (Some("devices"), Some(wanted)) => weapon_parts::devices(&mut assets, wanted)?,
        (Some("sights"), Some(wanted)) => weapon_parts::sights(&mut assets, wanted)?,
        (Some("weapon"), Some(template)) => weapons::weapon(&mut assets, &args, template)?,
        (Some("ogf"), Some(wanted)) => other_formats::ogf(wanted)?,
        (Some("xdb"), Some(part)) => other_formats::xdb(&args, part)?,
        (Some("skin"), Some(key)) => body::skin(&mut assets, key)?,
        (Some("weapons"), filter) => weapons::weapons(&mut assets, filter)?,
        (Some("grenade"), Some(bundle)) => weapons::grenade(&mut assets, bundle)?,
        (Some("paramuse"), Some(wanted)) => animators::paramuse(&mut assets, &args, wanted)?,
        (Some("knives"), _) => weapons::knives(&mut assets)?,
        (Some("states"), Some(template)) => animators::states(&mut assets, &args, template)?,
        (Some("controller"), Some(template)) => animators::controller(&mut assets, &args, template)?,
        (Some("engine"), _) => player_settings::engine(&mut assets, &args)?,
        (Some("hardsettings"), filter) => player_settings::hardsettings(&mut assets, filter)?,
        (Some("holding"), Some(rel)) => scene_objects::holding(&mut assets, &args, rel)?,
        (Some("spriteptrs"), Some(rel)) => scene_objects::spriteptrs(&mut assets, &args, rel)?,
        (Some("rawfile"), Some(rel)) => scene_objects::rawfile(&mut assets, &args, rel)?,
        (Some("rootmotion"), filter) => animators::rootmotion(&mut assets, &args, filter)?,
        (Some("clips"), Some(template)) => clips::clips(&mut assets, &args, template)?,
        (Some("blendcheck"), filter) => clips::blendcheck(&mut assets, filter)?,
        (Some("bodystates"), filter) => body::bodystates(&mut assets, &args, filter)?,
        (Some("bodyclips"), Some(filter)) => body::bodyclips(&mut assets, &args, filter)?,
        (Some("bc7bench"), Some(key)) => shaders_textures::bc7bench(&mut assets, &args, key)?,
        (Some("texstats"), Some(key)) => shaders_textures::texstats(&mut assets, &args, key)?,
        (Some("colliders"), Some(template)) => weapons::colliders(&mut assets, template)?,
        (Some("itemviews"), Some("loaders")) => panels::itemviews_loaders(&mut assets)?,
        (Some("itemviews"), _) => panels::itemviews(&mut assets, &args)?,
        (Some("aimsounds"), _) => sounds::aimsounds(&mut assets)?,
        (Some("follow"), Some(key)) => bundles::follow(&mut assets, &args, key)?,
        (Some("objects"), Some(key)) => bundles::objects(&mut assets, key)?,
        (Some("mixer"), _) => voices_mixer::mixer(&mut assets)?,
        (Some("tree"), Some(key)) => bundles::tree(&mut assets, key)?,
        (Some("animator"), Some(key)) => animators::animator(&mut assets, key)?,
        (Some("fittings"), which) => body::fittings(&mut assets, which)?,
        (Some("scripts"), Some(key)) => bundles::scripts(&mut assets, key)?,
        (Some("overlay"), Some(template)) => custom::overlay(&mut assets, &args, template)?,
        (Some("collisions"), key) => level::collisions(&mut assets, key)?,
        (Some("particles"), Some(key)) => particles::particles(&mut assets, &args, key)?,
        (Some("muzzles"), wanted) => particles::muzzles(&mut assets, wanted)?,
        (Some("custom"), which) => custom::custom(&mut assets, which)?,
        (Some("texture"), Some(key)) => bundles::texture(&mut assets, &args, key)?,
        (Some("fbx"), Some(file)) => other_formats::fbx(&args, &fbx_from, fbx_turn, file)?,
        _ => bail!("usage: probe find|info|names|dump|tree ..."),
    }
    Ok(())
}
