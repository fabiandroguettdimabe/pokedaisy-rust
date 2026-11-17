//! Native window + offline web UI. Emulation remains in the SDL/libretro process.
use super::*;
use crate::library;
use crate::protocol;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tao::{
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

enum Message {
    Action(String),
    Scanned(u64, Vec<Game>),
    CoreProbed(u64, serde_json::Value),
    Pipe(crate::player_pipe::PipeEvent),
    ImportProgress(u64, crate::import::ArchiveProgress),
    ImportFinished(
        u64,
        Vec<library::LibraryEntry>,
        crate::import::ImportSummary,
    ),
}

enum ImportSource {
    Loose(PathBuf),
    Archive(PathBuf, Vec<String>),
    Rejected(usize),
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
                let _ = ipc_proxy.send_event(Message::Action(request.body().clone()));
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
    let mut import_generation = 0u64;
    let mut active_import: Option<Arc<AtomicBool>> = None;
    let mut command_revision: u64 = 0;
    let mut pending_migration: Option<migration::MigrationReport> = None;
    let mut probe_generation = 0u64;
    let mut core_probe: Option<serde_json::Value> = None;
    event_loop.run(move |event,_,flow| {
        *flow=ControlFlow::Wait;
        match event {
            TaoEvent::WindowEvent { event:WindowEvent::CloseRequested,.. } => {
                if let Some(commands) = &player_commands { let _ = commands.send(crate::transport::PlayerCommand::Stop); }
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
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe);
            },
            TaoEvent::UserEvent(Message::CoreProbed(id, report)) if id == probe_generation => {
                core_probe = Some(report);
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe);
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
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe);
            },
            TaoEvent::UserEvent(Message::Pipe(event)) => {
                let matches_session = session.as_ref().is_some_and(|active| {
                    active.session_id == event.session_id && active.epoch == event.epoch
                });
                if matches_session {
                    match event.kind {
                        crate::player_pipe::PipeEventKind::Authenticated => {},
                        crate::player_pipe::PipeEventKind::Snapshot(snapshot) => {
                            if let Ok(value) = serde_json::to_string(&snapshot) {
                                let _ = webview.evaluate_script(&format!("window.vyntraPlayerSnapshot({value});"));
                            }
                        },
                        crate::player_pipe::PipeEventKind::Disconnected => {
                            player_commands = None;
                            session = None;
                            session_epoch = session_epoch.saturating_add(1);
                            publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe);
                        },
                        crate::player_pipe::PipeEventKind::Failed(reason) => {
                            player_commands = None;
                            eprintln!("Falló el canal de la sesión Player: {reason}");
                            session = None;
                            session_epoch = session_epoch.saturating_add(1);
                            publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe);
                        },
                    }
                }
            },
            TaoEvent::UserEvent(Message::Action(body)) => {
                let cmd = match protocol::parse_legacy_action(&body) {
                    Ok(cmd) => cmd,
                    Err(_) => return,
                };
                let action = cmd.payload.clone();
                let name = cmd.action.as_str();
                let mut refresh = false;
                let mut command_result: Option<serde_json::Value> = None;
                let result = (|| -> Result<(), String> {
                    validate_command_revision(cmd.expected_revision, command_revision)?;
                    validate_command_session(&session, session_epoch, &cmd)?;
                    match name {
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
                        "library_view" => {
                            let view = action["value"].as_str().ok_or("Vista inválida")?;
                            if !matches!(view, "grid" | "list") { return Err("Vista inválida".into()); }
                            config.library_view = view.into();
                            config.save(&config_path)?;
                        },
                        "library_filter" => {
                            let filter = action["value"].as_str().ok_or("Filtro inválido")?;
                            if !matches!(filter, "Todos" | "Favoritos" | "GB" | "GBC" | "GBA" | "DS") { return Err("Filtro inválido".into()); }
                            config.library_filter = filter.into();
                            config.save(&config_path)?;
                        },
                        "library_selection" => {
                            let selection = action["value"].as_str().ok_or("Selección inválida")?;
                            if selection.len() > 64 || !selection.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)) { return Err("Selección inválida".into()); }
                            config.library_selection = Some(selection.into());
                            config.save(&config_path)?;
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
                        "choose" => {
                            let field=action["field"].as_str().unwrap_or("");
                            let mut next=config.clone();
                            if let Some(i)=field.strip_prefix("core").and_then(|v|v.parse::<usize>().ok()).filter(|i|*i<3) {
                                if let Some(p)=rfd::FileDialog::new().set_title("Elegir núcleo libretro de confianza").add_filter("Núcleo Windows", &["dll"]).pick_file(){next.cores[i]=Some(p);}else{return Ok(());}
                            } else if ["library","saves","states","system"].contains(&field) {
                                if let Some(p)=rfd::FileDialog::new().set_title("Seleccionar carpeta · no mueve archivos anteriores").pick_folder() {
                                    *match field {"library"=>&mut next.library,"saves"=>&mut next.saves,"states"=>&mut next.states,_=>&mut next.system}=p;
                                } else{return Ok(());}
                            } else { return Err("Opción no válida".into()); }
                            next.save(&config_path)?; config=next; refresh=true;
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
                            if let Some(running)=child.as_mut(){
                                match running.try_wait().map_err(|e|e.to_string())? {
                                    None => return Err("Ya hay una partida abierta. Vuelve a su ventana para continuar.".into()),
                                    Some(_) => {
                                        child = None;
                                        session = None;
                                    }
                                }
                            }
                            let game = if let Some(storage_id) = action["storage_id"].as_str() {
                                crate::resolve_game_by_storage_id(&games, storage_id).ok_or("El juego cambió desde que se mostró la biblioteca; actualízala")?
                            } else {
                                games.get(action["id"].as_u64().ok_or("Selecciona un juego")? as usize).ok_or("Juego no encontrado")?
                            };
                            if !game.path.is_file(){return Err("La ROM ya no estǭ en su ubicaci��n. Imp��rtala desde su carpeta actual.".into());}
                            crate::validate_game_identity(game)?;
                            let core=config.core(game.system)?;
                            if !core.is_file(){return Err("La DLL seleccionada no existe. Revisa Ajustes > Nǧcleos.".into());}
                            let game_id = game_storage_key(game);
                            if library::mark_played(&mut imported, &game_id, now_unix()) {
                                library::save(&index_path, &imported)?;
                            }
                            let next_epoch = session_epoch.saturating_add(1);
                            let session_id = format!("sess-{}-{next_epoch}", std::process::id());
                            let pipe_proxy = proxy.clone();
                            let pipe = crate::player_pipe::start(session_id.clone(), next_epoch, move |event| {
                                let _ = pipe_proxy.send_event(Message::Pipe(event));
                            })?;
                            player_commands = Some(pipe.commands.clone());
                            child=Some(std::process::Command::new(env::current_exe().map_err(|e|e.to_string())?)
                                .arg("--emulator").arg("--data").arg(&root).arg(&game.path)
                                .arg("--pipe").arg(&pipe.name).arg("--nonce").arg(&pipe.nonce)
                                .arg("--session-id").arg(&session_id).arg("--session-epoch").arg(next_epoch.to_string())
                                .spawn().map_err(|e|e.to_string())?);
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
                            player_commands.as_ref().ok_or("No hay una partida conectada")?
                                .send(crate::transport::PlayerCommand::Stop)
                                .map_err(|error| format!("No se pudo detener Player: {error:?}"))?;
                        },
                        "pause_player" => {
                            let paused = action["value"].as_bool().ok_or("Estado de pausa inválido")?;
                            player_commands.as_ref().ok_or("No hay una partida conectada")?
                                .send(crate::transport::PlayerCommand::Pause(paused))
                                .map_err(|error| format!("No se pudo cambiar la pausa de Player: {error:?}"))?;
                        },
                        "open_folder"=> {
                            let field=action["field"].as_str().unwrap_or("");
                            let path=match field {"saves"=>&config.saves,"states"=>&config.states,"library"=>&config.library,"system"=>&config.system,"config"=>&data,_=>return Err("Carpeta no válida".into())};
                            std::process::Command::new("explorer.exe").arg(path).spawn().map_err(|e|e.to_string())?;
                        },
                        "export_save"=> {
                            if let Some(running)=child.as_mut(){if running.try_wait().map_err(|e|e.to_string())?.is_none(){return Err("Cierra la partida antes de exportar para evitar copiar un guardado durante su escritura.".into());}}
                            let files=save_files(&config);
                            let requested=PathBuf::from(action["file"].as_str().ok_or("Guardado inválido")?);
                            let source=files.iter().find(|path|**path==requested).ok_or("El guardado ya no está disponible; actualiza la lista")?;
                            if let Some(dest)=rfd::FileDialog::new().set_title("Exportar una copia del guardado").set_file_name(source.file_name().unwrap_or_default().to_string_lossy()).save_file(){
                                if source==&dest {return Err("Elige una ubicación distinta al original".into());}
                                fs::copy(source,dest).map_err(|e|e.to_string())?;
                            }
                        },
                        "migration_preview" => {
                            let report = migration::preview(&legacy_migration_items(&games, &config))?;
                            command_result = Some(json!({"migration": report}));
                            pending_migration = Some(report);
                        },
                        "migration_apply" => {
                            let report = pending_migration.clone().ok_or("Primero revisa los guardados heredados")?;
                            for entry in &report.items {
                                if !entry.item.source.starts_with(&config.saves) && !entry.item.source.starts_with(&config.states) {
                                    return Err("El journal contiene una ruta fuera de las carpetas configuradas".into());
                                }
                                if !entry.item.destination.starts_with(&config.saves) && !entry.item.destination.starts_with(&config.states) {
                                    return Err("El destino queda fuera de las carpetas configuradas".into());
                                }
                            }
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
                command_revision = command_revision.saturating_add(1);
                let ack = match &result {
                    Ok(_) => {
                        let mut ack = protocol::UiAck::success(cmd.command_id.clone(), command_result.take());
                        ack.request_id = cmd.request_id.clone();
                        ack.revision = Some(command_revision);
                        ack
                    }
                    Err(error) => {
                        let mut ack = protocol::UiAck::failure(
                            cmd.command_id.clone(),
                            protocol::AppError::new("command_failed", error.clone()),
                        );
                        ack.request_id = cmd.request_id.clone();
                        ack.revision = Some(command_revision);
                        ack
                    }
                };
                let _ = webview.evaluate_script(&format!("window.vyntraAck&&window.vyntraAck({});", json!(ack)));
                if let Err(error)=result { let _=webview.evaluate_script(&format!("window.vyntraError({});",json!(error))); }
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
                publish(&webview,&config,&games,&imported,scanning,&session,session_epoch,command_revision,&core_probe);
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

fn validate_command_session(
    session: &Option<domain::SessionIdentity>,
    session_epoch: u64,
    cmd: &protocol::UiCommandEnvelope,
) -> Result<(), String> {
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

fn validate_command_revision(expected: Option<u64>, current: u64) -> Result<(), String> {
    if expected.is_some_and(|revision| revision != current) {
        return Err("La biblioteca cambió desde tu última acción; vuelve a intentarlo".into());
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

fn legacy_migration_items(games: &[Game], config: &Settings) -> Vec<migration::MigrationItem> {
    let mut items = Vec::new();
    for game in games {
        let game_id = game_storage_key(game);
        let battery_source = legacy_battery_path(game, &config.saves);
        if battery_source.is_file() {
            items.push(migration::MigrationItem::new(
                battery_source,
                preferred_battery_path(game, &config.saves),
                game_id.clone(),
                "battery".into(),
            ));
        }
        let state_source = legacy_state_path(game, &config.states);
        if state_source.is_file() {
            items.push(migration::MigrationItem::new(
                state_source,
                preferred_state_path(game, &config.states),
                game_id,
                "state_unverified".into(),
            ));
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
) {
    let games:Vec<_>=games.iter().enumerate().map(|(id,g)|{let storage_id=game_storage_key(g);let metadata=imported.iter().find(|entry|entry.id==storage_id);json!({"id":id,"storage_id":storage_id,"title":g.title,"system":g.system,"path":g.path,"missing":!g.path.is_file(),"core":config.core(g.system).is_ok_and(|p|p.is_file()),"favorite":metadata.is_some_and(|entry|entry.favorite),"last_played_unix":metadata.and_then(|entry|entry.last_played_unix),"note":metadata.and_then(|entry|entry.note.clone()).unwrap_or_default()})}).collect();
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
    let _ = webview.evaluate_script(&format!("window.vyntraState({value});"));
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn stale_command_revision_is_rejected_but_legacy_revision_is_allowed() {
        assert!(validate_command_revision(Some(7), 7).is_ok());
        assert!(validate_command_revision(Some(6), 7).is_err());
        assert!(validate_command_revision(None, 7).is_ok());
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
}
