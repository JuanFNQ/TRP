//! A particle effect read whole or by name.

use super::*;

/// A whole effect (a system and every one below it, played together:
/// `Play(withChildren: true)`), and its lights; how many copies of it its
/// mediator plays in turn (one where none).
#[derive(Debug, Clone)]
pub struct ParticleEffect {
    pub name: String,
    pub systems: Vec<EffectSystem>,
    pub lights: Vec<EffectLight>,
    pub copies: usize,
    /// The effects' list's entry lays a deferred decal (`UseDeferredDecals`,
    /// with the effects' decal renderer set); a grenade's is the renderer's
    /// grenade decal (`deferred_grenade_decal`).
    pub deferred_decal: bool,
}

/// Reads an effect from its root system, down its objects' children.
pub fn read_effect(assets: &Assets, root: ObjRef) -> Result<ParticleEffect> {
    let game_object = |obj: ObjRef| -> Option<ObjRef> { assets.read(obj).ok()?.get("m_GameObject")?.as_pptr().and_then(|p| assets.resolve_loaded(obj.file, p)) };
    let components = |go: ObjRef| -> Vec<ObjRef> {
        assets
            .read(go)
            .ok()
            .and_then(|g| g.get("m_Component")?.as_array().map(|all| all.iter().filter_map(|pair| pair.get("component")?.as_pptr()).filter_map(|p| assets.resolve_loaded(go.file, p)).collect()))
            .unwrap_or_default()
    };
    let of_class = |go: ObjRef, class_id: i32| components(go).into_iter().find(|&c| assets.info(c).is_some_and(|info| info.class_id == class_id));
    let root_go = game_object(root).context("the effect's root has no game object")?;
    let name = assets.read(root_go)?.get("m_Name").and_then(Value::as_str).unwrap_or("").to_owned();
    let mut effect = ParticleEffect { name, systems: Vec::new(), lights: Vec::new(), copies: 1, deferred_decal: false };
    // Each object with its place, turn and size from the root (the root's
    // own being where it is played).
    let mut stack = vec![(root_go, [0.0f32; 3], [0.0f32, 0.0, 0.0, 1.0], [1.0f32; 3], 0usize)];
    while let Some((go, at, turn, size, depth)) = stack.pop() {
        // An object switched off (`m_IsActive`), and all under it, is not
        // played with the effect nor lit: the small grenade's `grenade_ash`
        // (recorder 0.11.0's `courses3`: followed, never playing).
        if depth > 0 && assets.read(go)?.get("m_IsActive").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        if let Some(system) = of_class(go, class::PARTICLE_SYSTEM) {
            effect.systems.push(EffectSystem { data: read_system(assets, system)?, position: at, rotation: turn, scale: size, depth, system, sub_emitter: false });
        }
        if let Some(light) = of_class(go, class::LIGHT) {
            // A script beside it that drives it (`MuzzleParticleLight`).
            let driver = components(go).into_iter().filter(|&c| script_class(assets, c).as_deref() == Some("MuzzleParticleLight")).find_map(|c| {
                let d = assets.read(c).ok()?;
                let system = d.get("_particleSystem")?.as_pptr().and_then(|p| assets.resolve_loaded(c.file, p))?;
                let mm = d.get("_minMaxIntensity")?;
                Some(LightDriver { system, min_max: [mm.get("x")?.as_f32()?, mm.get("y")?.as_f32()?], time: d.get("_time")?.as_f32()? })
            });
            effect.lights.push(EffectLight { position: at, driver, ..read_light(assets, light)? });
        }
        let Some(transform) = of_class(go, class::TRANSFORM) else { continue };
        let t = assets.read(transform)?;
        let children: Vec<ObjRef> = t.get("m_Children").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_pptr).filter_map(|p| assets.resolve_loaded(transform.file, p)).collect();
        for child in children.into_iter().rev() {
            let c = assets.read(child)?;
            let q = c.req("m_LocalRotation")?;
            let local_turn = [float(q, "x")?, float(q, "y")?, float(q, "z")?, float(q, "w")?];
            let (local_at, local_size) = (vec3(&c, "m_LocalPosition")?, vec3(&c, "m_LocalScale")?);
            let scaled = [local_at[0] * size[0], local_at[1] * size[1], local_at[2] * size[2]];
            let moved = rotate(turn, scaled);
            let child_at = [at[0] + moved[0], at[1] + moved[1], at[2] + moved[2]];
            let child_turn = crate::unity::extract::quat_mul(turn, local_turn);
            let child_size = [size[0] * local_size[0], size[1] * local_size[1], size[2] * local_size[2]];
            if let Some(child_go) = game_object(child) {
                stack.push((child_go, child_at, child_turn, child_size, depth + 1));
            }
        }
    }
    let set_off: Vec<ObjRef> = effect.systems.iter().flat_map(|s| s.data.sub_emitters.iter().filter_map(|e| e.system)).collect();
    for s in &mut effect.systems {
        s.sub_emitter = set_off.contains(&s.system);
    }
    Ok(effect)
}

/// An effect of the effects' list by its name (`Effects.EffectsArray`, as
/// `EmitGrenade` finds it), from its mediator's first system
/// (`BasicParticleSystemMediator`: copies played in turn, for effects at
/// once; the copies are alike, by their names `(1)` and on, and their
/// systems).
pub fn named_effect(assets: &mut Assets, name: &str) -> Result<ParticleEffect> {
    assets.load("shaders")?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR))
        .find(|&obj| script_class(assets, obj).as_deref() == Some("Effects"))
        .context("the effects' bundle has no Effects")?;
    let all = assets.read(effects)?;
    let entry = all
        .req("EffectsArray")?
        .as_array()
        .unwrap_or(&[])
        .iter()
        .find(|e| e.get("Name").and_then(Value::as_str) == Some(name))
        .with_context(|| format!("no effect named {name}"))?
        .clone();
    let mediator = entry.req("BasicParticleSystemMediator")?.as_pptr().and_then(|p| assets.resolve_loaded(effects.file, p)).with_context(|| format!("{name} has no particle mediator"))?;
    let copies = assets.read(mediator)?.req("_particleSystems")?.as_array().map(<[Value]>::to_vec).unwrap_or_default();
    let first = copies.first().and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(mediator.file, p)).with_context(|| format!("{name}'s mediator has no system"))?;
    let mut effect = read_effect(assets, first)?;
    effect.copies = copies.len();
    effect.deferred_decal = entry.get("UseDeferredDecals").and_then(Value::as_i64).unwrap_or(0) != 0 && entry.get("DeferredDecals").and_then(Value::as_pptr).is_some_and(|p| p.path_id != 0);
    Ok(effect)
}
