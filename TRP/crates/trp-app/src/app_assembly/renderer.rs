//! The clip player, the window's updates and the renderer's settings: backend, shader compiler, window, render thread.

use super::*;

pub(super) struct RendererSettings {
    pub(super) player: ClipPlayer,
    pub(super) updates: bevy::winit::WinitSettings,
    pub(super) plugins: bevy::app::PluginGroupBuilder,
}

/// Makes the clip player and settles the window's updates and the engine's default plugins with the renderer's settings.
pub(super) fn settings(options: &Options, options_state: &screens::options_view::OptionsState) -> RendererSettings {
    platform::run_log::phase("the hands, the level, the player's figures");
    let mut player = ClipPlayer::new("");
    player.queue = options.play.iter().cloned().collect();
    player.hold_at = options.hold_at;
    player.by_name = !options.play.is_empty() || options.hold_at.is_some();
    let present_mode = match options.present.as_str() {
        "off" => bevy::window::PresentMode::AutoNoVsync,
        "mailbox" => bevy::window::PresentMode::Mailbox,
        _ => bevy::window::PresentMode::AutoVsync,
    };
    // A window without the focus is updated only some 60 times a second, on
    // a timer. An unattended run seldom has the focus and is there to show
    // what a played one does, so it updates as a focused window would.
    let mut updates = bevy::winit::WinitSettings::game();
    if options.unattended() {
        updates.unfocused_mode = bevy::winit::UpdateMode::Continuous;
    }
    let mut renderer = bevy::render::settings::WgpuSettings::default();
    // The backend (`backend`): the engine's own variable `WGPU_BACKEND` has
    // the last word, then `--backend`, then the options; the other where
    // the one chosen finds no graphics card.
    let mut uses_dx12 = true;
    if std::env::var_os("WGPU_BACKEND").is_none() {
        let wanted = match options.backend.as_deref() {
            Some(key) => platform::backend::Renderer::from_key(key).unwrap_or_else(|| {
                eprintln!("ignoring unknown backend {key}");
                options_state.graphics.renderer
            }),
            None => options_state.graphics.renderer,
        };
        let (used, said) = platform::backend::choose(wanted, platform::backend::has_card);
        println!("{said}");
        renderer.backends = Some(platform::backend::backends(used));
        uses_dx12 = used == platform::backend::Renderer::Dx12;
    } else {
        println!("renderer: as WGPU_BACKEND says");
    }
    // DirectX 12's shaders by its newer compiler where it is found
    // (`dxc_libraries`); the engine's own choice, the variable
    // `WGPU_DX12_COMPILER`, still has the last word.
    if uses_dx12 && bevy::render::settings::Dx12Compiler::from_env().is_none() {
        match dxc_libraries() {
            Some((dxc, _)) => {
                println!("DirectX 12's shaders by DXC: {}", dxc.display());
                // (As the engine itself asks for it: shader model 6.7 at the
                // most, the dll's own signing taking the place of dxil.dll's.)
                renderer.dx12_shader_compiler =
                    bevy::render::settings::Dx12Compiler::DynamicDxc { dxc_path: dxc.display().to_string(), max_shader_model: wgpu_types::DxcShaderModel::V6_7 };
            }
            None if cfg!(windows) => println!("DirectX 12's shaders by FXC: DXC's dxcompiler.dll and dxil.dll are neither beside trp.exe nor in a Windows SDK"),
            None => {}
        }
    }
    let mut plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                // `local\window_keys_test.ps1` finds the window by it.
                title: "TRP (Tushonka Runtime Project)".into(),
                resolution: bevy::window::WindowResolution::new(options.window[0], options.window[1]),
                present_mode,
                ..default()
            }),
            ..default()
        })
        .set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(renderer),
            ..default()
        })
        // The engine's own log into the run's too (`run_log`).
        .set(bevy::log::LogPlugin { custom_layer: platform::run_log::engine_log, ..default() });
    let render_thread = options_state.render_thread_started;
    println!("render thread: {}", if render_thread { "on" } else { "off" });
    if !render_thread {
        plugins = plugins.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>();
    }

    platform::run_log::phase("the renderer's settings");
    RendererSettings { player, updates, plugins }
}
