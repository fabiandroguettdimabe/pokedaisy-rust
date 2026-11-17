//! Native window + offline web UI. Emulation remains in the SDL/libretro process.
use super::*;
use crate::library;
use crate::protocol;
use serde_json::json;
use std::ffi::c_void;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tao::{
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

enum Message {
    Action(String, ActionSource),
    Scanned(u64, Vec<Game>),
    CoreProbed(u64, serde_json::Value),
    Pipe(crate::player_pipe::PipeEvent),
    ImportProgress(u64, crate::import::ArchiveProgress),
    ImportFinished(
        u64,
        Vec<library::LibraryEntry>,
        crate::import::ImportSummary,
    ),
    VaultImportReady(
        u64,
        Result<(crate::vault::SaveBundleManifest, Vec<u8>), String>,
    ),
    VaultExportFinished(u64, Result<(), String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActionSource {
    Launcher,
    Companion(u64),
}

fn action_source_is_current(source: ActionSource, companion_generation: Option<u64>) -> bool {
    match source {
        ActionSource::Launcher => true,
        ActionSource::Companion(generation) => companion_generation == Some(generation),
    }
}

fn companion_position(
    desired: tao::dpi::PhysicalPosition<i32>,
    monitor_origin: tao::dpi::PhysicalPosition<i32>,
    monitor_size: tao::dpi::PhysicalSize<u32>,
    window_size: tao::dpi::PhysicalSize<u32>,
) -> tao::dpi::PhysicalPosition<i32> {
    let clamp_axis = |desired: i32, origin: i32, monitor_extent: u32, window_extent: u32| {
        let origin = i64::from(origin);
        let max_position = origin + i64::from(monitor_extent.saturating_sub(window_extent));
        i64::from(desired)
            .clamp(origin, max_position.max(origin))
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };

    tao::dpi::PhysicalPosition::new(
        clamp_axis(
            desired.x,
            monitor_origin.x,
            monitor_size.width,
            window_size.width,
        ),
        clamp_axis(
            desired.y,
            monitor_origin.y,
            monitor_size.height,
            window_size.height,
        ),
    )
}

fn companion_logical_size(
    monitor_size: tao::dpi::PhysicalSize<u32>,
    scale_factor: f64,
    preferred: Option<(f64, f64)>,
) -> (tao::dpi::LogicalSize<f64>, tao::dpi::LogicalSize<f64>) {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let available_width = f64::from(monitor_size.width) / scale;
    let available_height = f64::from(monitor_size.height) / scale;
    let (preferred_width, preferred_height) = preferred.unwrap_or((1120.0, 820.0));
    let width = preferred_width
        .clamp(300.0, 4096.0)
        .min(available_width.max(1.0));
    let height = preferred_height
        .clamp(240.0, 4096.0)
        .min(available_height.max(1.0));
    let min_width = 760.0_f64.min(width);
    let min_height = 600.0_f64.min(height);

    (
        tao::dpi::LogicalSize::new(width, height),
        tao::dpi::LogicalSize::new(min_width, min_height),
    )
}

fn restored_companion_position(
    bounds: &settings::CompanionWindowBounds,
    monitor_origin: tao::dpi::PhysicalPosition<i32>,
    scale_factor: f64,
) -> tao::dpi::PhysicalPosition<i32> {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let physical_offset = |value: f64| {
        (value * scale)
            .round()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32
    };
    tao::dpi::PhysicalPosition::new(
        monitor_origin.x.saturating_add(physical_offset(bounds.x)),
        monitor_origin.y.saturating_add(physical_offset(bounds.y)),
    )
}

fn companion_bounds_for_window(
    window: &tao::window::Window,
) -> Option<settings::CompanionWindowBounds> {
    let monitor = window.current_monitor()?;
    let position = window.outer_position().ok()?;
    let scale = monitor.scale_factor();
    let origin = monitor.position();
    let inner_size = window.inner_size().to_logical::<f64>(scale);
    Some(settings::CompanionWindowBounds {
        monitor_name: monitor.name(),
        x: f64::from(position.x.saturating_sub(origin.x)) / scale,
        y: f64::from(position.y.saturating_sub(origin.y)) / scale,
        width: inner_size.width,
        height: inner_size.height,
    })
}

enum ImportSource {
    Loose(PathBuf),
    Archive(PathBuf, Vec<String>),
    Rejected(usize),
}

struct PreparedBundleRestore {
    game: Game,
    manifest: crate::domain::SaveManifest,
    payload: Vec<u8>,
    destination: PathBuf,
    manifest_path: PathBuf,
}

pub fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    let root = args
        .iter()
        .position(|s| s == "--data")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or(env::current_dir().map_err(|e| e.to_string())?);
    let data = root.join("data");
    fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let config_path = data.join("vyntra-settings.json");
    let mut config = Settings::load(&config_path, &root)?;
    let mut config_fingerprint = settings_fingerprint(&config_path)?;
    for path in [
        &config.library,
        &config.saves,
        &config.states,
        &config.system,
    ] {
        fs::create_dir_all(path).map_err(|e| e.to_string())?;
    }
    let index_path = data.join("vyntra-library.json");
    let mut imported: Vec<library::LibraryEntry> = library::load(&index_path)?;
    for arg in &args {
        let p = PathBuf::from(arg);
        if supported(&p) && p.is_file() && !imported.iter().any(|e| e.path == p) {
            imported.push(library::LibraryEntry::from_path(p));
        }
    }
    let event_loop = EventLoopBuilder::<Message>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("Vyntra Desktop")
        .with_window_icon(window_icon())
        .with_inner_size(tao::dpi::LogicalSize::new(1360.0, 880.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(960.0, 640.0))
        .build(&event_loop)
        .map_err(|e| e.to_string())?;
    let proxy = event_loop.create_proxy();
    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_html(include_str!("../../../ui/dist/index.html"))
        .with_navigation_handler({
            let initial = std::cell::Cell::new(true);
            move |url| {
                if url.starts_with("data:text/html;charset=utf-8;base64,") && initial.replace(false)
                {
                    return true;
                }
                url == "about:blank" || url.starts_with("about:blank#")
            }
        })
        .with_ipc_handler(move |request| {
            if request.body().len() < 32768 {
                let _ = ipc_proxy.send_event(Message::Action(
                    request.body().clone(),
                    ActionSource::Launcher,
                ));
            }
        })
        .build(&window)
        .map_err(|e| {
            format!("No se pudo abrir WebView2: {e}. Puedes iniciar el frontend SDL con --sdl.")
        })?;
    let mut games: Vec<Game> = Vec::new();
    let mut child: Option<std::process::Child> = None;
    let mut generation = 0u64;
    let mut scanning = false;
    let mut active_scan: Option<Arc<AtomicBool>> = None;
    let mut session: Option<domain::SessionIdentity> = None;
    let mut session_epoch: u64 = 0;
    let mut player_commands: Option<crate::transport::PlayerCommandSender> = None;
    let mut player_cancellation: Option<crate::player_pipe::PipeCancellation> = None;
    let mut player_authenticated = false;
    let mut player_save_lock: Option<PlayerSaveLock> = None;
    let mut player_command_generation = 0u64;
    let mut import_generation = 0u64;
    let mut active_import: Option<Arc<AtomicBool>> = None;
    let mut vault_generation = 0u64;
    let mut vault_busy = false;
    let mut command_revision: u64 = 0;
    let mut command_acks = protocol::CommandAckCache::new(128);
    let mut pending_migration: Option<migration::MigrationReport> = None;
    let mut probe_generation = 0u64;
    let mut core_probe: Option<serde_json::Value> = None;
    let mut companion_gen3_source: Option<(PathBuf, crate::gen3_save::Gen3Format)> = None;
    let mut companion_window: Option<tao::window::Window> = None;
    let mut companion_webview: Option<wry::WebView> = None;
    let mut companion_generation: Option<u64> = None;
    let mut next_companion_generation = 0u64;
    event_loop.run(move |event,target,flow| {
        *flow=ControlFlow::Wait;
        match event {
            TaoEvent::WindowEvent { window_id, event:WindowEvent::CloseRequested,.. } => {
                if companion_window.as_ref().is_some_and(|window|window.id()==window_id) {
                    if let Some(bounds) = companion_window
                        .as_ref()
                        .and_then(companion_bounds_for_window)
                    {
                        match SettingsMutex::acquire() {
                            Ok(_lock) => {
                                let can_save = match reload_external_settings(
                                    &config_path,
                                    &root,
                                    &config_fingerprint,
                                ) {
                                    Ok(Some((latest, fingerprint))) => {
                                        config = latest;
                                        config_fingerprint = fingerprint;
                                        true
                                    }
                                    Ok(None) => true,
                                    Err(error) => {
                                        eprintln!("No se pudieron actualizar los ajustes antes de guardar Companion: {error}");
                                        false
                                    }
                                };
                                if can_save {
                                    let previous = config.companion_window.clone();
                                    config.companion_window = Some(bounds);
                                    match config.save(&config_path) {
                                        Ok(()) => {
                                            if let Ok(fingerprint) = settings_fingerprint(&config_path) {
                                                config_fingerprint = fingerprint;
                                            }
                                        }
                                        Err(error) => {
                                            config.companion_window = previous;
                                            eprintln!("No se pudieron guardar los límites de Companion: {error}");
                                        }
                                    }
                                }
                            }
                            Err(error) => eprintln!("No se pudieron guardar los límites de Companion: {error}"),
                        }
                    }
                    companion_generation=None;
                    companion_webview=None;
                    companion_window=None;
                    return;
                }
                if player_authenticated {
                    let stopped = stop_player_child(
                        &mut child,
                        || player_commands.as_ref().is_some_and(|commands| {
                            commands.send(crate::transport::PlayerCommand::Stop).is_ok()
                        }),
                        Duration::from_secs(2),
                    );
                    match stopped {
                        Ok(true) => {}
                        Ok(false) => eprintln!("Player no cerró después de Stop; se terminó el proceso."),
                        Err(error) => eprintln!("No se pudo cerrar Player al salir: {error}"),
                    }
                } else if let Err(error) = terminate_child_process(&mut child) {
                    eprintln!("No se pudo cerrar Player al salir: {error}");
                }
                if let Some(cancellation) = &player_cancellation { cancellation.cancel(); }
                *flow=ControlFlow::Exit
            },
            TaoEvent::UserEvent(Message::Scanned(id,mut found)) if id==generation => {
                active_scan = None;
                for entry in &imported { if !found.iter().any(|g| g.path==entry.path) && (!entry.path.is_file() || supported(&entry.path)) { found.push(game_from_path(entry.path.clone())); } }
                games=found; scanning=false;
                let mut catalog_changed = false;
                for entry in &mut imported {
                    if entry.path.is_file() {
                        let seen = Some(now_unix());
                        if entry.last_seen_unix != seen {
                            entry.last_seen_unix = seen;
                            catalog_changed = true;
                        }
                        let current_hash = identity::sha256_file(&entry.path).ok();
                        if entry.content_hash != current_hash {
                            entry.refresh();
                            catalog_changed = true;
                        }
                    }
                }
                if catalog_changed {
                    let _ = library::save(&index_path, &imported);
                }
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
            },
            TaoEvent::UserEvent(Message::CoreProbed(id, report)) if id == probe_generation => {
                core_probe = Some(report);
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
            },
            TaoEvent::UserEvent(Message::ImportProgress(id, progress)) if id == import_generation => {
                if let Ok(payload) = serde_json::to_string(&json!({
                    "phase": progress.phase,
                    "member": progress.member,
                    "completed_bytes": progress.completed_bytes,
                    "total_bytes": progress.total_bytes,
                    "batch_item": progress.batch_item,
                    "batch_total": progress.batch_total,
                })) {
                    let _ = webview.evaluate_script(&format!("window.vyntraImportProgress&&window.vyntraImportProgress({payload});"));
                }
            },
            TaoEvent::UserEvent(Message::ImportFinished(id, new_entries, summary)) if id == import_generation => {
                imported = new_entries;
                active_import = None;
                if summary.imported > 0 {
                    let _ = library::save(&index_path, &imported);
                    for entry in &imported {
                        if entry.path.is_file() && supported(&entry.path) && !games.iter().any(|game| game.path == entry.path) {
                            games.push(game_from_path(entry.path.clone()));
                        }
                    }
                }
                let done = json!({"imported":summary.imported,"duplicates":summary.duplicates,"rejected":summary.rejected,"cancelled":summary.cancelled});
                if let Ok(payload) = serde_json::to_string(&done) {
                    let _ = webview.evaluate_script(&format!("window.vyntraImportDone&&window.vyntraImportDone({payload});"));
                }
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
            },
            TaoEvent::UserEvent(Message::VaultImportReady(id, result)) if id == vault_generation => {
                vault_busy = false;
                match result.and_then(|(manifest, payload)| prepare_bundle_restore(manifest, payload, &games, &config)) {
                    Err(error) => {
                        let _ = webview.evaluate_script(&format!("window.vyntraError({});", json!(error)));
                    }
                    Ok(prepared) => {
                        let kind = if prepared.manifest.kind == domain::SaveKind::SaveState { "estado" } else { "SRAM" };
                        let prompt = format!("Importar {kind} verificado para {}. Si ya existe un guardado válido, se conservará una copia antes de reemplazarlo.", prepared.game.title);
                        let answer = rfd::MessageDialog::new()
                            .set_title("Importar paquete de Vyntra Vault")
                            .set_description(&prompt)
                            .set_level(rfd::MessageLevel::Warning)
                            .set_buttons(rfd::MessageButtons::YesNo)
                            .show();
                        if answer == rfd::MessageDialogResult::Yes {
                            match commit_bundle_restore(prepared, &config) {
                                Ok(()) => {
                                    publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
                                    let _ = webview.evaluate_script("window.vyntraAck&&window.vyntraAck({ok:true,result:{vault_import:{restored:true}}});");
                                }
                                Err(error) => { let _ = webview.evaluate_script(&format!("window.vyntraError({});", json!(error))); }
                            }
                        } else {
                            let _ = webview.evaluate_script("window.vyntraAck&&window.vyntraAck({ok:true,result:{vault_import:{cancelled:true}}});");
                        }
                    }
                }
            },
            TaoEvent::UserEvent(Message::VaultExportFinished(id, result)) if id == vault_generation => {
                vault_busy = false;
                match result {
                    Ok(()) => { let _ = webview.evaluate_script("window.vyntraAck&&window.vyntraAck({ok:true,result:{vault_export:{done:true}}});"); }
                    Err(error) => { let _ = webview.evaluate_script(&format!("window.vyntraError({});", json!(error))); }
                }
            },
            TaoEvent::UserEvent(Message::Pipe(event)) => {
                let matches_session = session.as_ref().is_some_and(|active| {
                    active.session_id == event.session_id && active.epoch == event.epoch
                });
                if matches_session {
                    match event.kind {
                        crate::player_pipe::PipeEventKind::Authenticated => {
                            player_authenticated = true;
                        },
                        crate::player_pipe::PipeEventKind::Cancelled => {
                            player_commands = None;
                            player_cancellation = None;
                            player_authenticated = false;
                            session = None;
                            session_epoch = session_epoch.saturating_add(1);
                            publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
                        },
                        crate::player_pipe::PipeEventKind::Snapshot(snapshot) => {
                            if let Ok(value) = serde_json::to_string(&snapshot) {
                                evaluate_both(&webview,companion_webview.as_ref(),&format!("window.vyntraPlayerSnapshot({value});"));
                            }
                        },
                        crate::player_pipe::PipeEventKind::CommandResult { request_id, command, result } => {
                            let response = match result {
                                Ok(()) => json!({"ok":true,"result":{"player_command":{"id":request_id,"command":command,"ok":true}}}),
                                Err(error) => json!({"ok":true,"result":{"player_command":{"id":request_id,"command":command,"ok":false,"error":error}}}),
                            };
                            if let Ok(value) = serde_json::to_string(&response) {
                                evaluate_both(&webview,companion_webview.as_ref(),&format!("window.vyntraAck&&window.vyntraAck({value});"));
                            }
                        },
                        crate::player_pipe::PipeEventKind::Disconnected => {
                            let exit_status = child
                                .as_mut()
                                .and_then(|process| process.try_wait().ok().flatten());
                            if exit_status.is_some() {
                                child = None;
                            }
                            let notice = match exit_status {
                                Some(status) => player_exit_notice(status.success(), status.code()),
                                None if child.is_some() => Some(
                                    "Se perdió el canal local; Player intentará reconectar automáticamente.".into(),
                                ),
                                None => None,
                            };
                            if let Some(notice) = notice {
                                let _ = webview.evaluate_script(&format!(
                                    "window.vyntraError&&window.vyntraError({});",
                                    json!(notice)
                                ));
                            }
                            player_authenticated = false;
                            if exit_status.is_some() {
                                player_save_lock = None;
                                if let Some(cancellation) = &player_cancellation {
                                    cancellation.cancel();
                                }
                                player_commands = None;
                                player_cancellation = None;
                                session = None;
                                session_epoch = session_epoch.saturating_add(1);
                            }
                            publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
                        },
                        crate::player_pipe::PipeEventKind::Failed(reason) => {
                            player_commands = None;
                            player_cancellation = None;
                            player_authenticated = false;
                            let notice = if child.is_some() {
                                format!("Falló el canal local de Player: {reason}. Usa Detener si la ventana sigue abierta.")
                            } else {
                                format!("Falló el canal local de Player: {reason}.")
                            };
                            eprintln!("{notice}");
                            let _ = webview.evaluate_script(&format!(
                                "window.vyntraError&&window.vyntraError({});",
                                json!(notice)
                            ));
                            session = None;
                            session_epoch = session_epoch.saturating_add(1);
                            publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
                        },
                    }
                }
            },
            TaoEvent::UserEvent(Message::Action(body, source)) => {
                if !action_source_is_current(source, companion_generation) {
                    return;
                }
                let cmd = match protocol::parse_ui_command(&body) {
                    Ok(cmd) => cmd,
                    Err(error) => {
                        let ack = protocol::UiAck::failure("invalid-envelope", error.clone());
                        evaluate_from(&webview,companion_webview.as_ref(),source,&format!(
                            "window.vyntraAck&&window.vyntraAck({});",
                            json!(ack)
                        ));
                        evaluate_from(&webview,companion_webview.as_ref(),source,&format!(
                            "window.vyntraError&&window.vyntraError({});",
                            json!(error.reason)
                        ));
                        return;
                    }
                };
                let _settings_lock = if is_settings_mutation(&cmd.action) {
                    match SettingsMutex::acquire() {
                        Ok(lock) => Some(lock),
                        Err(reason) => {
                            let ack = protocol::UiAck::failure(cmd.command_id.clone(), protocol::AppError::new("settings_busy", reason).retryable());
                            evaluate_from(&webview,companion_webview.as_ref(),source,&format!("window.vyntraAck&&window.vyntraAck({});", json!(ack)));
                            return;
                        }
                    }
                } else { None };
                if let Some(cached) = command_acks.lookup(&cmd).cloned() {
                    let mut replay = cached;
                    replay.revision = Some(command_revision);
                    evaluate_from(&webview,companion_webview.as_ref(),source,&format!("window.vyntraAck&&window.vyntraAck({});", json!(replay)));
                    return;
                }
                let action = cmd.payload.clone();
                let name = cmd.action.as_str();
                let mut refresh = false;
                let mut command_result: Option<serde_json::Value> = None;
                let mut external_config_error = None;
                if is_settings_mutation(name) {
                    match reload_external_settings(&config_path, &root, &config_fingerprint) {
                        Ok(Some((latest, current))) => { config = latest; config_fingerprint = current; command_revision = command_revision.saturating_add(1); refresh = true; }
                        Ok(None) => {},
                        Err(error) => external_config_error = Some(error),
                    }
                }
                let mut stale_revision = cmd
                    .expected_revision
                    .is_some_and(|revision| revision != command_revision);
                let result = (|| -> Result<(), String> {
                    if let Some(error) = external_config_error { return Err(error); }
                    validate_command_revision(cmd.expected_revision, command_revision)?;
                    validate_command_session(&session, session_epoch, &cmd)?;
                    match name {
                        "open_companion_window" => {
                            if companion_window.is_none() {
                                next_companion_generation = next_companion_generation
                                    .checked_add(1)
                                    .ok_or("No se pudo asignar una nueva sesión a Companion")?;
                                let generation = next_companion_generation;
                                let mut builder=WindowBuilder::new().with_title("Vyntra Companion").with_inner_size(tao::dpi::LogicalSize::new(1120.0,820.0)).with_min_inner_size(tao::dpi::LogicalSize::new(760.0,600.0));
                                let saved_bounds = config.companion_window.as_ref();
                                let monitor = saved_bounds
                                    .and_then(|bounds| {
                                        bounds.monitor_name.as_ref().and_then(|name| {
                                            window.available_monitors().find(|monitor| {
                                                monitor.name().as_ref() == Some(name)
                                            })
                                        })
                                    })
                                    .or_else(|| window.current_monitor());
                                if let Some(monitor) = monitor {
                                    let scale = monitor.scale_factor();
                                    let (inner_size, min_inner_size) =
                                        companion_logical_size(
                                            monitor.size(),
                                            scale,
                                            saved_bounds.map(|bounds| (bounds.width, bounds.height)),
                                        );
                                    builder = builder
                                        .with_inner_size(inner_size)
                                        .with_min_inner_size(min_inner_size);
                                    let requested_size = tao::dpi::PhysicalSize::new(
                                        (inner_size.width * scale).round() as u32,
                                        (inner_size.height * scale).round() as u32,
                                    );
                                    let desired = saved_bounds
                                        .map(|bounds| {
                                            restored_companion_position(
                                                bounds,
                                                monitor.position(),
                                                scale,
                                            )
                                        })
                                        .or_else(|| {
                                            window.outer_position().ok().map(|position| {
                                                tao::dpi::PhysicalPosition::new(
                                                    position.x.saturating_add(80),
                                                    position.y.saturating_add(60),
                                                )
                                            })
                                        })
                                        .unwrap_or(monitor.position());
                                    builder = builder.with_position(companion_position(
                                        desired,
                                        monitor.position(),
                                        monitor.size(),
                                        requested_size,
                                    ));
                                }
                                let new_window=builder.build(target).map_err(|error|format!("No se pudo abrir Companion: {error}"))?;
                                let companion_proxy=proxy.clone();
                                let new_webview=WebViewBuilder::new().with_html(include_str!("../../../ui/dist/index.html")).with_initialization_script("window.vyntraCompanionOnly=true;").with_ipc_handler(move |request|{
                                    if request.body().len()<32768{let _=companion_proxy.send_event(Message::Action(request.body().clone(),ActionSource::Companion(generation)));}
                                }).build(&new_window).map_err(|error|format!("No se pudo iniciar Companion: {error}"))?;
                                companion_generation=Some(generation);
                                companion_window=Some(new_window);
                                companion_webview=Some(new_webview);
                            }
                        },
                        "ready" | "scan" => refresh=true,
                        "reset_settings_section" => {
                            let section = action["section"].as_str().ok_or("Sección inválida")?;
                            let previous = config.clone();
                            config.reset_section(section, &root)?;
                            if section == "folders" {
                                let directories = [config.library.clone(), config.saves.clone(), config.states.clone(), config.system.clone()];
                                for path in directories {
                                    if let Err(error) = fs::create_dir_all(path) {
                                        config = previous;
                                        return Err(error.to_string());
                                    }
                                }
                            }
                            if let Err(error) = config.save(&config_path) {
                                config = previous;
                                return Err(error);
                            }
                            if section == "folders" {
                                refresh = true;
                            }
                        },
                        "favorite" => {
                            let id = action["storage_id"].as_str().ok_or("Juego inválido")?;
                            let favorite = action["value"].as_bool().ok_or("Favorito inválido")?;
                            if !library::set_favorite(&mut imported, id, favorite) { return Err("El juego ya no está en la biblioteca".into()); }
                            library::save(&index_path, &imported)?;
                        },
                        "game_note" => {
                            let id = action["storage_id"].as_str().ok_or("Juego inválido")?;
                            let note = action["value"].as_str().ok_or("Nota inválida")?;
                            if note.len() > 2000 || note.chars().any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t') { return Err("La nota debe tener hasta 2000 bytes y texto válido".into()); }
                            if !library::set_note(&mut imported, id, note.to_owned()) { return Err("El juego ya no está en la biblioteca".into()); }
                            library::save(&index_path, &imported)?;
                        },
                        "game_smoothing" => {
                            let id = action["storage_id"].as_str().ok_or("Juego inválido")?;
                            if !games.iter().any(|game| game_storage_key(game) == id) {
                                return Err("El juego ya no está en la biblioteca".into());
                            }
                            let value = action["value"].as_bool();
                            let previous = config.clone();
                            config.set_game_smooth(id, value)?;
                            if let Err(error) = config.save(&config_path) {
                                config = previous;
                                return Err(error);
                            }
                        },
                        "system_smoothing" => {
                            let system = action["system"].as_str().ok_or("Sistema inválido")?;
                            let value = action["value"].as_bool();
                            let previous = config.clone();
                            config.set_system_smooth(system, value)?;
                            if let Err(error) = config.save(&config_path) {
                                config = previous;
                                return Err(error);
                            }
                        },
                        "game_volume" => {
                            let id = action["storage_id"].as_str().ok_or("Juego inválido")?;
                            if !games.iter().any(|game| game_storage_key(game) == id) { return Err("El juego ya no está en la biblioteca".into()); }
                            let value = action["value"].as_u64().map(|value| value as u8);
                            let previous = config.clone();
                            config.set_game_volume(id, value)?;
                            if let Err(error) = config.save(&config_path) { config = previous; return Err(error); }
                        },
                        "system_volume" => {
                            let system = action["system"].as_str().ok_or("Sistema inválido")?;
                            let value = action["value"].as_u64().map(|value| value as u8);
                            let previous = config.clone();
                            config.set_system_volume(system, value)?;
                            if let Err(error) = config.save(&config_path) { config = previous; return Err(error); }
                        },
                        "game_fast_forward" => {
                            let id = action["storage_id"].as_str().ok_or("Juego inválido")?;
                            if !games.iter().any(|game| game_storage_key(game) == id) { return Err("El juego ya no está en la biblioteca".into()); }
                            let value = action["value"].as_u64().map(|value| value as u32);
                            let previous = config.clone();
                            config.set_game_fast_forward(id, value)?;
                            if let Err(error) = config.save(&config_path) { config = previous; return Err(error); }
                        },
                        "system_fast_forward" => {
                            let system = action["system"].as_str().ok_or("Sistema inválido")?;
                            let value = action["value"].as_u64().map(|value| value as u32);
                            let previous = config.clone();
                            config.set_system_fast_forward(system, value)?;
                            if let Err(error) = config.save(&config_path) { config = previous; return Err(error); }
                        },
                        "game_autosave" => {
                            let id = action["storage_id"].as_str().ok_or("Juego inválido")?;
                            if !games.iter().any(|game| game_storage_key(game) == id) { return Err("El juego ya no está en la biblioteca".into()); }
                            let value = action["value"].as_u64();
                            let previous = config.clone();
                            config.set_game_autosave(id, value)?;
                            if let Err(error) = config.save(&config_path) { config = previous; return Err(error); }
                        },
                        "system_autosave" => {
                            let system = action["system"].as_str().ok_or("Sistema inválido")?;
                            let value = action["value"].as_u64();
                            let previous = config.clone();
                            config.set_system_autosave(system, value)?;
                            if let Err(error) = config.save(&config_path) { config = previous; return Err(error); }
                        },
                        "library_view" => {
                            let view = action["value"].as_str().ok_or("Vista inválida")?;
                            if !matches!(view, "grid" | "list") { return Err("Vista inválida".into()); }
                            let mut next = config.clone();
                            next.library_view = view.into();
                            persist_settings(&mut config, next, &config_path)?;
                        },
                        "library_filter" => {
                            let filter = action["value"].as_str().ok_or("Filtro inválido")?;
                            if !matches!(filter, "Todos" | "Favoritos" | "GB" | "GBC" | "GBA" | "DS") { return Err("Filtro inválido".into()); }
                            let mut next = config.clone();
                            next.library_filter = filter.into();
                            persist_settings(&mut config, next, &config_path)?;
                        },
                        "library_selection" => {
                            let selection = action["value"].as_str().ok_or("Selección inválida")?;
                            if selection.len() > 64 || !selection.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)) { return Err("Selección inválida".into()); }
                            let mut next = config.clone();
                            next.library_selection = Some(selection.into());
                            persist_settings(&mut config, next, &config_path)?;
                        },
                        "relink" => {
                            let id = action["storage_id"].as_str().ok_or("Juego inválido")?;
                            if let Some(path) = rfd::FileDialog::new().set_title("Volver a localizar esta ROM · debe ser idéntica").add_filter("ROM GB / GBC / GBA / DS", &["gb","gbc","gba","nds"]).pick_file() {
                                library::relink_entry(&mut imported, id, &path)?;
                                library::save(&index_path, &imported)?;
                                command_result = Some(json!({"relinked":true}));
                                refresh = true;
                            }
                        },
                        "type_matchup" => {
                            let attack = action["attack"].as_u64().filter(|id| *id <= u8::MAX as u64).ok_or("Tipo atacante inválido")? as u8;
                            let defense_values = action["defenses"].as_array().ok_or("Tipos defensores inválidos")?;
                            let defenses = defense_values.iter().map(|value| value.as_u64().filter(|id| *id <= u8::MAX as u64).map(|id| id as u8).ok_or_else(|| "Tipo defensor inválido".to_owned())).collect::<Result<Vec<_>, _>>()?;
                            let result = crate::reference::resolve_matchup(&crate::reference::gen3_type_chart()?, attack, &defenses)?;
                            command_result = Some(serde_json::to_value(result).map_err(|error| error.to_string())?);
                        },
                        "import" => {
                            if active_import.is_some() { return Err("Ya hay una importación en curso".into()); }
                            if let Some(paths)=rfd::FileDialog::new().set_title("Importar ROMs o ZIP a Vyntra").add_filter("ROMs y ZIP", &["gb","gbc","gba","nds","zip"]).pick_files() {
                                let mut sources = Vec::new();
                                for path in paths {
                                    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) {
                                        let policy = crate::import::ArchivePolicy::default();
                                        match crate::import::archive_rom_members(&path, policy) {
                                            Ok(candidates) => {
                                                let mut selected = Vec::new();
                                                let mut cancelled = false;
                                                for member in candidates.iter() {
                                                    if candidates.len() == 1 {
                                                        selected.push(member.clone());
                                                    } else {
                                                        match rfd::MessageDialog::new()
                                                            .set_parent(&window)
                                                            .set_title("Seleccionar ROM del ZIP")
                                                            .set_description(format!("¿Importar {}?", member))
                                                            .set_buttons(rfd::MessageButtons::YesNoCancel)
                                                            .show()
                                                        {
                                                            rfd::MessageDialogResult::Yes => selected.push(member.clone()),
                                                            rfd::MessageDialogResult::Cancel => { cancelled = true; break; },
                                                            _ => {},
                                                        }
                                                    }
                                                }
                                                if cancelled { break; }
                                                if !selected.is_empty() { sources.push(ImportSource::Archive(path, selected)); }
                                            }
                                            Err(_) => sources.push(ImportSource::Rejected(1)),
                                        }
                                    } else {
                                        sources.push(ImportSource::Loose(path));
                                    }
                                }
                                if !sources.is_empty() {
                                    import_generation = import_generation.saturating_add(1);
                                    let id = import_generation;
                                    let cancelled = Arc::new(AtomicBool::new(false));
                                    let cancel_worker = Arc::clone(&cancelled);
                                    let mut worker_entries = imported.clone();
                                    let worker_data = data.join("import-cache");
                                    let worker_proxy = proxy.clone();
                                    let spawn = std::thread::Builder::new().name("vyntra-import".into()).spawn(move || {
                                        let mut summary = crate::import::ImportSummary::default();
                                        let batch_total = sources.len();
                                        for (batch_index, source) in sources.into_iter().enumerate() {
                                            if cancel_worker.load(Ordering::Acquire) { summary.cancelled = true; break; }
                                            match source {
                                                ImportSource::Loose(path) => {
                                                    let proxy = worker_proxy.clone();
                                                    let result = crate::import::import_paths_controlled(
                                                        &mut worker_entries, [path], 1024 * 1024 * 1024,
                                                        &cancel_worker, move |mut progress| { progress.batch_item = batch_index + 1; progress.batch_total = batch_total; let _ = proxy.send_event(Message::ImportProgress(id, progress)); },
                                                    );
                                                    summary.imported += result.imported;
                                                    summary.duplicates += result.duplicates;
                                                    summary.rejected += result.rejected;
                                                    if result.cancelled { summary.cancelled = true; break; }
                                                }
                                                ImportSource::Archive(path, selected) => {
                                                    let proxy = worker_proxy.clone();
                                                    let result = crate::import::import_archive_members_controlled(
                                                        &mut worker_entries, &path, &worker_data,
                                                        crate::import::ArchivePolicy::default(), &selected,
                                                        &cancel_worker, move |mut progress| { progress.batch_item = batch_index + 1; progress.batch_total = batch_total; let _ = proxy.send_event(Message::ImportProgress(id, progress)); },
                                                    );
                                                    summary.imported += result.imported;
                                                    summary.duplicates += result.duplicates;
                                                    summary.rejected += result.rejected;
                                                    if result.cancelled { summary.cancelled = true; break; }
                                                }
                                                ImportSource::Rejected(count) => summary.rejected += count,
                                            }
                                        }
                                        let _ = worker_proxy.send_event(Message::ImportFinished(id, worker_entries, summary));
                                    });
                                    match spawn {
                                        Ok(_) => {
                                            active_import = Some(cancelled);
                                            command_result = Some(json!({"import":{"started":true}}));
                                        }
                                        Err(error) => return Err(format!("No se pudo iniciar la importación: {error}")),
                                    }
                                }
                            }
                        },
                        "import_roots" => {
                            if active_import.is_some() { return Err("Ya hay una importación en curso".into()); }
                            if let Some(mut roots) = rfd::FileDialog::new().set_title("Elegir hasta 16 carpetas de ROMs").pick_folders() {
                                let rejected_roots = roots.len().saturating_sub(16);
                                roots.truncate(16);
                                if !roots.is_empty() {
                                    import_generation = import_generation.saturating_add(1);
                                    let id = import_generation;
                                    let cancelled = Arc::new(AtomicBool::new(false));
                                    let cancel_worker = Arc::clone(&cancelled);
                                    let mut worker_entries = imported.clone();
                                    let worker_proxy = proxy.clone();
                                    let spawn = std::thread::Builder::new().name("vyntra-import-roots".into()).spawn(move || {
                                        let mut summary = crate::import::ImportSummary::default();
                                        summary.rejected = rejected_roots;
                                        let scan_proxy = worker_proxy.clone();
                                        let (paths, rejected_paths) = crate::import::collect_rom_paths_controlled(
                                            &roots, 10_000, &cancel_worker, move |visited| {
                                                let _ = scan_proxy.send_event(Message::ImportProgress(id, crate::import::ArchiveProgress {
                                                    phase: "scan".into(), completed_bytes: visited as u64, total_bytes: 100_000,
                                                    member: "Revisando carpetas".into(), batch_item: 0, batch_total: 0,
                                                }));
                                            },
                                        );
                                        summary.rejected += rejected_paths;
                                        if cancel_worker.load(Ordering::Acquire) { summary.cancelled = true; }
                                        let sources: Vec<_> = paths.into_iter().map(ImportSource::Loose).collect();
                                        let batch_total = sources.len();
                                        for (batch_index, source) in sources.into_iter().enumerate() {
                                            if cancel_worker.load(Ordering::Acquire) { summary.cancelled = true; break; }
                                            match source {
                                                ImportSource::Loose(path) => {
                                                    let proxy = worker_proxy.clone();
                                                    let result = crate::import::import_paths_controlled(
                                                        &mut worker_entries, [path], 1024 * 1024 * 1024,
                                                        &cancel_worker, move |mut progress| { progress.batch_item = batch_index + 1; progress.batch_total = batch_total; let _ = proxy.send_event(Message::ImportProgress(id, progress)); },
                                                    );
                                                    summary.imported += result.imported;
                                                    summary.duplicates += result.duplicates;
                                                    summary.rejected += result.rejected;
                                                    if result.cancelled { summary.cancelled = true; break; }
                                                }
                                                ImportSource::Rejected(count) => summary.rejected += count,
                                                ImportSource::Archive(_, _) => unreachable!(),
                                            }
                                        }
                                        let _ = worker_proxy.send_event(Message::ImportFinished(id, worker_entries, summary));
                                    });
                                    match spawn {
                                        Ok(_) => {
                                            active_import = Some(cancelled);
                                            command_result = Some(json!({"import":{"started":true}}));
                                        }
                                        Err(error) => return Err(format!("No se pudo iniciar la importación: {error}")),
                                    }
                                }
                            }
                        },
                        "cancel_import" => {
                            let cancellation = active_import.as_ref().ok_or("No hay una importación activa")?;
                            cancellation.store(true, Ordering::Release);
                        },
                        "read_gen3_save" | "refresh_gen3_save" => {
                            let format=match action["format"].as_str().unwrap_or_default(){
                                "ruby_sapphire"=>crate::gen3_save::Gen3Format::RubySapphire,
                                "emerald"=>crate::gen3_save::Gen3Format::Emerald,
                                "firered_leafgreen"=>crate::gen3_save::Gen3Format::FireRedLeafGreen,
                                _=>return Err("Perfil de guardado Gen 3 no compatible".into()),
                            };
                            let path=if name=="read_gen3_save" {
                                let Some(path)=rfd::FileDialog::new().set_title("Abrir guardado GBA · lectura solamente").add_filter("Guardados GBA", &["sav","srm","dat"]).pick_file() else { return Ok(()); };
                                path
                            } else {
                                let (path,cached_format)=companion_gen3_source.as_ref().ok_or("Todavía no hay un guardado para actualizar")?;
                                if *cached_format!=format{return Err("El juego seleccionado cambió; vuelve a elegir el guardado".into());}
                                path.clone()
                            };
                            let metadata=fs::metadata(&path).map_err(|error|format!("No se pudo leer el archivo: {error}"))?;
                            if metadata.len()>1024*1024{return Err("El guardado supera el límite de lectura de 1 MiB".into());}
                            let bytes=fs::read(&path).map_err(|error|format!("No se pudo leer el guardado: {error}"))?;
                            let snapshot=crate::gen3_save::read(&bytes,format)?;
                            companion_gen3_source=Some((path.clone(),format));
                            command_result=Some(json!({"gen3_save":snapshot,"file_name":path.file_name().unwrap_or_default().to_string_lossy()}));
                        },                        "choose" => {
                            let field=action["field"].as_str().unwrap_or("");
                            let mut next=config.clone();
                            if let Some(i)=field.strip_prefix("core").and_then(|v|v.parse::<usize>().ok()).filter(|i|*i<3) {
                                if let Some(p)=rfd::FileDialog::new().set_title("Elegir núcleo libretro de confianza").add_filter("Núcleo Windows", &["dll"]).pick_file(){next.cores[i]=Some(p);}else{return Ok(());}
                            } else if ["library","saves","states","system"].contains(&field) {
                                if let Some(p)=rfd::FileDialog::new().set_title("Seleccionar carpeta · no mueve archivos anteriores").pick_folder() {
                                    *match field {"library"=>&mut next.library,"saves"=>&mut next.saves,"states"=>&mut next.states,_=>&mut next.system}=p;
                                } else{return Ok(());}
                            } else { return Err("Opción no válida".into()); }
                            let _settings_lock = SettingsMutex::acquire()?;
                            let current = settings_fingerprint(&config_path)?;
                            if current != config_fingerprint {
                                config = Settings::load(&config_path, &root)?;
                                config_fingerprint = current;
                                command_revision = command_revision.saturating_add(1);
                                stale_revision = true;
                                refresh = true;
                                return Err("La configuración cambió en otra ventana mientras elegías la ruta. Revisa el estado y vuelve a elegirla.".into());
                            }
                            next.save(&config_path)?; config=next; config_fingerprint=settings_fingerprint(&config_path)?; refresh=true;
                        },
                        "set" => {
                            let mut next=config.clone(); let field=action["field"].as_str().unwrap_or(""); let value=&action["value"];
                            match field {
                                "volume"=>next.volume=value.as_u64().ok_or("Volumen inválido")?.min(100) as u8,
                                "smooth"=>next.smooth=value.as_bool().ok_or("Filtro inválido")?,
                                "fullscreen"=>next.fullscreen=value.as_bool().ok_or("Valor inválido")?,
                                "recursive"=>next.recursive=value.as_bool().ok_or("Valor inválido")?,
                                "fast_forward"=>next.fast_forward=value.as_u64().ok_or("Velocidad inválida")?.clamp(2,4) as u32,
                                "autosave"=>next.autosave=value.as_u64().ok_or("Intervalo inválido")?.clamp(10,120),
                                "reduced_motion"=>next.reduced_motion=value.as_bool().ok_or("Valor inválido")?,
                                "keys_reset"=>next.keys=Settings::default().keys,
                                "cores_reset"=>next.cores=[None,None,None],
                                "key"=> {
                                    let i=action["index"].as_u64().filter(|i|*i<14).ok_or("Botón inválido")? as usize;
                                    let key=Keycode::from_name(value.as_str().ok_or("Tecla inválida")?).ok_or("Tecla no reconocida")?;
                                    if [Keycode::Escape,Keycode::F2,Keycode::F4,Keycode::F11,Keycode::Space].contains(&key) || next.keys.iter().enumerate().any(|(j,k)|j!=i && Keycode::from_name(k)==Some(key)){return Err("Tecla reservada o ya asignada".into());}
                                    next.keys[i]=key.name();
                                },
                                _=>return Err("Preferencia no válida".into()),
                            }
                            next.save(&config_path)?; config=next;
                            if field=="recursive"{refresh=true;}
                        },
                        "play"=> {
                            ensure_player_stopped(&mut child, &mut player_save_lock, "Ya hay una partida abierta. Vuelve a su ventana para continuar.")?;
                            session = None;
                            let game = if let Some(storage_id) = action["storage_id"].as_str() {
                                let source_index = action["source_index"].as_u64().ok_or("Selecciona una copia del juego")? as usize;
                                let source = crate::resolve_game_by_source(&games, storage_id, source_index).ok_or("La copia seleccionada cambió desde que se mostró la biblioteca; actualízala")?;
                                source
                            } else {
                                games.get(action["id"].as_u64().ok_or("Selecciona un juego")? as usize).ok_or("Juego no encontrado")?
                            };
                            if !game.path.is_file(){return Err("La ROM ya no está en su ubicación. Impórtala desde su carpeta actual.".into());}
                            crate::validate_game_identity(game)?;
                            let core=config.core(game.system)?;
                            if !core.is_file(){return Err("La DLL seleccionada no existe. Revisa Ajustes > Núcleos.".into());}
                            let game_id = game_storage_key(game);
                            fs::create_dir_all(&config.saves).map_err(|error| format!("No se pudo preparar la carpeta de guardados: {error}"))?;
                            fs::create_dir_all(&config.states).map_err(|error| format!("No se pudo preparar la carpeta de estados: {error}"))?;
                            let save_lock = PlayerSaveLock::acquire(&config.saves, &config.states, &game_id)?;
                            if library::mark_played(&mut imported, &game_id, now_unix()) {
                                library::save(&index_path, &imported)?;
                            }
                            let player_executable = env::current_exe().map_err(|e| e.to_string())?;
                            let next_epoch = session_epoch.saturating_add(1);
                            let session_id = format!("sess-{}-{next_epoch}", std::process::id());
                            let pipe_proxy = proxy.clone();
                            let pipe = crate::player_pipe::start(session_id.clone(), next_epoch, move |event| {
                                let _ = pipe_proxy.send_event(Message::Pipe(event));
                            })?;
                            player_commands = Some(pipe.commands.clone());
                            let cancellation = pipe.cancellation.clone();
                            let spawn_result = std::process::Command::new(player_executable)
                                .arg("--emulator").arg("--data").arg(&root).arg(&game.path)
                                .arg("--pipe").arg(&pipe.name).arg("--nonce").arg(&pipe.nonce)
                                .arg("--session-id").arg(&session_id).arg("--session-epoch").arg(next_epoch.to_string())
                                .spawn();
                            match spawn_result {
                                Ok(process) => {
                                    child = Some(process);
                                    player_save_lock = Some(save_lock);
                                }
                                Err(error) => {
                                    cancellation.cancel();
                                    player_commands = None;
                                    return Err(error.to_string());
                                }
                            }
                            player_cancellation = Some(cancellation);
                            player_authenticated = false;
                            session_epoch = next_epoch;
                            session = Some(domain::SessionIdentity {
                                session_id,
                                game_id: game_storage_key(game),
                                rom_sha256: identity::sha256_file(&game.path).ok(),
                                core_hash: identity::sha256_file(&core).ok(),
                                adapter_id: None,
                                epoch: session_epoch,
                            });
                        },
                        "stop_player" => {
                            if !player_authenticated {
                                if let Some(cancellation) = &player_cancellation { cancellation.cancel(); }
                                terminate_child_process(&mut child)?;
                                player_commands = None;
                                player_cancellation = None;
                                player_authenticated = false;
                                player_save_lock = None;
                                session = None;
                                session_epoch = session_epoch.saturating_add(1);
                                return Ok(());
                            }
                            let sent = player_commands.as_ref().is_some_and(|commands| {
                                commands
                                    .send(crate::transport::PlayerCommand::Stop)
                                    .is_ok()
                            });
                            if !sent {
                                terminate_child_process(&mut child)?;
                                player_commands = None;
                                player_cancellation = None;
                                player_authenticated = false;
                                player_save_lock = None;
                                session = None;
                                session_epoch = session_epoch.saturating_add(1);
                            }
                        },
                        "pause_player" => {
                            let paused = action["value"].as_bool().ok_or("Estado de pausa inválido")?;
                            player_commands.as_ref().ok_or("No hay una partida conectada")?
                                .send(crate::transport::PlayerCommand::Pause(paused))
                                .map_err(|error| format!("No se pudo cambiar la pausa de Player: {error:?}"))?;
                        },
                        "save_player_state" | "load_player_state" => {
                            player_command_generation = player_command_generation.saturating_add(1);
                            if player_command_generation == 0 { player_command_generation = 1; }
                            let command = if name == "save_player_state" {
                                crate::transport::PlayerCommand::SaveState(player_command_generation)
                            } else {
                                crate::transport::PlayerCommand::LoadState(player_command_generation)
                            };
                            player_commands.as_ref().ok_or("No hay una partida conectada")?
                                .send(command)
                                .map_err(|error| format!("No se pudo enviar la operación a Player: {error:?}"))?;
                            command_result = Some(json!({"player_command":{"started":true,"id":player_command_generation,"command":name}}));
                        },
                        "open_folder"=> {
                            let field=action["field"].as_str().unwrap_or("");
                            let path=match field {"saves"=>&config.saves,"states"=>&config.states,"library"=>&config.library,"system"=>&config.system,"config"=>&data,_=>return Err("Carpeta no válida".into())};
                            std::process::Command::new("explorer.exe").arg(path).spawn().map_err(|e|e.to_string())?;
                        },
                        "export_vault_bundle" => {
                            if vault_busy { return Err("Ya hay una operación Vault en curso".into()); }
                            ensure_player_stopped(&mut child, &mut player_save_lock, "Cierra la partida antes de empaquetar un guardado.")?;
                            let requested = PathBuf::from(action["file"].as_str().ok_or("Guardado inválido")?);
                            let source = save_files(&config).into_iter().find(|path| *path == requested)
                                .ok_or("El guardado ya no está disponible; actualiza la lista")?;
                            let default_name = format!("{}.vyntra.zip", source.file_stem().unwrap_or_default().to_string_lossy());
                            if let Some(destination) = rfd::FileDialog::new()
                                .set_title("Exportar paquete verificado de Vault")
                                .add_filter("Paquete Vyntra Vault", &["zip"])
                                .set_file_name(default_name)
                                .save_file()
                            {
                                if destination == source { return Err("El paquete debe guardarse en otra ubicación".into()); }
                                vault_generation = vault_generation.saturating_add(1);
                                let id = vault_generation;
                                let tx = proxy.clone();
                                let worker_config = config.clone();
                                vault_busy = true;
                                let spawn = std::thread::Builder::new().name("vyntra-vault-export".into()).spawn(move || {
                                    let result = bundle_for_save_file(&source, &worker_config)
                                        .and_then(|bytes| atomic_write(&destination, &bytes));
                                    let _ = tx.send_event(Message::VaultExportFinished(id, result));
                                });
                                if let Err(error) = spawn {
                                    vault_busy = false;
                                    return Err(format!("No se pudo iniciar la exportación Vault: {error}"));
                                }
                                command_result = Some(json!({"vault_export":{"started":true}}));
                            }
                        },
                        "import_vault_bundle" => {
                            if vault_busy { return Err("Ya hay una operación Vault en curso".into()); }
                            ensure_player_stopped(&mut child, &mut player_save_lock, "Cierra la partida antes de importar un guardado.")?;
                            if let Some(bundle_path) = rfd::FileDialog::new()
                                .set_title("Elegir paquete verificado de Vyntra Vault")
                                .add_filter("Paquete Vault", &["zip"])
                                .pick_file()
                            {
                                let size = fs::metadata(&bundle_path).map_err(|error| error.to_string())?.len();
                                if size > crate::vault::MAX_BUNDLE_BYTES { return Err("El paquete Vault supera el límite de tamaño".into()); }
                                vault_generation = vault_generation.saturating_add(1);
                                let id = vault_generation;
                                let tx = proxy.clone();
                                vault_busy = true;
                                let spawn = std::thread::Builder::new().name("vyntra-vault-import".into()).spawn(move || {
                                    let result = crate::vault::read_bundle_file(&bundle_path);
                                    let _ = tx.send_event(Message::VaultImportReady(id, result));
                                });
                                if let Err(error) = spawn {
                                    vault_busy = false;
                                    return Err(format!("No se pudo iniciar la importación Vault: {error}"));
                                }
                                command_result = Some(json!({"vault_import":{"started":true}}));
                            }
                        },
                        "export_save"=> {
                            ensure_player_stopped(&mut child, &mut player_save_lock, "Cierra la partida antes de exportar para evitar copiar un guardado durante su escritura.")?;
                            let files=save_files(&config);
                            let requested=PathBuf::from(action["file"].as_str().ok_or("Guardado inválido")?);
                            let source=files.iter().find(|path|**path==requested).ok_or("El guardado ya no está disponible; actualiza la lista")?;
                            if let Some(dest)=rfd::FileDialog::new().set_title("Exportar una copia del guardado").set_file_name(source.file_name().unwrap_or_default().to_string_lossy()).save_file(){
                                if source==&dest {return Err("Elige una ubicación distinta al original".into());}
                                fs::copy(source,dest).map_err(|e|e.to_string())?;
                            }
                        },
                        "migration_preview" => {
                            let report =
                                migration::preview(&legacy_migration_items(&games, &config, &root))?;
                            command_result = Some(json!({"migration": report}));
                            pending_migration = Some(report);
                        },
                        "migration_apply" => {
                            let report = pending_migration.clone().ok_or("Primero revisa los guardados heredados")?;
                            validate_migration_report_paths(&report, &config, &root)?;
                            migration::apply(&data.join("migration-journal-v1.json"), &report)?;
                            let refreshed = migration::preview(&report.items.into_iter().map(|entry| entry.item).collect::<Vec<_>>())?;
                            command_result = Some(json!({"migration": refreshed}));
                            pending_migration = Some(refreshed);
                            refresh = true;
                        },
                        "probe_core" => {
                            let index = action["index"].as_u64().filter(|i| *i < 3).ok_or("Núcleo inválido")? as usize;
                            let system = ["GBC", "GBA", "DS"][index];
                            let expected_system = system.to_string();
                            let core_path = config.core(system)?;
                            if !core_path.is_file() { return Err("El archivo del núcleo no existe".into()); }
                            probe_generation = probe_generation.saturating_add(1);
                            let id = probe_generation;
                            core_probe = Some(json!({"path":core_path,"status":"probing","reason":null}));
                            let exe = env::current_exe().map_err(|e|e.to_string())?;
                            let tx = proxy.clone();
                            std::thread::spawn(move || {
                                let report = crate::probe::probe_child(&exe,&core_path,&expected_system,std::time::Duration::from_secs(8))
                                    .unwrap_or_else(|reason| json!({"path":core_path,"status":"process_failed","reason":reason}));
                                let _ = tx.send_event(Message::CoreProbed(id,report));
                            });
                        },
                        _=>return Err("Acción no disponible".into()),
                    } Ok(())
                })();
                if result.is_ok() && is_settings_mutation(name) {
                    if let Ok(fingerprint) = settings_fingerprint(&config_path) { config_fingerprint = fingerprint; }
                }
                command_revision = command_revision.saturating_add(1);
                let ack = match &result {
                    Ok(_) => {
                        let mut ack = protocol::UiAck::success(cmd.command_id.clone(), command_result.take());
                        ack.request_id = cmd.request_id.clone();
                        ack.revision = Some(command_revision);
                        ack
                    }
                    Err(error) => {
                        let app_error = if stale_revision {
                            protocol::AppError::new("stale_revision", error.clone()).retryable()
                        } else {
                            protocol::AppError::new("command_failed", error.clone())
                        };
                        let mut ack = protocol::UiAck::failure(cmd.command_id.clone(), app_error);
                        ack.request_id = cmd.request_id.clone();
                        ack.revision = Some(command_revision);
                        ack
                    }
                };
                command_acks.remember(&cmd, ack.clone());
                let target_webview=if matches!(source, ActionSource::Companion(_)) { companion_webview.as_ref().unwrap_or(&webview) } else { &webview };
                let _ = target_webview.evaluate_script(&format!("window.vyntraAck&&window.vyntraAck({});", json!(ack)));
                if let Err(error)=result {
                    if !stale_revision {
                        let _=target_webview.evaluate_script(&format!("window.vyntraError({});",json!(error)));
                    }
                }
                if refresh {
                    if let Some(previous) = active_scan.take() { previous.store(true, Ordering::Release); }
                    generation = generation.saturating_add(1);
                    scanning = true;
                    let id = generation;
                    let cancelled = Arc::new(AtomicBool::new(false));
                    let worker_cancelled = Arc::clone(&cancelled);
                    active_scan = Some(cancelled);
                    let cfg = config.clone();
                    let tx = proxy.clone();
                    std::thread::spawn(move || {
                        let found = scan_games_controlled(&cfg.library, cfg.recursive, &worker_cancelled);
                        let _ = tx.send_event(Message::Scanned(id, found));
                    });
                }
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe, companion_webview.as_ref());
            },
            _=>{}
        }
    });
}

