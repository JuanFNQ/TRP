// (trp: the game's own surface light, `shading.rs`; `docs/spec/lighting.md`,
// "Tarkov's surface shading".) Put into Bevy's `pbr_functions.wgsl` before
// `apply_pbr_lighting`, which hands a fragment here in the view that asks
// for it. Every figure is the game's number, as its gamma project's
// G-buffer and light passes have them; the sum is made linear by the 2.2
// power at the end, which the frame's encoding (`gamma_ui`) undoes.

#ifndef PREPASS_FRAGMENT

// A linear colour as the stored figure it was read from (the sRGB curve).
fn trp_stored(c: vec3<f32>) -> vec3<f32> {
    let v = max(c, vec3(0.0));
    return select(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3(0.0031308));
}

// A linear light of the frame as the game's number (the 2.2 power).
fn trp_numbers(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2));
}

fn trp_pow5(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x;
}

// The engine's attenuation texture at a share of the range squared.
fn trp_attenuation(share_squared: f32) -> f32 {
    if share_squared >= 1.0 {
        return 0.0;
    }
    let near = 1.0 / (1.0 + 25.0 * max(share_squared, 0.0));
    if share_squared < 0.64 {
        return near;
    }
    return near * (1.0 - (share_squared - 0.64) / 0.36);
}

// What the surface gives the deferred passes: its colour (held to one),
// specular colour and smoothness, normal and the way to the eye.
struct TrpSurface {
    color: vec3<f32>,
    specular: vec3<f32>,
    smoothness: f32,
    N: vec3<f32>,
    V: vec3<f32>,
}

// One light (`trp_sim::shading::standard_light`): `L` the way to it,
// `light` its colour times intensity times attenuation, cookie and shadow.
fn trp_standard_light(s: TrpSurface, L: vec3<f32>, light: vec3<f32>) -> vec3<f32> {
    let sum = L + s.V;
    let H = sum * inverseSqrt(max(dot(sum, sum), 0.001));
    let nl = saturate(dot(s.N, L));
    let nh = saturate(dot(s.N, H));
    let lh = saturate(dot(L, H));
    let nv = abs(dot(s.N, s.V));
    let perceptual = 1.0 - s.smoothness;
    let a = max(perceptual * perceptual, 0.002);
    let a2 = a * a;
    let grazing = 2.0 * lh * lh * perceptual - 0.5;
    let diffuse = (1.0 + grazing * trp_pow5(1.0 - nl)) * (1.0 + grazing * trp_pow5(1.0 - nv)) * nl;
    let visibility = 0.5 / (nl * (nv * (1.0 - a) + a) + nv * (nl * (1.0 - a) + a) + 1e-5);
    let d = (nh * a2 - nh) * nh + 1.0;
    let distribution = a2 * 0.31830988 / (d * d + 1e-7);
    var specular = sqrt(max(visibility * distribution * 3.14159265, 1e-4)) * nl;
    if all(s.specular == vec3(0.0)) {
        specular = 0.0;
    }
    let fresnel = s.specular + (1.0 - s.specular) * trp_pow5(1.0 - lh);
    return s.color * light * diffuse + specular * light * fresnel;
}

