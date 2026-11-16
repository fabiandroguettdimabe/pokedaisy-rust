//! Native window + offline web UI. Emulation remains in the SDL/libretro process.
use super::*;
use serde_json::{Value, json};
use tao::{
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

enum Message {
    Action(String),
    Scanned(u64, Vec<Game>),
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
    let mut imported: Vec<PathBuf> = match fs::read(&index_path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Índice de biblioteca inválido: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.to_string()),
    };
    for arg in &args {
        let p = PathBuf::from(arg);
        if supported(&p) && p.is_file() && !imported.contains(&p) {
            imported.push(p);
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
    event_loop.run(move |event,_,flow| {
        *flow=ControlFlow::Wait;
        match event {
            TaoEvent::WindowEvent { event:WindowEvent::CloseRequested,.. } => *flow=ControlFlow::Exit,
            TaoEvent::UserEvent(Message::Scanned(id,mut found)) if id==generation => {
                for path in &imported { if !found.iter().any(|g|&g.path==path) { found.push(game_from_path(path.clone())); } }
                games=found; scanning=false;
                publish(&webview,&config,&games,scanning);
            },
            TaoEvent::UserEvent(Message::Action(body)) => {
                let action:Value=match serde_json::from_str(&body){Ok(v)=>v,Err(_)=>return};
                let name=action["action"].as_str().unwrap_or("");
                let mut refresh=false;
                let result=(|| -> Result<(),String> {
                    match name {
                        "ready" | "scan" => refresh=true,
                        "import" => {
                            if let Some(paths)=rfd::FileDialog::new().set_title("Importar ROMs a Vyntra").add_filter("ROMs GB / GBC / GBA / DS", &["gb","gbc","gba","nds"]).pick_files() {
                                for path in paths { if supported(&path) && !imported.contains(&path){ imported.push(path); } }
                                let bytes=serde_json::to_vec_pretty(&imported).map_err(|e|e.to_string())?;
                                if index_path.exists(){fs::copy(&index_path,index_path.with_extension("json.bak")).map_err(|e|e.to_string())?;}
                                let temp=index_path.with_extension("tmp"); fs::write(&temp,bytes).map_err(|e|e.to_string())?;
                                fs::rename(temp,&index_path).map_err(|e|e.to_string())?; refresh=true;
                            }
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
                            if let Some(running)=child.as_mut(){if running.try_wait().map_err(|e|e.to_string())?.is_none(){return Err("Ya hay una partida abierta. Vuelve a su ventana para continuar.".into());}}
                            let game=games.get(action["id"].as_u64().ok_or("Selecciona un juego")? as usize).ok_or("Juego no encontrado")?;
                            if !game.path.is_file(){return Err("La ROM ya no está en su ubicación. Impórtala desde su carpeta actual.".into());}
                            let core=config.core(game.system)?;
                            if !core.is_file(){return Err("La DLL seleccionada no existe. Revisa Ajustes > Núcleos.".into());}
                            child=Some(std::process::Command::new(env::current_exe().map_err(|e|e.to_string())?).arg("--emulator").arg("--data").arg(&root).arg(&game.path).spawn().map_err(|e|e.to_string())?);
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
                        _=>return Err("Acción no disponible".into()),
                    } Ok(())
                })();
                if let Err(error)=result { let _=webview.evaluate_script(&format!("window.vyntraError({});",json!(error))); }
                if refresh {generation+=1; scanning=true; let id=generation; let cfg=config.clone(); let tx=proxy.clone(); std::thread::spawn(move||{let found=scan_games(&cfg.library,cfg.recursive);let _=tx.send_event(Message::Scanned(id,found));});}
                publish(&webview,&config,&games,scanning);
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

fn save_files(config: &Settings) -> Vec<PathBuf> {
    let mut files: Vec<_> = [&config.saves, &config.states]
        .iter()
        .flat_map(|dir| fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| ["sav", "srm", "ss", "state"].contains(&s))
        })
        .collect();
    files.sort_by_key(|p| std::cmp::Reverse(p.metadata().ok().and_then(|m| m.modified().ok())));
    files
}
fn publish(webview: &wry::WebView, config: &Settings, games: &[Game], scanning: bool) {
    let games:Vec<_>=games.iter().enumerate().map(|(id,g)|json!({"id":id,"title":g.title,"system":g.system,"path":g.path,"missing":!g.path.is_file(),"core":config.core(g.system).is_ok_and(|p|p.is_file())})).collect();
    let cores:Vec<_>=["GBC","GBA","DS"].iter().enumerate().map(|(id,s)| {let path=config.core(s).ok();json!({"id":id,"system":s,"path":path,"installed":path.as_ref().is_some_and(|p|p.is_file())})}).collect();
    let saves:Vec<_>=save_files(config).iter().enumerate().map(|(id,p)|{let meta=p.metadata().ok();json!({"id":id,"path":p,"name":p.file_name().unwrap_or_default().to_string_lossy(),"bytes":meta.as_ref().map(|m|m.len()),"date":meta.and_then(|m|m.modified().ok()).and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_secs()),"state":p.starts_with(&config.states)})}).collect();
    let value =
        json!({"config":config,"games":games,"cores":cores,"saves":saves,"scanning":scanning});
    let _ = webview.evaluate_script(&format!("window.vyntraState({value});"));
}