fn window_icon() -> Option<tao::window::Icon> {
    let polygon = [
        (3.0, 9.0),
        (20.0, 9.0),
        (32.0, 34.0),
        (44.0, 9.0),
        (61.0, 9.0),
        (32.0, 57.0),
    ];
    let mut pixels = vec![0u8; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let mut inside = false;
            for i in 0..polygon.len() {
                let (ax, ay) = polygon[i];
                let (bx, by) = polygon[(i + 1) % polygon.len()];
                if (ay > y as f32) != (by > y as f32)
                    && (x as f32) < (bx - ax) * (y as f32 - ay) / (by - ay) + ax
                {
                    inside = !inside;
                }
            }
            if inside {
                let at = (y * 64 + x) * 4;
                pixels[at..at + 4].copy_from_slice(&[
                    (245 - y * 2) as u8,
                    (85 - y) as u8,
                    (112 - y) as u8,
                    255,
                ]);
            }
        }
    }
    tao::window::Icon::from_rgba(pixels, 64, 64).ok()
}

fn player_exit_notice(success: bool, code: Option<i32>) -> Option<String> {
    if success {
        None
    } else {
        Some(match code {
            Some(code) => format!("Player terminó inesperadamente (código {code})."),
            None => "Player terminó inesperadamente (sin código de salida).".into(),
        })
    }
}