fn trp_game_lighting(in: pbr_types::PbrInput) -> vec4<f32> {
    let base = in.material.base_color;
    let metallic = in.material.metallic;
    var s: TrpSurface;
    s.color = min(trp_stored(calculate_diffuse_color(base.rgb, metallic, in.material.specular_transmission, in.material.diffuse_transmission)), vec3(1.0));
    s.specular = trp_stored(calculate_F0(base.rgb, metallic, in.material.reflectance));
    s.smoothness = 1.0 - in.material.perceptual_roughness;
    s.N = in.N;
    s.V = in.V;
    let P = in.world_position.xyz;

    let view_z = dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z
    ), in.world_position);
    let cluster_index = clustering::fragment_cluster_index(in.frag_coord.xy, view_z, in.is_orthographic);
    var ranges = clustering::unpack_clusterable_object_index_ranges(cluster_index);
    let receiver = (in.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u;

    var lit = vec3(0.0);

    // Point lights: the colour times intensity Bevy holds (its power over
    // 4π), by the engine's attenuation.
    for (var i: u32 = ranges.first_point_light_index_offset; i < ranges.first_spot_light_index_offset; i = i + 1u) {
        let id = clustering::get_clusterable_object_id(i);
        let light = &view_bindings::clusterable_objects.data[id];
        let to_light = (*light).position_radius.xyz - P;
        let share = dot(to_light, to_light) * (*light).color_inverse_square_range.w;
        var shadow = 1.0;
        if receiver && ((*light).flags & mesh_view_types::POINT_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = shadows::fetch_point_shadow(id, in.world_position, in.world_normal);
        }
        lit += trp_standard_light(s, normalize(to_light), (*light).color_inverse_square_range.rgb * trp_attenuation(share) * shadow);
    }

    // Spot lights: the same, by Bevy's cone (`PROVISIONAL`: the engine's
    // default spot cookie not read).
    for (var i: u32 = ranges.first_spot_light_index_offset; i < ranges.first_reflection_probe_index_offset; i = i + 1u) {
        let id = clustering::get_clusterable_object_id(i);
        let light = &view_bindings::clusterable_objects.data[id];
        let to_light = (*light).position_radius.xyz - P;
        let L = normalize(to_light);
        let share = dot(to_light, to_light) * (*light).color_inverse_square_range.w;
        var spot_dir = vec3<f32>((*light).light_custom_data.x, 0.0, (*light).light_custom_data.y);
        spot_dir.y = sqrt(max(0.0, 1.0 - spot_dir.x * spot_dir.x - spot_dir.z * spot_dir.z));
        if ((*light).flags & mesh_view_types::POINT_LIGHT_FLAGS_SPOT_LIGHT_Y_NEGATIVE) != 0u {
            spot_dir.y = -spot_dir.y;
        }
        let cone = saturate(dot(-spot_dir, L) * (*light).light_custom_data.z + (*light).light_custom_data.w);
        var spot = cone * cone;
#ifdef LIGHT_TEXTURES
        // A spot with a cookie of its own (a tactical device's light:
        // tactical-devices spec, "The look"): the engine takes the cookie
        // in place of its default one (`UnitySpotCookie`, the picture's
        // alpha over the square the cone's angle spans), and nothing of a
        // cone besides; nothing behind the light or outside the square.
        if (*light).decal_index != 0xFFFFFFFFu {
            let decal = view_bindings::clustered_decals.decals[(*light).decal_index];
            let local = (decal.local_from_world * vec4(P, 1.0)).xyz;
            spot = 0.0;
            if local.z < 0.0 {
                let uv = (local.xy / (local.z * (*light).spot_light_tan_angle)) * vec2(-0.5, 0.5) + 0.5;
                if all(clamp(uv, vec2(0.0), vec2(1.0)) == uv) {
                    spot = textureSampleLevel(
                        view_bindings::clustered_decal_textures[decal.base_color_texture_index],
                        view_bindings::clustered_decal_sampler,
                        uv,
                        0.0
                    ).a;
                }
            }
        }
#endif
        var shadow = 1.0;
        if receiver && ((*light).flags & mesh_view_types::POINT_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = shadows::fetch_spot_shadow(id, in.world_position, in.world_normal, (*light).shadow_map_near_z);
        }
        lit += trp_standard_light(s, L, (*light).color_inverse_square_range.rgb * trp_attenuation(share) * spot * shadow);
    }

    // The sun: its colour times intensity, times its cookie and shadow.
    for (var i: u32 = 0u; i < view_bindings::lights.n_directional_lights; i = i + 1u) {
        let light = &view_bindings::lights.directional_lights[i];
        var shadow = 1.0;
        if receiver && ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = shadows::fetch_directional_shadow(i, in.world_position, in.world_normal, view_z);
        }
        var cookie = 1.0;
#ifdef LIGHT_TEXTURES
        if (*light).decal_index != 0xFFFFFFFFu {
            let decal = view_bindings::clustered_decals.decals[(*light).decal_index];
            let at = (decal.local_from_world * vec4(P, 1.0)).xyz;
            let uv = at.xy * vec2(-0.5, 0.5) + 0.5;
            if decal.tag != 0u || all(clamp(uv, vec2(0.0), vec2(1.0)) == uv) {
                cookie = textureSampleLevel(
                    view_bindings::clustered_decal_textures[decal.base_color_texture_index],
                    view_bindings::clustered_decal_sampler,
                    uv - floor(uv),
                    0.0
                ).r;
            } else {
                cookie = 0.0;
            }
        }
#endif
        lit += trp_standard_light(s, (*light).direction_to_light.xyz, (*light).color.rgb * cookie * shadow);
    }

    // The ambient after the lights (`Hidden/WriteScreenAmbient`): the
    // environment map's light at the normal (the harmonics taken to gamma,
    // or the hall's), as the game's number, times the colour.
    // (Not named `ambient`: Bevy's module imports that name.)
    var surround = vec3(0.0);
#ifdef ENVIRONMENT_MAP
    var layer: lighting::LayerLightingInput;
    layer.N = s.N;
    layer.R = reflect(-s.V, s.N);
    layer.NdotV = max(dot(s.N, s.V), 0.0001);
    layer.perceptual_roughness = in.material.perceptual_roughness;
    layer.roughness = in.material.perceptual_roughness * in.material.perceptual_roughness;
    let radiances = environment_map::compute_radiances(layer, &ranges, P, false);
    surround = s.color * trp_numbers(radiances.irradiance * view_bindings::view.exposure);
#endif
    // Its brightness term, the figure the view's ambient colour carries.
    let brightness = dot(s.color, vec3(0.2126, 0.7152, 0.0722));
    surround += brightness * brightness * view_bindings::lights.ambient_color.r;

    // The emission, Bevy's, as the game's number.
    let emissive = in.material.emissive;
    let emission = trp_numbers(emissive.rgb * base.a * mix(1.0, view_bindings::view.exposure, emissive.a));

    return vec4(pow(max(lit + surround + emission, vec3(0.0)), vec3(2.2)), base.a);
}

#endif // PREPASS_FRAGMENT