fn terminate_child_process(child: &mut Option<std::process::Child>) -> Result<(), String> {
    let Some(mut process) = child.take() else {
        return Ok(());
    };
    match process.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            if let Err(kill_error) = process.kill() {
                if process.try_wait().ok().flatten().is_some() {
                    return Ok(());
                }
                *child = Some(process);
                return Err(kill_error.to_string());
            }
            match process.wait() {
                Ok(_) => Ok(()),
                Err(error) => {
                    *child = Some(process);
                    Err(error.to_string())
                }
            }
        }
        Err(error) => {
            *child = Some(process);
            Err(error.to_string())
        }
    }
}

fn stop_player_child(
    child: &mut Option<std::process::Child>,
    request_stop: impl FnOnce() -> bool,
    timeout: Duration,
) -> Result<bool, String> {
    let Some(process) = child.as_mut() else {
        return Ok(true);
    };
    if process
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        *child = None;
        return Ok(true);
    }
    if !request_stop() {
        terminate_child_process(child)?;
        return Ok(false);
    }
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let Some(process) = child.as_mut() else {
            return Ok(true);
        };
        if process
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            *child = None;
            return Ok(true);
        }
        if std::time::Instant::now() >= deadline {
            terminate_child_process(child)?;
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn ensure_player_stopped(
    child: &mut Option<std::process::Child>,
    save_lock: &mut Option<PlayerSaveLock>,
    message: &str,
) -> Result<(), String> {
    let Some(process) = child.as_mut() else {
        *save_lock = None;
        return Ok(());
    };
    if process
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Err(message.to_owned());
    }
    *child = None;
    *save_lock = None;
    Ok(())
}

fn validate_command_session(
    session: &Option<domain::SessionIdentity>,
    session_epoch: u64,
    cmd: &protocol::UiCommandEnvelope,
) -> Result<(), String> {
    if cmd.action == "stop_player" && session.is_none() {
        return Ok(());
    }
    if let Some(expected_session) = cmd.session_id.as_deref() {
        if session.as_ref().map(|s| s.session_id.as_str()) != Some(expected_session) {
            return Err("La sesión ya no está activa. Vuelve a abrir el juego.".into());
        }
    }
    if let Some(epoch) = cmd.epoch {
        if epoch != session_epoch {
            return Err("Comando de una sesión anterior. Actualiza la vista.".into());
        }
    }
    Ok(())
}

fn is_settings_mutation(action: &str) -> bool {
    matches!(
        action,
        "reset_settings_section"
            | "game_smoothing"
            | "system_smoothing"
            | "game_volume"
            | "system_volume"
            | "game_fast_forward"
            | "system_fast_forward"
            | "game_autosave"
            | "system_autosave"
            | "library_view"
            | "library_filter"
            | "library_selection"
            | "set"
    )
}

fn settings_fingerprint(path: &Path) -> Result<Option<String>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(crate::identity::sha256_bytes(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("No se pudo revisar la configuración: {error}")),
    }
}

fn persist_settings(current: &mut Settings, next: Settings, path: &Path) -> Result<(), String> {
    next.save(path)?;
    *current = next;
    Ok(())
}

fn reload_external_settings(
    path: &Path,
    root: &Path,
    known: &Option<String>,
) -> Result<Option<(Settings, Option<String>)>, String> {
    let current = settings_fingerprint(path)?;
    if &current == known {
        return Ok(None);
    }
    let latest = Settings::load(path, root)
        .map_err(|error| format!("No se pudo recargar la configuración actualizada: {error}"))?;
    Ok(Some((latest, current)))
}

struct SettingsMutex(*mut c_void);

struct PlayerSaveLock(Vec<*mut c_void>);

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *mut c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    fn ReleaseMutex(handle: *mut c_void) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

impl SettingsMutex {
    fn acquire() -> Result<Self, String> {
        let name: Vec<u16> = "Local\\VyntraSettingsMutation-v1"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err("No se pudo abrir el bloqueo compartido de ajustes".into());
        }
        let wait = unsafe { WaitForSingleObject(handle, 30_000) };
        if wait != 0 && wait != 0x80 {
            unsafe {
                CloseHandle(handle);
            }
            return Err("Otra ventana mantiene ocupados los ajustes. Intenta de nuevo.".into());
        }
        Ok(Self(handle))
    }
}

impl Drop for SettingsMutex {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

impl PlayerSaveLock {
    fn acquire(saves: &Path, states: &Path, game_id: &str) -> Result<Self, String> {
        let saves = fs::canonicalize(saves)
            .map_err(|error| format!("No se pudo resolver la carpeta de guardados: {error}"))?;
        let states = fs::canonicalize(states)
            .map_err(|error| format!("No se pudo resolver la carpeta de estados: {error}"))?;
        let mut names = [
            format!(
                "Local\\VyntraPlayerSave-v1-{}",
                crate::identity::sha256_bytes(
                    format!(
                        "battery|{}|{game_id}",
                        saves.to_string_lossy().to_lowercase()
                    )
                    .as_bytes()
                )
            ),
            format!(
                "Local\\VyntraPlayerSave-v1-{}",
                crate::identity::sha256_bytes(
                    format!(
                        "state|{}|{game_id}",
                        states.to_string_lossy().to_lowercase()
                    )
                    .as_bytes()
                )
            ),
        ];
        names.sort();
        let mut handles = Vec::with_capacity(names.len());
        for name in names {
            let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
            if handle.is_null() {
                Self::release_handles(&mut handles);
                return Err("No se pudo abrir el bloqueo de la partida".into());
            }
            match unsafe { WaitForSingleObject(handle, 0) } {
                0 | 0x80 => handles.push(handle),
                0x102 => {
                    unsafe { CloseHandle(handle) };
                    Self::release_handles(&mut handles);
                    return Err("Este juego ya está abierto en otra instancia de Vyntra".into());
                }
                _ => {
                    unsafe { CloseHandle(handle) };
                    Self::release_handles(&mut handles);
                    return Err("No se pudo adquirir el bloqueo de los guardados".into());
                }
            }
        }
        Ok(Self(handles))
    }

    fn release_handles(handles: &mut Vec<*mut c_void>) {
        for handle in handles.drain(..).rev() {
            unsafe {
                ReleaseMutex(handle);
                CloseHandle(handle);
            }
        }
    }
}

impl Drop for PlayerSaveLock {
    fn drop(&mut self) {
        Self::release_handles(&mut self.0);
    }
}

fn validate_command_revision(expected: Option<u64>, current: u64) -> Result<(), String> {
    if expected.is_some_and(|revision| revision != current) {
        return Err("La biblioteca cambió desde tu última acción; vuelve a intentarlo".into());
    }
    Ok(())
}

fn read_bounded_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > max_bytes {
        return Err("El archivo supera el tamaño máximo permitido".into());
    }
    Ok(bytes)
}

fn bundle_for_save_file(source: &Path, config: &Settings) -> Result<Vec<u8>, String> {
    let is_state = source.starts_with(&config.states);
    if !is_state && !source.starts_with(&config.saves) {
        return Err("El guardado está fuera de las carpetas de Vault".into());
    }
    let storage_id = source
        .ancestors()
        .find(|ancestor| {
            ancestor
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "by-id")
        })
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or("El guardado no pertenece a una carpeta de identidad verificable")?
        .to_owned();
    let parent = source.parent().ok_or("Ruta de guardado inválida")?;
    let manifest_path = parent.join(if is_state {
        "state.manifest.json"
    } else {
        "manifest.json"
    });
    let manifest: domain::SaveManifest = serde_json::from_slice(
        &read_bounded_file(&manifest_path, 64 * 1024)
            .map_err(|_| "El guardado no tiene manifiesto verificable")?,
    )
    .map_err(|_| "El manifiesto del guardado está dañado")?;
    let payload = read_bounded_file(source, crate::vault::MAX_BUNDLE_PAYLOAD_BYTES)
        .map_err(|_| "El guardado supera el tamaño máximo para un paquete Vault")?;
    let rom_hash = manifest
        .rom_sha256
        .as_deref()
        .ok_or("Falta identidad SHA-256 de ROM")?;
    let core_hash = manifest
        .core_hash
        .as_deref()
        .ok_or("Falta identidad SHA-256 del núcleo")?;
    if !storage_id.eq_ignore_ascii_case(rom_hash.get(..16).ok_or("Hash de ROM inválido")?)
        || manifest.save_id != format!("{}-{:?}", storage_id, manifest.kind).to_lowercase()
    {
        return Err("La carpeta o el manifiesto no corresponden a la identidad de esta ROM".into());
    }
    if is_state {
        if manifest.kind != domain::SaveKind::SaveState {
            return Err("El manifiesto no corresponde a un estado".into());
        }
        vault::validate_state(&manifest, &payload, rom_hash, core_hash)?;
    } else {
        if manifest.kind != domain::SaveKind::BatteryRam {
            return Err("El manifiesto no corresponde a una SRAM".into());
        }
        vault::validate_battery(&manifest, &payload, rom_hash, core_hash)?;
    }
    let bundle_manifest = vault::SaveBundleManifest {
        schema_version: 1,
        storage_id,
        kind: manifest.kind,
        rom_sha256: rom_hash.to_owned(),
        core_hash: core_hash.to_owned(),
        state_format: manifest.state_format,
        bytes: payload.len() as u64,
        sha256: identity::sha256_bytes(&payload),
    };
    vault::encode_bundle(&bundle_manifest, &payload)
}

fn prepare_bundle_restore(
    bundle: vault::SaveBundleManifest,
    payload: Vec<u8>,
    games: &[Game],
    config: &Settings,
) -> Result<PreparedBundleRestore, String> {
    let game = games
        .iter()
        .find(|game| game_storage_key(game) == bundle.storage_id)
        .ok_or("No hay una ROM de esta biblioteca con la identidad del paquete")?;
    let rom_hash = identity::sha256_file(&game.path).map_err(|error| error.to_string())?;
    if rom_hash != bundle.rom_sha256 {
        return Err("El contenido de la ROM no coincide con el paquete Vault".into());
    }
    let core_path = config
        .core(game.system)
        .map_err(|_| "No hay un núcleo instalado para verificar el paquete")?;
    let core_hash = identity::sha256_file(&core_path)
        .map_err(|_| "No se pudo verificar el núcleo instalado")?;
    if core_hash != bundle.core_hash {
        return Err("El paquete se creó con otra versión del núcleo".into());
    }
    let manifest = domain::SaveManifest {
        save_id: format!("{}-{:?}", bundle.storage_id, bundle.kind).to_lowercase(),
        kind: bundle.kind,
        rom_sha256: Some(bundle.rom_sha256.clone()),
        core_hash: Some(bundle.core_hash.clone()),
        state_format: bundle.state_format.clone(),
        slot: (bundle.kind == domain::SaveKind::SaveState).then_some(1),
        bytes: Some(bundle.bytes),
        sha256: Some(bundle.sha256.clone()),
        parent_id: None,
        created_at_utc: None,
        created_at_unix: Some(now_unix()),
        status: "committed".into(),
    };
    match bundle.kind {
        domain::SaveKind::SaveState => {
            vault::validate_state(&manifest, &payload, &rom_hash, &core_hash)?
        }
        domain::SaveKind::BatteryRam => {
            vault::validate_battery(&manifest, &payload, &rom_hash, &core_hash)?
        }
        _ => return Err("El paquete no contiene un guardado restaurable".into()),
    }
    let (destination, manifest_file) = match bundle.kind {
        domain::SaveKind::SaveState => (
            preferred_state_path(game, &config.states),
            "state.manifest.json",
        ),
        domain::SaveKind::BatteryRam => {
            (preferred_battery_path(game, &config.saves), "manifest.json")
        }
        _ => unreachable!(),
    };
    let manifest_path = destination
        .parent()
        .ok_or("Destino de restauración inválido")?
        .join(manifest_file);
    Ok(PreparedBundleRestore {
        game: game.clone(),
        manifest,
        payload,
        destination,
        manifest_path,
    })
}

fn commit_bundle_restore(prepared: PreparedBundleRestore, config: &Settings) -> Result<(), String> {
    commit_bundle_restore_with(prepared, config, atomic_write)
}

fn commit_bundle_restore_with(
    prepared: PreparedBundleRestore,
    config: &Settings,
    mut write: impl FnMut(&Path, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    validate_game_identity(&prepared.game)?;
    let is_state = prepared.manifest.kind == domain::SaveKind::SaveState;
    let backup = if is_state {
        backup_verified_state(
            &prepared.game,
            &config.states,
            prepared.manifest.core_hash.as_deref(),
        )?
    } else {
        backup_verified_battery(
            &prepared.game,
            &config.saves,
            prepared.manifest.core_hash.as_deref(),
        )?
    };
    if prepared.destination.is_file() && backup.is_none() {
        return Err("El guardado actual no se pudo verificar y queda intacto; muévelo manualmente antes de importar".into());
    }
    write(&prepared.destination, &prepared.payload)?;
    let manifest_bytes =
        serde_json::to_vec_pretty(&prepared.manifest).map_err(|error| error.to_string())?;
    if let Err(error) = write(&prepared.manifest_path, &manifest_bytes) {
        if let Some(backup_dir) = backup {
            let (payload_name, manifest_name) = if is_state {
                ("slot1.ss", "state.manifest.json")
            } else {
                ("battery.sav", "manifest.json")
            };
            let old_payload = fs::read(backup_dir.join(payload_name)).map_err(|restore_error| format!("No se pudo confirmar manifiesto ({error}); la copia previa permanece en {} ({restore_error})", backup_dir.display()))?;
            let old_manifest = fs::read(backup_dir.join(manifest_name)).map_err(|restore_error| format!("No se pudo confirmar manifiesto ({error}); la copia previa permanece en {} ({restore_error})", backup_dir.display()))?;
            write(&prepared.destination, &old_payload).map_err(|restore_error| format!("Falló el manifiesto ({error}) y la restauración ({restore_error}); copia previa: {}", backup_dir.display()))?;
            write(&prepared.manifest_path, &old_manifest).map_err(|restore_error| format!("Falló el manifiesto ({error}) y la restauración ({restore_error}); copia previa: {}", backup_dir.display()))?;
        } else {
            let _ = fs::remove_file(&prepared.destination);
        }
        return Err(format!(
            "No se pudo confirmar el manifiesto del guardado: {error}"
        ));
    }
    Ok(())
}

fn save_files(config: &Settings) -> Vec<PathBuf> {
    let mut files = save_files_in(&[&config.saves, &config.states]);
    files.sort_by_key(|p| {
        std::cmp::Reverse(
            p.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH),
        )
    });
    files
}

fn legacy_migration_roots(config: &Settings, root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut battery_dirs = vec![
        config.saves.clone(),
        root.join("data/saves"),
        root.join("saves"),
    ];
    battery_dirs.sort();
    battery_dirs.dedup();
    let mut state_dirs = vec![
        config.states.clone(),
        root.join("data/states"),
        root.join("states"),
    ];
    state_dirs.sort();
    state_dirs.dedup();
    (battery_dirs, state_dirs)
}

fn validate_migration_report_paths(
    report: &migration::MigrationReport,
    config: &Settings,
    root: &Path,
) -> Result<(), String> {
    let (battery_roots, state_roots) = legacy_migration_roots(config, root);
    for entry in &report.items {
        let (source_roots, destination_root) = match entry.item.kind.as_str() {
            "battery" => (&battery_roots, &config.saves),
            "state_unverified" => (&state_roots, &config.states),
            _ => return Err("Tipo de guardado de migración desconocido".into()),
        };
        let source_allowed = source_roots
            .iter()
            .any(|source_root| regular_file_within_root(&entry.item.source, source_root));
        if !source_allowed {
            return Err("El origen queda fuera de las carpetas conocidas de guardados".into());
        }
        if !destination_within_root(&entry.item.destination, destination_root) {
            return Err("El destino queda fuera de la carpeta configurada".into());
        }
    }
    Ok(())
}

fn regular_file_within_root(path: &Path, root: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }
    let (Ok(path), Ok(root)) = (fs::canonicalize(path), fs::canonicalize(root)) else {
        return false;
    };
    path.starts_with(root)
}

fn destination_within_root(destination: &Path, root: &Path) -> bool {
    if !destination.starts_with(root) {
        return false;
    }
    let Ok(canonical_root) = fs::canonicalize(root) else {
        return false;
    };
    if let Ok(metadata) = fs::symlink_metadata(destination) {
        if !metadata.file_type().is_file() {
            return false;
        }
        return fs::canonicalize(destination).is_ok_and(|path| path.starts_with(&canonical_root));
    }

    let Some(mut ancestor) = destination.parent() else {
        return false;
    };
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                return fs::canonicalize(ancestor)
                    .is_ok_and(|path| path.starts_with(&canonical_root));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(parent) = ancestor.parent() else {
                    return false;
                };
                ancestor = parent;
            }
            Err(_) => return false,
        }
    }
}

fn legacy_migration_items(
    games: &[Game],
    config: &Settings,
    root: &Path,
) -> Vec<migration::MigrationItem> {
    let (battery_dirs, state_dirs) = legacy_migration_roots(config, root);
    let mut items = Vec::new();
    for game in games {
        let game_id = game_storage_key(game);
        let battery_destination = preferred_battery_path(game, &config.saves);
        for dir in &battery_dirs {
            let battery_source = legacy_battery_path(game, dir);
            if regular_file_within_root(&battery_source, dir)
                && battery_source != battery_destination
            {
                items.push(migration::MigrationItem::new(
                    battery_source,
                    battery_destination.clone(),
                    game_id.clone(),
                    "battery".into(),
                ));
            }
        }
        let state_destination = preferred_state_path(game, &config.states);
        for dir in &state_dirs {
            let state_source = legacy_state_path(game, dir);
            if regular_file_within_root(&state_source, dir) && state_source != state_destination {
                items.push(migration::MigrationItem::new(
                    state_source,
                    state_destination.clone(),
                    game_id.clone(),
                    "state_unverified".into(),
                ));
            }
        }
    }
    items
}

fn save_files_in(roots: &[&Path]) -> Vec<PathBuf> {
    fn walk(root: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_symlink() {
                continue;
            }
            if p.is_dir() {
                walk(&p, out);
            } else if p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| ["sav", "srm", "ss", "state"].contains(&s))
            {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for root in roots {
        walk(root, &mut files);
    }
    files
}
fn game_identity_for_ui(entry: Option<&library::LibraryEntry>) -> serde_json::Value {
    json!({
        "game_code": entry.and_then(|value| value.game_code.as_deref()),
        "revision": entry.and_then(|value| value.revision),
        "confidence": entry.and_then(|value| value.confidence),
    })
}

fn evaluate_both(primary: &wry::WebView, companion: Option<&wry::WebView>, script: &str) {
    let _ = primary.evaluate_script(script);
    if let Some(companion) = companion {
        let _ = companion.evaluate_script(script);
    }
}

fn evaluate_from(
    primary: &wry::WebView,
    companion: Option<&wry::WebView>,
    source: ActionSource,
    script: &str,
) {
    let target = if matches!(source, ActionSource::Companion(_)) {
        companion.unwrap_or(primary)
    } else {
        primary
    };
    let _ = target.evaluate_script(script);
}

fn publish(
    webview: &wry::WebView,
    config: &Settings,
    games: &[Game],
    imported: &[library::LibraryEntry],
    scanning: bool,
    session: &Option<domain::SessionIdentity>,
    session_epoch: u64,
    command_revision: u64,
    core_probe: &Option<serde_json::Value>,
    secondary: Option<&wry::WebView>,
) {
    let games:Vec<_>=games.iter().enumerate().map(|(id,g)|{let storage_id=game_storage_key(g);let metadata=imported.iter().find(|entry|entry.id==storage_id);json!({"id":id,"storage_id":storage_id,"title":g.title,"system":g.system,"path":g.path,"missing":!g.path.is_file(),"core":config.core(g.system).is_ok_and(|p|p.is_file()),"favorite":metadata.is_some_and(|entry|entry.favorite),"last_played_unix":metadata.and_then(|entry|entry.last_played_unix),"note":metadata.and_then(|entry|entry.note.clone()).unwrap_or_default(),"identity":game_identity_for_ui(metadata)})}).collect();
    let cores:Vec<_>=["GBC","GBA","DS"].iter().enumerate().map(|(id,s)| {let path=config.core(s).ok();json!({"id":id,"system":s,"path":path,"installed":path.as_ref().is_some_and(|p|p.is_file())})}).collect();
    let saves:Vec<_>=save_files(config).iter().enumerate().map(|(id,p)|{let meta=p.metadata().ok();json!({"id":id,"path":p,"name":p.file_name().unwrap_or_default().to_string_lossy(),"bytes":meta.as_ref().map(|m|m.len()),"date":meta.and_then(|m|m.modified().ok()).and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_secs()),"state":p.starts_with(&config.states)})}).collect();
    let value = json!({
        "config": config,
        "games": games,
        "cores": cores,
        "saves": saves,
        "scanning": scanning,
        "session": session,
        "session_epoch": session_epoch,
        "command_revision": command_revision,
        "core_probe": core_probe,
        "reference_type_chart": crate::reference::gen3_type_chart().ok()
    });
    let script = format!("window.vyntraState({value});");
    let _ = webview.evaluate_script(&script);
    if let Some(secondary) = secondary {
        let _ = secondary.evaluate_script(&script);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_position_stays_inside_monitor_with_negative_origin() {
        let monitor_origin = tao::dpi::PhysicalPosition::new(-1920, 0);
        let monitor_size = tao::dpi::PhysicalSize::new(1920, 1080);
        let companion_size = tao::dpi::PhysicalSize::new(1120, 820);

        assert_eq!(
            companion_position(
                tao::dpi::PhysicalPosition::new(-1850, 80),
                monitor_origin,
                monitor_size,
                companion_size,
            ),
            tao::dpi::PhysicalPosition::new(-1850, 80)
        );
        assert_eq!(
            companion_position(
                tao::dpi::PhysicalPosition::new(-20, 900),
                monitor_origin,
                monitor_size,
                companion_size,
            ),
            tao::dpi::PhysicalPosition::new(-1120, 260)
        );
    }

    #[test]
    fn companion_size_fits_small_high_dpi_monitor() {
        let (size, minimum) =
            companion_logical_size(tao::dpi::PhysicalSize::new(900, 600), 1.5, None);

        assert_eq!(size, tao::dpi::LogicalSize::new(600.0, 400.0));
        assert_eq!(minimum, tao::dpi::LogicalSize::new(600.0, 400.0));
    }

    #[test]
    fn restored_companion_bounds_follow_monitor_origin_and_scale() {
        let bounds = settings::CompanionWindowBounds {
            monitor_name: Some("DISPLAY2".into()),
            x: 80.0,
            y: 40.0,
            width: 960.0,
            height: 720.0,
        };
        assert_eq!(
            restored_companion_position(&bounds, tao::dpi::PhysicalPosition::new(-1920, 0), 1.5,),
            tao::dpi::PhysicalPosition::new(-1800, 60)
        );
    }

    #[test]
    fn queued_actions_from_a_closed_companion_generation_are_rejected() {
        assert!(action_source_is_current(ActionSource::Launcher, None));
        assert!(action_source_is_current(
            ActionSource::Companion(7),
            Some(7)
        ));
        assert!(!action_source_is_current(
            ActionSource::Companion(6),
            Some(7)
        ));
        assert!(!action_source_is_current(ActionSource::Companion(7), None));
    }

    #[test]
    fn active_player_blocks_sensitive_operations_without_losing_child_handle() {
        const HOLD_ENV: &str = "VYNTRA_TEST_HOLD_PLAYER_CHILD";
        const READY_ENV: &str = "VYNTRA_TEST_PLAYER_CHILD_READY";
        let ready_path =
            std::env::temp_dir().join(format!("vyntra-player-guard-ready-{}", std::process::id()));
        let _ = fs::remove_file(&ready_path);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shell::tests::hold_player_child_until_killed"])
            .env(HOLD_ENV, "1")
            .env(READY_ENV, &ready_path)
            .spawn()
            .unwrap();
        for _ in 0..200 {
            if ready_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(ready_path.exists(), "child process did not start");
        let mut child = Some(child);
        let mut save_lock = None;

        assert!(ensure_player_stopped(&mut child, &mut save_lock, "player activo").is_err());
        assert!(child.is_some(), "guard must preserve the live child handle");

        terminate_child_process(&mut child).unwrap();
        let _ = fs::remove_file(ready_path);
    }

    #[test]
    fn exited_player_is_reaped_before_sensitive_operations() {
        let root =
            std::env::temp_dir().join(format!("vyntra-exited-player-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let saves = root.join("saves");
        let states = root.join("states");
        fs::create_dir_all(&saves).unwrap();
        fs::create_dir_all(&states).unwrap();
        let mut child = Some(
            std::process::Command::new("cmd.exe")
                .args(["/C", "exit", "0"])
                .spawn()
                .unwrap(),
        );
        child.as_mut().unwrap().wait().unwrap();
        let mut save_lock = Some(PlayerSaveLock::acquire(&saves, &states, "exited-game").unwrap());

        assert!(ensure_player_stopped(&mut child, &mut save_lock, "player activo").is_ok());
        assert!(child.is_none());
        assert!(save_lock.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn authenticated_player_exits_gracefully_after_stop_request() {
        const READY_ENV: &str = "VYNTRA_TEST_STOP_PLAYER_READY";
        const STOP_ENV: &str = "VYNTRA_TEST_STOP_PLAYER_FILE";
        let ready_path =
            std::env::temp_dir().join(format!("vyntra-stop-ready-{}", std::process::id()));
        let stop_path =
            std::env::temp_dir().join(format!("vyntra-stop-signal-{}", std::process::id()));
        let _ = fs::remove_file(&ready_path);
        let _ = fs::remove_file(&stop_path);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shell::tests::stop_aware_player_child"])
            .env(READY_ENV, &ready_path)
            .env(STOP_ENV, &stop_path)
            .spawn()
            .unwrap();
        for _ in 0..200 {
            if ready_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(ready_path.exists(), "child Player fixture did not start");
        let mut child = Some(child);
        let graceful = stop_player_child(
            &mut child,
            || {
                fs::write(&stop_path, "stop").unwrap();
                true
            },
            std::time::Duration::from_secs(2),
        )
        .unwrap();
        assert!(graceful);
        assert!(child.is_none());
        let _ = fs::remove_file(ready_path);
        let _ = fs::remove_file(stop_path);
    }

    #[test]
    fn authenticated_player_that_ignores_stop_is_forcefully_reaped() {
        const HOLD_ENV: &str = "VYNTRA_TEST_HOLD_PLAYER_CHILD";
        const READY_ENV: &str = "VYNTRA_TEST_PLAYER_CHILD_READY";
        let ready_path =
            std::env::temp_dir().join(format!("vyntra-player-close-ready-{}", std::process::id()));
        let _ = fs::remove_file(&ready_path);
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shell::tests::hold_player_child_until_killed"])
            .env(HOLD_ENV, "1")
            .env(READY_ENV, &ready_path)
            .spawn()
            .unwrap();
        for _ in 0..200 {
            if ready_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(ready_path.exists(), "child Player fixture did not start");
        let mut child = Some(child);
        let graceful =
            stop_player_child(&mut child, || true, std::time::Duration::from_millis(20)).unwrap();
        assert!(!graceful);
        assert!(child.is_none());
        let _ = fs::remove_file(ready_path);
    }

    #[test]
    fn stop_aware_player_child() {
        let (Some(ready), Some(stop)) = (
            std::env::var_os("VYNTRA_TEST_STOP_PLAYER_READY"),
            std::env::var_os("VYNTRA_TEST_STOP_PLAYER_FILE"),
        ) else {
            return;
        };
        let ready = std::path::PathBuf::from(ready);
        let stop = std::path::PathBuf::from(stop);
        fs::write(ready, "ready").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !stop.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(stop.exists(), "parent did not send graceful Stop");
    }

    #[test]
    fn library_ui_identity_contains_catalog_code_revision_and_confidence_only() {
        let entry = library::LibraryEntry {
            id: "stable-id".into(),
            path: std::path::PathBuf::from("private/game.gba"),
            sources: Vec::new(),
            content_hash: None,
            title: None,
            system: Some("GBA".into()),
            game_code: Some("ABCD".into()),
            revision: Some(1),
            confidence: Some(domain::IdentityConfidence::Partial),
            last_seen_unix: None,
            favorite: false,
            last_played_unix: None,
            note: None,
        };
        let identity = game_identity_for_ui(Some(&entry));

        assert_eq!(identity["game_code"], "ABCD");
        assert_eq!(identity["revision"], 1);
        assert_eq!(identity["confidence"], "partial");
        assert!(identity.get("path").is_none());
        assert!(identity.get("content_hash").is_none());
    }

    fn cmd(action: &str) -> protocol::UiCommandEnvelope {
        protocol::UiCommandEnvelope {
            protocol_version: protocol::default_protocol_version(),
            command_id: "c1".into(),
            action: action.into(),
            request_id: None,
            session_id: None,
            epoch: None,
            expected_revision: None,
            payload: serde_json::Value::Null,
        }
    }

    fn session() -> domain::SessionIdentity {
        domain::SessionIdentity {
            session_id: "sess-1".into(),
            game_id: "game-1".into(),
            rom_sha256: None,
            core_hash: None,
            adapter_id: None,
            epoch: 3,
        }
    }

    #[test]
    fn commands_without_session_context_are_accepted() {
        assert!(validate_command_session(&None, 0, &cmd("scan")).is_ok());
    }

    #[test]
    fn matching_session_and_epoch_are_accepted() {
        let mut c = cmd("set");
        c.session_id = Some("sess-1".into());
        c.epoch = Some(3);
        assert!(validate_command_session(&Some(session()), 3, &c).is_ok());
    }

    #[test]
    fn stale_session_or_epoch_is_rejected() {
        let mut c = cmd("set");
        c.session_id = Some("sess-old".into());
        assert!(validate_command_session(&Some(session()), 3, &c).is_err());
        c.session_id = Some("sess-1".into());
        c.epoch = Some(2);
        assert!(validate_command_session(&Some(session()), 3, &c).is_err());
        assert!(validate_command_session(&None, 3, &c).is_err());
    }

    #[test]
    fn stop_is_allowed_after_its_player_session_disconnects() {
        let mut stop = cmd("stop_player");
        stop.session_id = Some("sess-1".into());
        stop.epoch = Some(3);
        assert!(validate_command_session(&None, 4, &stop).is_ok());
    }

    #[test]
    fn player_exit_notice_only_reports_unexpected_process_exit() {
        assert_eq!(player_exit_notice(true, Some(0)), None);
        assert_eq!(
            player_exit_notice(false, Some(9)).as_deref(),
            Some("Player terminó inesperadamente (código 9).")
        );
        assert_eq!(
            player_exit_notice(false, None).as_deref(),
            Some("Player terminó inesperadamente (sin código de salida).")
        );
    }

    #[test]
    fn disconnected_player_process_can_be_terminated_explicitly() {
        const HOLD_ENV: &str = "VYNTRA_TEST_HOLD_PLAYER_CHILD";
        const READY_ENV: &str = "VYNTRA_TEST_PLAYER_CHILD_READY";
        let ready_path =
            std::env::temp_dir().join(format!("vyntra-player-child-ready-{}", std::process::id()));
        let _ = fs::remove_file(&ready_path);
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shell::tests::hold_player_child_until_killed"])
            .env(HOLD_ENV, "1")
            .env(READY_ENV, &ready_path)
            .spawn()
            .unwrap();
        for _ in 0..200 {
            if ready_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        if !ready_path.exists() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child process did not start its hold test");
        }
        let mut owned_child = Some(child);

        terminate_child_process(&mut owned_child).unwrap();

        assert!(owned_child.is_none());
        let _ = fs::remove_file(ready_path);
    }

    #[test]
    fn hold_player_child_until_killed() {
        if std::env::var_os("VYNTRA_TEST_HOLD_PLAYER_CHILD").is_some() {
            let ready_path = std::env::var_os("VYNTRA_TEST_PLAYER_CHILD_READY").unwrap();
            fs::write(ready_path, b"ready").unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }
    }

    #[test]
    fn stale_command_revision_is_rejected_but_legacy_revision_is_allowed() {
        assert!(validate_command_revision(Some(7), 7).is_ok());
        assert!(validate_command_revision(Some(6), 7).is_err());
        assert!(validate_command_revision(None, 7).is_ok());
    }

    #[test]
    fn second_settings_window_reloads_external_change_and_rejects_old_revision() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-settings-two-windows-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("settings.json");
        let mut first_window = Settings::default();
        first_window.save(&path).unwrap();
        let stale_window = Settings::load(&path, &root).unwrap();
        let stale_fingerprint = settings_fingerprint(&path).unwrap();

        first_window.volume = 35;
        first_window.save(&path).unwrap();
        let (reloaded, new_fingerprint) =
            reload_external_settings(&path, &root, &stale_fingerprint)
                .unwrap()
                .expect("second window detects the changed settings file");
        assert_eq!(reloaded.volume, 35);
        assert_ne!(new_fingerprint, stale_fingerprint);
        assert!(validate_command_revision(Some(0), 1).is_err());
        assert_eq!(stale_window.volume, 80);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn failed_settings_write_keeps_active_preferences_unchanged() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-settings-write-failure-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("settings.json");
        let mut active = Settings::default();
        active.save(&path).unwrap();
        let original = fs::read(&path).unwrap();
        fs::create_dir(path.with_extension("tmp")).unwrap();
        let mut next = active.clone();
        next.library_view = "list".into();
        assert!(persist_settings(&mut active, next, &path).is_err());
        assert_eq!(active.library_view, "grid");
        assert_eq!(fs::read(&path).unwrap(), original);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn settings_mutex_serializes_independent_processes() {
        const READY_ENV: &str = "VYNTRA_SETTINGS_MUTEX_TEST_READY";
        if let Some(ready_path) = env::var_os(READY_ENV) {
            let _lock = SettingsMutex::acquire().unwrap();
            fs::write(ready_path, b"ready").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(500));
            return;
        }
        let ready =
            std::env::temp_dir().join(format!("vyntra-mutex-ready-{}.txt", std::process::id()));
        let _ = fs::remove_file(&ready);
        let mut child = std::process::Command::new(env::current_exe().unwrap())
            .arg("--exact")
            .arg("shell::tests::settings_mutex_serializes_independent_processes")
            .arg("--nocapture")
            .env(READY_ENV, &ready)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !ready.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            ready.exists(),
            "child process failed to acquire the named mutex"
        );
        let started = std::time::Instant::now();
        let _lock = SettingsMutex::acquire().unwrap();
        assert!(started.elapsed() >= std::time::Duration::from_millis(300));
        assert!(child.wait().unwrap().success());
        let _ = fs::remove_file(ready);
    }

    #[test]
    fn player_save_lock_blocks_a_second_process_for_the_same_game_only() {
        const SAVES_ENV: &str = "VYNTRA_PLAYER_SAVE_LOCK_TEST_SAVES";
        const STATES_ENV: &str = "VYNTRA_PLAYER_SAVE_LOCK_TEST_STATES";
        const RESULT_ENV: &str = "VYNTRA_PLAYER_SAVE_LOCK_TEST_RESULT";
        if let (Some(saves), Some(states), Some(result)) = (
            env::var_os(SAVES_ENV),
            env::var_os(STATES_ENV),
            env::var_os(RESULT_ENV),
        ) {
            let acquired =
                PlayerSaveLock::acquire(Path::new(&saves), Path::new(&states), "same-game").is_ok();
            fs::write(
                result,
                if acquired {
                    &b"acquired"[..]
                } else {
                    &b"busy"[..]
                },
            )
            .unwrap();
            return;
        }

        let root =
            std::env::temp_dir().join(format!("vyntra-player-save-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let saves = root.join("saves");
        let states = root.join("states");
        let other_saves = root.join("other-saves");
        let other_states = root.join("other-states");
        fs::create_dir_all(&saves).unwrap();
        fs::create_dir_all(&states).unwrap();
        fs::create_dir_all(&other_saves).unwrap();
        fs::create_dir_all(&other_states).unwrap();
        let lock = PlayerSaveLock::acquire(&saves, &states, "same-game").unwrap();
        let other_game = PlayerSaveLock::acquire(&saves, &states, "other-game");
        assert!(
            other_game.is_ok(),
            "different saves must remain independent"
        );
        drop(other_game);

        for (child_saves, child_states, result_name) in [
            (&saves, &other_states, "same-saves-result.txt"),
            (&other_saves, &states, "same-states-result.txt"),
        ] {
            let result = root.join(result_name);
            let mut child = std::process::Command::new(env::current_exe().unwrap())
                .arg("--exact")
                .arg(
                    "shell::tests::player_save_lock_blocks_a_second_process_for_the_same_game_only",
                )
                .env(SAVES_ENV, child_saves)
                .env(STATES_ENV, child_states)
                .env(RESULT_ENV, &result)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !result.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert!(
                result.exists(),
                "child process did not report its lock result"
            );
            assert_eq!(fs::read(&result).unwrap(), b"busy");
            assert!(child.wait().unwrap().success());
        }
        drop(lock);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_apply_accepts_known_legacy_roots_with_custom_save_folders() {
        let root =
            std::env::temp_dir().join(format!("vyntra-legacy-apply-roots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let saves = root.join("custom/saves");
        let states = root.join("custom/states");
        let historical = root.join("saves");
        let outside = root.join("outside");
        fs::create_dir_all(&saves).unwrap();
        fs::create_dir_all(&states).unwrap();
        fs::create_dir_all(&historical).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let known_source = historical.join("legacy.gb.sav");
        let outside_source = outside.join("unrelated.sav");
        fs::write(&known_source, b"legacy battery").unwrap();
        fs::write(&outside_source, b"outside data").unwrap();
        let destination = saves.join("by-id/game/battery.sav");
        let config = Settings {
            saves: saves.clone(),
            states,
            ..Settings::default()
        };
        let known_report = migration::preview(&[migration::MigrationItem::new(
            known_source,
            destination.clone(),
            "game".into(),
            "battery".into(),
        )])
        .unwrap();
        let outside_report = migration::preview(&[migration::MigrationItem::new(
            outside_source,
            destination,
            "game".into(),
            "battery".into(),
        )])
        .unwrap();

        assert!(validate_migration_report_paths(&known_report, &config, &root).is_ok());
        assert!(validate_migration_report_paths(&outside_report, &config, &root).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_apply_rejects_a_source_reached_through_an_escaping_junction() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-legacy-source-junction-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let saves = root.join("custom/saves");
        let states = root.join("custom/states");
        let historical = root.join("saves");
        let outside = root.join("outside");
        fs::create_dir_all(&saves).unwrap();
        fs::create_dir_all(&states).unwrap();
        fs::create_dir_all(&historical).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let linked_root = historical.join("linked");
        let output = std::process::Command::new("cmd.exe")
            .arg("/C")
            .arg(format!(
                "mklink /J {} {}",
                linked_root.display(),
                outside.display()
            ))
            .output()
            .expect("Windows cmd.exe is required for the junction fixture");
        assert!(
            output.status.success(),
            "could not create source junction: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let source = linked_root.join("legacy.gb.sav");
        fs::write(&source, b"outside battery").unwrap();
        let destination = saves.join("by-id/game/battery.sav");
        let config = Settings {
            saves,
            states,
            ..Settings::default()
        };
        let report = migration::preview(&[migration::MigrationItem::new(
            source,
            destination,
            "game".into(),
            "battery".into(),
        )])
        .unwrap();

        assert!(validate_migration_report_paths(&report, &config, &root).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_apply_rejects_a_destination_under_an_escaping_junction() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-legacy-destination-junction-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let saves = root.join("custom").join("saves");
        let states = root.join("custom").join("states");
        let outside = root.join("outside");
        fs::create_dir_all(&saves).unwrap();
        fs::create_dir_all(&states).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let linked_parent = saves.join("linked");
        let output = std::process::Command::new("cmd.exe")
            .arg("/C")
            .arg(format!(
                "mklink /J {} {}",
                linked_parent.display(),
                outside.display()
            ))
            .output()
            .expect("Windows cmd.exe is required for the junction fixture");
        assert!(
            output.status.success(),
            "could not create destination junction at {} -> {}: {}",
            linked_parent.display(),
            outside.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        let source = root.join("legacy.gb.sav");
        fs::write(&source, b"legacy battery").unwrap();
        let destination = linked_parent.join("game/battery.sav");
        let config = Settings {
            saves,
            states,
            ..Settings::default()
        };
        let report = migration::preview(&[migration::MigrationItem::new(
            source,
            destination,
            "game".into(),
            "battery".into(),
        )])
        .unwrap();

        assert!(validate_migration_report_paths(&report, &config, &root).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_preview_discovers_known_legacy_roots_outside_custom_folders() {
        let root = std::env::temp_dir().join(format!("vyntra-legacy-roots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let rom_path = root.join("roms/Same.gb");
        fs::create_dir_all(rom_path.parent().unwrap()).unwrap();
        fs::write(&rom_path, b"fixture rom").unwrap();
        fs::create_dir_all(root.join("data/saves")).unwrap();
        fs::create_dir_all(root.join("data/states")).unwrap();
        let battery = root.join("data/saves/Same.gb.sav");
        let state = root.join("data/states/Same.gb.ss");
        fs::write(&battery, b"legacy battery").unwrap();
        fs::write(&state, b"legacy state").unwrap();

        let game = Game {
            path: rom_path,
            title: "Same".into(),
            system: "GB",
            storage_key: std::sync::OnceLock::new(),
            rom_sha256: std::sync::OnceLock::new(),
        };
        let mut config = Settings::default();
        config.saves = root.join("custom/saves");
        config.states = root.join("custom/states");
        let items = legacy_migration_items(std::slice::from_ref(&game), &config, &root);

        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|item| item.source == battery));
        assert!(items.iter().any(|item| item.source == state));
        assert!(
            items
                .iter()
                .all(|item| item.destination.starts_with(&root.join("custom")))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_preview_keeps_homonymous_legacy_sram_as_conflict() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-legacy-homonym-preview-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let first_path = root.join("roms/one/Same.gb");
        let second_path = root.join("roms/two/Same.gb");
        fs::create_dir_all(first_path.parent().unwrap()).unwrap();
        fs::create_dir_all(second_path.parent().unwrap()).unwrap();
        fs::write(&first_path, b"first ROM").unwrap();
        fs::write(&second_path, b"second ROM").unwrap();
        let legacy_save = root.join("data/saves/Same.gb.sav");
        fs::create_dir_all(legacy_save.parent().unwrap()).unwrap();
        fs::write(&legacy_save, b"legacy SRAM").unwrap();
        let games = vec![game_from_path(first_path), game_from_path(second_path)];
        let mut config = Settings::default();
        config.saves = root.join("custom/saves");
        config.states = root.join("custom/states");

        let items = legacy_migration_items(&games, &config, &root);
        let report = migration::preview(&items).unwrap();
        let battery_items: Vec<_> = report
            .items
            .iter()
            .filter(|item| item.item.kind == "battery")
            .collect();

        assert_eq!(battery_items.len(), 2);
        assert!(
            battery_items
                .iter()
                .all(|item| item.status == migration::MigrationStatus::Conflict)
        );
        assert!(legacy_save.is_file());
        assert!(
            battery_items
                .iter()
                .all(|item| !item.item.destination.exists())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_listing_walks_by_id_directories_and_filters_extensions() {
        let root = std::env::temp_dir().join(format!("vyntra-saves-{}", std::process::id()));
        let nested = root.join("by-id").join("abc123");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("battery.sav"), b"ram").unwrap();
        fs::write(nested.join("slot1.ss"), b"state").unwrap();
        fs::write(nested.join("manifest.json"), b"{}").unwrap();
        fs::write(root.join("top.srm"), b"ram").unwrap();
        let mut found = save_files_in(&[&root]);
        found.sort();
        assert_eq!(found.len(), 3);
        assert!(found.contains(&nested.join("battery.sav")));
        assert!(found.contains(&nested.join("slot1.ss")));
        assert!(found.contains(&root.join("top.srm")));
        assert_eq!(save_files_in(&[&root.join("missing")]).len(), 0);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn vault_export_bundles_only_manifest_verified_saves() {
        let root = std::env::temp_dir().join(format!("vyntra-vault-export-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let rom = root.join("fixture.gb");
        fs::write(&rom, b"fixture rom").unwrap();
        let game = Game {
            path: rom,
            title: "Fixture".into(),
            system: "GB",
            storage_key: std::sync::OnceLock::new(),
            rom_sha256: std::sync::OnceLock::new(),
        };
        let state_root = root.join("states");
        let state_path = preferred_state_path(&game, &state_root);
        let payload = b"fixture state";
        atomic_write(&state_path, payload).unwrap();
        super::super::write_save_manifest(
            &game,
            &state_root,
            domain::SaveKind::SaveState,
            payload,
            Some(1),
            Some(&"b".repeat(64)),
            "committed",
        );
        let mut config = Settings::default();
        config.states = state_root;
        config.saves = root.join("saves");

        let bundle = bundle_for_save_file(&state_path, &config).unwrap();
        let (manifest, decoded) = crate::vault::decode_bundle(&bundle).unwrap();
        assert_eq!(manifest.storage_id, game_storage_key(&game));
        assert_eq!(decoded, payload);
        let manifest_path = state_path.parent().unwrap().join("state.manifest.json");
        fs::remove_file(manifest_path).unwrap();
        assert!(bundle_for_save_file(&state_path, &config).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vault_restore_requires_same_rom_and_core_and_backups_before_replace() {
        let root =
            std::env::temp_dir().join(format!("vyntra-vault-restore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let rom_path = root.join("fixture.gb");
        let core_path = root.join("fixture-core.dll");
        fs::write(&rom_path, b"fixture rom").unwrap();
        fs::write(&core_path, b"fixture core").unwrap();
        let game = Game {
            path: rom_path.clone(),
            title: "Fixture".into(),
            system: "GB",
            storage_key: std::sync::OnceLock::new(),
            rom_sha256: std::sync::OnceLock::new(),
        };
        let mut config = Settings::default();
        config.states = root.join("states");
        config.saves = root.join("saves");
        config.cores[0] = Some(core_path.clone());
        let rom_hash = identity::sha256_file(&rom_path).unwrap();
        let core_hash = identity::sha256_file(&core_path).unwrap();
        let state_path = preferred_state_path(&game, &config.states);
        atomic_write(&state_path, b"old state").unwrap();
        super::super::write_save_manifest(
            &game,
            &config.states,
            domain::SaveKind::SaveState,
            b"old state",
            Some(1),
            Some(&core_hash),
            "committed",
        );
        let payload = b"new portable state";
        let bundle = crate::vault::SaveBundleManifest {
            schema_version: 1,
            storage_id: game_storage_key(&game),
            kind: domain::SaveKind::SaveState,
            rom_sha256: rom_hash.clone(),
            core_hash: core_hash.clone(),
            state_format: Some("libretro-serialize-v1".into()),
            bytes: payload.len() as u64,
            sha256: identity::sha256_bytes(payload),
        };
        let prepared = prepare_bundle_restore(
            bundle.clone(),
            payload.to_vec(),
            std::slice::from_ref(&game),
            &config,
        )
        .unwrap();
        commit_bundle_restore(prepared, &config).unwrap();
        assert_eq!(fs::read(&state_path).unwrap(), payload);
        let backup_root = state_path.parent().unwrap().join("backups");
        assert_eq!(fs::read_dir(backup_root).unwrap().count(), 1);

        let manifest_path = state_path.parent().unwrap().join("state.manifest.json");
        let committed_manifest = fs::read(&manifest_path).unwrap();
        let interrupted_payload = b"second portable state";
        let interrupted_bundle = crate::vault::SaveBundleManifest {
            schema_version: 1,
            storage_id: game_storage_key(&game),
            kind: domain::SaveKind::SaveState,
            rom_sha256: rom_hash.clone(),
            core_hash: core_hash.clone(),
            state_format: Some("libretro-serialize-v1".into()),
            bytes: interrupted_payload.len() as u64,
            sha256: identity::sha256_bytes(interrupted_payload),
        };
        let interrupted = prepare_bundle_restore(
            interrupted_bundle,
            interrupted_payload.to_vec(),
            std::slice::from_ref(&game),
            &config,
        )
        .unwrap();
        let mut writes = 0;
        assert!(
            commit_bundle_restore_with(interrupted, &config, |path, bytes| {
                writes += 1;
                if writes == 2 {
                    return Err("fallo de escritura inyectado".into());
                }
                atomic_write(path, bytes)
            })
            .is_err()
        );
        assert_eq!(fs::read(&state_path).unwrap(), payload);
        assert_eq!(fs::read(&manifest_path).unwrap(), committed_manifest);

        let mut wrong_rom = bundle;
        wrong_rom.rom_sha256 = "0".repeat(64);
        assert!(
            prepare_bundle_restore(
                wrong_rom,
                payload.to_vec(),
                std::slice::from_ref(&game),
                &config
            )
            .is_err()
        );
        assert_eq!(fs::read(&state_path).unwrap(), payload);

        let wrong_core = crate::vault::SaveBundleManifest {
            schema_version: 1,
            storage_id: game_storage_key(&game),
            kind: domain::SaveKind::SaveState,
            rom_sha256: identity::sha256_file(&game.path).unwrap(),
            core_hash: "0".repeat(64),
            state_format: Some("libretro-serialize-v1".into()),
            bytes: payload.len() as u64,
            sha256: identity::sha256_bytes(payload),
        };
        assert!(
            prepare_bundle_restore(
                wrong_core,
                payload.to_vec(),
                std::slice::from_ref(&game),
                &config
            )
            .is_err()
        );

        atomic_write(&manifest_path, b"corrupt manifest").unwrap();
        let unverified_payload = b"must not replace an unverified save";
        let unverified_bundle = crate::vault::SaveBundleManifest {
            schema_version: 1,
            storage_id: game_storage_key(&game),
            kind: domain::SaveKind::SaveState,
            rom_sha256: identity::sha256_file(&game.path).unwrap(),
            core_hash,
            state_format: Some("libretro-serialize-v1".into()),
            bytes: unverified_payload.len() as u64,
            sha256: identity::sha256_bytes(unverified_payload),
        };
        let prepared = prepare_bundle_restore(
            unverified_bundle,
            unverified_payload.to_vec(),
            std::slice::from_ref(&game),
            &config,
        )
        .unwrap();
        assert!(commit_bundle_restore(prepared, &config).is_err());
        assert_eq!(fs::read(&state_path).unwrap(), payload);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[ignore = "smoke manual de restauración Vault con core mGBA local; requiere VYNTRA_CORE_GAME_SMOKE_PATH y VYNTRA_CORE_GAME_SMOKE_ROM"]
    fn configured_mgba_roundtrips_a_verified_vault_battery_restore() {
        let core_path = std::path::PathBuf::from(
            std::env::var_os("VYNTRA_CORE_GAME_SMOKE_PATH")
                .expect("define VYNTRA_CORE_GAME_SMOKE_PATH"),
        );
        let rom_path = std::path::PathBuf::from(
            std::env::var_os("VYNTRA_CORE_GAME_SMOKE_ROM")
                .expect("define VYNTRA_CORE_GAME_SMOKE_ROM"),
        );
        let root =
            std::env::temp_dir().join(format!("vyntra-vault-mgba-smoke-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let saves = root.join("saves");
        let states = root.join("states");
        let system = root.join("system");
        fs::create_dir_all(&saves).unwrap();
        fs::create_dir_all(&states).unwrap();
        fs::create_dir_all(&system).unwrap();

        let mut config = Settings::default();
        config.cores[1] = Some(core_path.clone());
        config.saves = saves.clone();
        config.states = states;
        let game = crate::game_from_path(rom_path);
        let games = std::slice::from_ref(&game);
        let mut core = crate::open_game(&game, games, &saves, &system, &config)
            .expect("open_game must start the homebrew ROM in mGBA");
        let original = core
            .read_save_ram()
            .expect("mGBA must expose battery RAM for Vault smoke");
        assert!(!original.is_empty());
        let core_hash = identity::sha256_file(&core_path).unwrap();
        crate::save_battery(&mut core, &game, &saves, Some(&core_hash));
        drop(core);

        let mut restored = original;
        restored[0] ^= 0xa5;
        let manifest = crate::vault::SaveBundleManifest {
            schema_version: 1,
            storage_id: crate::game_storage_key(&game),
            kind: domain::SaveKind::BatteryRam,
            rom_sha256: identity::sha256_file(&game.path).unwrap(),
            core_hash,
            state_format: None,
            bytes: restored.len() as u64,
            sha256: identity::sha256_bytes(&restored),
        };
        let archive = crate::vault::encode_bundle(&manifest, &restored).unwrap();
        let (manifest, payload) = crate::vault::decode_bundle(&archive).unwrap();
        let prepared = prepare_bundle_restore(manifest, payload, games, &config)
            .expect("Vault must validate the same ROM, core, and battery payload");
        commit_bundle_restore(prepared, &config)
            .expect("Vault must commit the backup and restored payload/manifest");

        let restored = crate::load_compatible_battery(&game, &saves, &core_path)
            .unwrap()
            .expect("restored Vault battery must validate against the committed manifest");
        let mut reopened = crate::open_game(&game, games, &saves, &system, &config)
            .expect("open_game must load the Vault-restored battery into mGBA");
        assert_eq!(
            reopened.read_save_ram().unwrap(),
            restored,
            "the real core must receive the bytes committed by Vault"
        );
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
}
