use fontdue::Font;
use pokedaisy_libretro::{
    CorePaths, JoypadButton, JoypadState, LibretroCore, PixelFormat, VideoFrame,
};
use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const BG: Color = Color::RGB(238, 236, 220);
const INK: Color = Color::RGB(25, 42, 39);
const MUTED: Color = Color::RGB(93, 108, 101);
const GREEN: Color = Color::RGB(37, 102, 71);
const AQUA: Color = Color::RGB(150, 211, 189);
const PAPER: Color = Color::RGB(255, 255, 248);
const BORDER: Color = Color::RGB(187, 197, 180);
const SIDEBAR: Color = Color::RGB(25, 62, 52);
const CORAL: Color = Color::RGB(226, 103, 75);

#[derive(Clone)]
struct Game {
    path: PathBuf,
    title: String,
    system: &'static str,
}

#[derive(Clone, Copy)]
enum Screen {
    Library,
    Tools,
    Settings,
    Playing,
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut root = env::current_dir().map_err(|e| e.to_string())?;
    let mut rom_arg = None;
    let mut arg = 0;
    while arg < args.len() {
        if args[arg] == "--data" {
            if let Some(value) = args.get(arg + 1) {
                root = PathBuf::from(value);
            }
            arg += 2;
        } else {
            let path = PathBuf::from(&args[arg]);
            if supported(&path) && path.exists() {
                rom_arg = Some(path);
            }
            arg += 1;
        }
    }
    let data = root.join("data");
    let rom_dir = data.join("roms");
    fs::create_dir_all(&rom_dir)
        .map_err(|e| format!("No se pudo crear {}: {e}", rom_dir.display()))?;
    let save_dir = data.join("saves");
    let state_dir = data.join("states");
    let system_dir = data.join("system");
    for dir in [&save_dir, &state_dir, &system_dir] {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let audio_subsystem = sdl.audio()?;
    let audio_device = audio_subsystem.open_queue::<i16, _>(
        None,
        &AudioSpecDesired {
            freq: Some(32_768),
            channels: Some(2),
            samples: Some(1024),
        },
    )?;
    audio_device.resume();
    let controller_subsystem = sdl.game_controller()?;
    let mut game_controller = if controller_subsystem.num_joysticks()? > 0 {
        controller_subsystem.open(0).ok()
    } else {
        None
    };
    let display = video.desktop_display_mode(0)?;
    let window_w = ((display.w as f32 * 0.60) as u32).clamp(820, 1120);
    let window_h = ((display.h as f32 * 0.58) as u32).clamp(520, 740);
    let window = video
        .window("PokeDaisy · Desktop", window_w, window_h)
        .position_centered()
        .resizable()
        .allow_highdpi()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    let pixel_font = Font::from_bytes(
        include_bytes!("../../../assets/PixelOperator.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .map_err(|e| e.to_string())?;
    let body_font = Font::from_bytes(
        include_bytes!("../../../assets/AtkinsonHyperlegible-Regular.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .map_err(|e| e.to_string())?;
    let strong_font = Font::from_bytes(
        include_bytes!("../../../assets/AtkinsonHyperlegible-Bold.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .map_err(|e| e.to_string())?;
    let logo_image = image::load_from_memory(include_bytes!("../../../assets/pokedaisy-mark.png"))
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (logo_w, logo_h) = logo_image.dimensions();
    let mut logo = texture_creator
        .create_texture_static(sdl2::pixels::PixelFormatEnum::RGBA32, logo_w, logo_h)
        .map_err(|e| e.to_string())?;
    logo.set_blend_mode(sdl2::render::BlendMode::Blend);
    logo.update(None, logo_image.as_raw(), logo_w as usize * 4)
        .map_err(|e| e.to_string())?;
    let mut event_pump = sdl.event_pump()?;
    let mut games = scan_games(&rom_dir);
    let mut selected = 0usize;
    let mut screen = Screen::Library;
    let mut core: Option<LibretroCore> = None;
    let mut last_error = None;
    let mut last_frame = None;
    let mut last_auto = Instant::now();
    let mut last_save = Instant::now();
    if let Some(path) = rom_arg {
        if let Some(index) = games.iter().position(|g| g.path == path) {
            selected = index;
        } else {
            games.insert(0, game_from_path(path));
        }
        if let Some(game) = games.get(selected) {
            match open_game(game, &save_dir, &system_dir) {
                Ok(c) => {
                    core = Some(c);
                    screen = Screen::Playing;
                    last_error = None;
                }
                Err(e) => last_error = Some(e),
            }
        }
    }

    'app: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'app,
                Event::DropFile { filename, .. } => {
                    let p = PathBuf::from(filename);
                    if supported(&p) {
                        if let Some(i) = games.iter().position(|g| g.path == p) {
                            selected = i;
                        } else {
                            games.insert(0, game_from_path(p));
                            selected = 0;
                        }
                        screen = Screen::Library;
                    }
                }
                Event::ControllerDeviceAdded { which, .. } if game_controller.is_none() => {
                    game_controller = controller_subsystem.open(which).ok();
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    if game_controller
                        .as_ref()
                        .is_some_and(|c| c.instance_id() == which)
                    {
                        game_controller = None;
                    }
                }
                Event::MouseButtonDown { x, y, clicks, .. }
                    if !matches!(screen, Screen::Playing) =>
                {
                    if x < 176 {
                        screen = match y {
                            116..=174 => Screen::Library,
                            182..=240 => Screen::Tools,
                            248..=306 => Screen::Settings,
                            _ => screen,
                        };
                        continue;
                    }
                    if !matches!(screen, Screen::Library) {
                        continue;
                    }
                    let (w, _) = canvas.output_size()?;
                    let add_rom_clicked = (y <= 100 && x >= w as i32 - 205)
                        || (games.is_empty()
                            && (407..=458).contains(&y)
                            && (226..=410).contains(&x));
                    if add_rom_clicked {
                        if let Some(path) = pick_rom() {
                            if let Some(i) = games.iter().position(|g| g.path == path) {
                                selected = i;
                            } else {
                                games.insert(0, game_from_path(path));
                                selected = 0;
                            }
                            if clicks >= 1 {
                                if let Some(game) = games.get(selected).cloned() {
                                    match open_game(&game, &save_dir, &system_dir) {
                                        Ok(c) => {
                                            core = Some(c);
                                            screen = Screen::Playing;
                                            last_error = None;
                                        }
                                        Err(e) => last_error = Some(e),
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    let layout = library_grid_metrics(w, canvas.output_size()?.1);
                    if let Some(index) = layout.index_at(x, y, selected) {
                        if index < games.len() {
                            selected = index;
                            if clicks >= 2 {
                                if let Some(game) = games.get(selected).cloned() {
                                    match open_game(&game, &save_dir, &system_dir) {
                                        Ok(c) => {
                                            core = Some(c);
                                            screen = Screen::Playing;
                                            last_error = None;
                                        }
                                        Err(e) => last_error = Some(e),
                                    }
                                }
                            }
                        }
                    }
                }
                Event::MouseWheel { y, .. } if matches!(screen, Screen::Library) => {
                    let (w, h) = canvas.output_size()?;
                    let step = library_grid_metrics(w, h).columns.max(1);
                    if y < 0 {
                        selected = (selected + step).min(games.len().saturating_sub(1));
                    } else if y > 0 {
                        selected = selected.saturating_sub(step);
                    }
                }
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => match screen {
                    Screen::Library => match key {
                        Keycode::Down | Keycode::J => {
                            if !games.is_empty() {
                                selected = (selected + 1).min(games.len() - 1);
                            }
                        }
                        Keycode::Up | Keycode::K => selected = selected.saturating_sub(1),
                        Keycode::Return | Keycode::Space => {
                            if let Some(game) = games.get(selected).cloned() {
                                match open_game(&game, &save_dir, &system_dir) {
                                    Ok(c) => {
                                        core = Some(c);
                                        screen = Screen::Playing;
                                        last_error = None;
                                    }
                                    Err(e) => last_error = Some(e),
                                }
                            }
                        }
                        Keycode::F5 => games = scan_games(&rom_dir),
                        Keycode::Escape => break 'app,
                        _ => {}
                    },
                    Screen::Tools | Screen::Settings => match key {
                        Keycode::Escape | Keycode::G | Keycode::Home => screen = Screen::Library,
                        Keycode::Return | Keycode::Space => {
                            if let Some(game) = games.get(selected).cloned() {
                                match open_game(&game, &save_dir, &system_dir) {
                                    Ok(c) => {
                                        core = Some(c);
                                        screen = Screen::Playing;
                                        last_error = None;
                                    }
                                    Err(e) => last_error = Some(e),
                                }
                            }
                        }
                        _ => {}
                    },
                    Screen::Playing => match key {
                        Keycode::Escape => {
                            if let Some(c) = core.as_mut() {
                                save_battery(c, &games[selected].path, &save_dir);
                            }
                            core = None;
                            screen = Screen::Library;
                            last_frame = None;
                        }
                        Keycode::F2 => {
                            if let (Some(c), Some(g)) = (core.as_mut(), games.get(selected)) {
                                match c.serialize() {
                                    Ok(bytes) => {
                                        let path = state_path(&g.path, &state_dir);
                                        if let Err(e) = fs::write(path, bytes) {
                                            last_error =
                                                Some(format!("No se pudo guardar estado: {e}"));
                                        }
                                    }
                                    Err(e) => {
                                        last_error = Some(format!("Estado no disponible: {e:?}"))
                                    }
                                }
                            }
                        }
                        Keycode::F4 => {
                            if let (Some(c), Some(g)) = (core.as_mut(), games.get(selected)) {
                                match fs::read(state_path(&g.path, &state_dir)) {
                                    Ok(bytes) => {
                                        if let Err(e) = c.unserialize(&bytes) {
                                            last_error =
                                                Some(format!("No se pudo cargar estado: {e:?}"));
                                        }
                                    }
                                    Err(e) => {
                                        last_error = Some(format!("No se encontró el estado: {e}"))
                                    }
                                }
                            }
                        }
                        Keycode::F11 => {
                            let _ = canvas
                                .window_mut()
                                .set_fullscreen(sdl2::video::FullscreenType::Desktop);
                        }
                        _ => {}
                    },
                },
                _ => {}
            }
        }

        let (w, h) = canvas.output_size()?;
        canvas.set_draw_color(BG);
        canvas.clear();
        match screen {
            Screen::Library => draw_library(
                &mut canvas,
                &pixel_font,
                &body_font,
                &strong_font,
                &logo,
                &games,
                selected,
                w,
                h,
                last_error.as_deref(),
            )?,
            Screen::Tools => draw_info_page(
                &mut canvas,
                &pixel_font,
                &body_font,
                &strong_font,
                &logo,
                w,
                h,
                false,
            )?,
            Screen::Settings => draw_info_page(
                &mut canvas,
                &pixel_font,
                &body_font,
                &strong_font,
                &logo,
                w,
                h,
                true,
            )?,
            Screen::Playing => {
                let fast_forward = event_pump
                    .keyboard_state()
                    .is_scancode_pressed(sdl2::keyboard::Scancode::Space);
                let frame_interval = if fast_forward {
                    Duration::from_millis(1)
                } else {
                    Duration::from_micros(16_667)
                };
                if last_auto.elapsed() >= frame_interval {
                    if let Some(c) = core.as_mut() {
                        c.set_fast_forward(fast_forward);
                        let keys = event_pump.keyboard_state();
                        let mut input = JoypadState::default();
                        for (k, b) in [
                            (Keycode::Z, JoypadButton::A),
                            (Keycode::X, JoypadButton::B),
                            (Keycode::F, JoypadButton::Y),
                            (Keycode::Tab, JoypadButton::X),
                            (Keycode::Return, JoypadButton::Start),
                            (Keycode::Backspace, JoypadButton::Select),
                            (Keycode::Up, JoypadButton::Up),
                            (Keycode::Down, JoypadButton::Down),
                            (Keycode::Left, JoypadButton::Left),
                            (Keycode::Right, JoypadButton::Right),
                            (Keycode::Q, JoypadButton::L),
                            (Keycode::E, JoypadButton::R),
                            (Keycode::Num1, JoypadButton::L2),
                            (Keycode::Num2, JoypadButton::R2),
                        ] {
                            if keys.is_scancode_pressed(pressed_key(k)) {
                                input.set(b, true);
                            }
                        }
                        if let Some(pad) = game_controller.as_ref() {
                            use sdl2::controller::{Axis, Button};
                            for (button, retro) in [
                                (Button::B, JoypadButton::B),
                                (Button::A, JoypadButton::A),
                                (Button::Y, JoypadButton::Y),
                                (Button::X, JoypadButton::X),
                                (Button::Start, JoypadButton::Start),
                                (Button::Back, JoypadButton::Select),
                                (Button::DPadUp, JoypadButton::Up),
                                (Button::DPadDown, JoypadButton::Down),
                                (Button::DPadLeft, JoypadButton::Left),
                                (Button::DPadRight, JoypadButton::Right),
                                (Button::LeftShoulder, JoypadButton::L),
                                (Button::RightShoulder, JoypadButton::R),
                            ] {
                                if pad.button(button) {
                                    input.set(retro, true);
                                }
                            }
                            if pad.axis(Axis::TriggerLeft) > 16_384 {
                                input.set(JoypadButton::L2, true);
                            }
                            if pad.axis(Axis::TriggerRight) > 16_384 {
                                input.set(JoypadButton::R2, true);
                            }
                        }
                        match c.run_frame(input) {
                            Ok(f) => {
                                last_frame = f.video;
                                if !f.audio.is_empty() {
                                    if fast_forward {
                                        audio_device.clear();
                                    } else {
                                        let _ = audio_device.queue_audio(&f.audio);
                                    }
                                }
                            }
                            Err(e) => last_error = Some(format!("Error del core: {e:?}")),
                        }
                        if let Some(frame) = last_frame.as_ref() {
                            draw_play(&mut canvas, &texture_creator, frame, w, h)?;
                        }
                        if let Some(e) = last_error.as_deref() {
                            text(
                                &mut canvas,
                                &pixel_font,
                                e,
                                40,
                                h as i32 - 38,
                                14,
                                Color::RGB(196, 48, 48),
                            );
                        }
                        if last_save.elapsed() > Duration::from_secs(20) {
                            if let Some(p) = games.get(selected).map(|g| g.path.clone()) {
                                if let Some(c) = core.as_mut() {
                                    save_battery(c, &p, &save_dir);
                                }
                                last_save = Instant::now();
                            }
                        }
                    }
                    last_auto = Instant::now();
                } else if let Some(frame) = last_frame.as_ref() {
                    draw_play(&mut canvas, &texture_creator, frame, w, h)?;
                }
                draw_play_header(&mut canvas, &pixel_font, w)?;
            }
        }
        canvas.present();
        std::thread::sleep(Duration::from_millis(4));
    }
    if let Some(c) = core.as_mut() {
        if let Some(g) = games.get(selected) {
            save_battery(c, &g.path, &save_dir);
        }
    }
    Ok(())
}

// KeyboardState uses the SDL scancode enum; keep the game mapping in one compact helper.
fn pressed_key(key: Keycode) -> sdl2::keyboard::Scancode {
    sdl2::keyboard::Scancode::from_keycode(key).unwrap_or(sdl2::keyboard::Scancode::A)
}

fn open_game(game: &Game, saves: &Path, system: &Path) -> Result<LibretroCore, String> {
    let core_path = find_core(game.system)?;
    let mut core = LibretroCore::load(
        &core_path,
        CorePaths {
            save_dir: saves.to_path_buf(),
            system_dir: system.to_path_buf(),
            panel_width: 1280,
            panel_height: 760,
        },
    )
    .map_err(|e| format!("No se pudo iniciar {}: {e:?}", core_path.display()))?;
    core.load_game(&game.path)
        .map_err(|e| format!("No se pudo abrir {}: {e:?}", game.path.display()))?;
    let save_path = saves.join(format!(
        "{}.sav",
        game.path.file_name().unwrap_or_default().to_string_lossy()
    ));
    if let Ok(bytes) = fs::read(save_path) {
        let _ = core.write_save_ram(&bytes);
    }
    Ok(core)
}

fn find_core(system: &str) -> Result<PathBuf, String> {
    let mut dirs = vec![env::current_dir().map_err(|e| e.to_string())?.join("cores")];
    if let Ok(d) = env::var("POKEDAISY_CORE_DIR") {
        dirs.insert(0, PathBuf::from(d));
    }
    let names: &[&str] = match system {
        "GBA" => &["pokedaisy_gba_libretro.dll", "pokedaisy_gba_libretro.so"],
        "DS" => &["pokedaisy_ds_libretro.dll", "pokedaisy_ds_libretro.so"],
        _ => &["pokedaisy_gbc_libretro.dll", "pokedaisy_gbc_libretro.so"],
    };
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| p.exists())
        .ok_or_else(|| {
            format!(
                "Falta el core de {system}. Coloca el DLL en ./cores o define POKEDAISY_CORE_DIR."
            )
        })
}

fn scan_games(root: &Path) -> Vec<Game> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p)
            } else if supported(&p) {
                out.push(game_from_path(p));
            }
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}
fn supported(p: &Path) -> bool {
    p.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "gb" | "gbc" | "gba" | "nds"
        )
    })
}
fn pick_rom() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open a Pokémon game or ROM hack")
        .add_filter(
            "Game Boy / Game Boy Color / GBA / Nintendo DS",
            &["gb", "gbc", "gba", "nds"],
        )
        .pick_file()
}
fn game_from_path(path: PathBuf) -> Game {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let system = match ext.as_str() {
        "gba" => "GBA",
        "nds" => "DS",
        "gb" => "GB",
        "gbc" => "GBC",
        _ => "ROM",
    };
    let title = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(['_', '-'], " ");
    Game {
        path,
        title,
        system,
    }
}
fn save_battery(core: &mut LibretroCore, rom: &Path, dir: &Path) {
    if let Ok(data) = core.read_save_ram() {
        let path = dir.join(format!(
            "{}.sav",
            rom.file_name().unwrap_or_default().to_string_lossy()
        ));
        let _ = fs::write(path, data);
    }
}

fn state_path(rom: &Path, dir: &Path) -> PathBuf {
    dir.join(format!(
        "{}.ss",
        rom.file_name().unwrap_or_default().to_string_lossy()
    ))
}

#[derive(Clone, Copy)]
struct GridLayout {
    left: i32,
    top: i32,
    card_w: i32,
    card_h: i32,
    gap: i32,
    columns: usize,
    rows: usize,
}

impl GridLayout {
    fn index_at(self, x: i32, y: i32, selected: usize) -> Option<usize> {
        let col = (x - self.left) / (self.card_w + self.gap);
        let row = (y - self.top) / (self.card_h + self.gap);
        if col < 0 || row < 0 || col as usize >= self.columns || row as usize >= self.rows {
            return None;
        }
        if (x - self.left) % (self.card_w + self.gap) >= self.card_w
            || (y - self.top) % (self.card_h + self.gap) >= self.card_h
        {
            return None;
        }
        let page_size = self.columns * self.rows;
        Some((selected / page_size) * page_size + row as usize * self.columns + col as usize)
    }
}

fn library_grid_metrics(w: u32, h: u32) -> GridLayout {
    let left = 202;
    let available = (w as i32 - left - 30).max(260);
    let columns = if available >= 940 {
        3
    } else if available >= 620 {
        2
    } else {
        1
    };
    let gap = 15;
    let card_w = ((available - (columns as i32 - 1) * gap) / columns as i32).max(240);
    let top = 168;
    let card_h = 184;
    let rows = ((h as i32 - top - 62) / (card_h + gap)).max(1) as usize;
    GridLayout {
        left,
        top,
        card_w,
        card_h,
        gap,
        columns,
        rows,
    }
}

fn draw_library(
    c: &mut Canvas<Window>,
    pixel: &Font,
    body: &Font,
    strong: &Font,
    logo: &Texture<'_>,
    games: &[Game],
    selected: usize,
    w: u32,
    h: u32,
    error: Option<&str>,
) -> Result<(), String> {
    c.set_draw_color(BG);
    c.clear();
    draw_sidebar(c, body, logo, h, 0)?;
    text(c, pixel, "GAME LIBRARY", 203, 43, 26, INK);
    text(
        c,
        body,
        "Your collection, ready when you are.",
        204,
        83,
        16,
        MUTED,
    );
    draw_button(
        c,
        strong,
        Rect::new((w as i32) - 198, 34, 166, 48),
        "OPEN A GAME",
    )?;
    c.set_draw_color(BORDER);
    c.draw_line(Point::new(202, 112), Point::new((w as i32) - 32, 112))
        .map_err(|e| e.to_string())?;
    if games.is_empty() {
        let x = 202;
        let y = 145;
        let content_w = (w as i32 - x - 30).max(500);
        let guide_w = if content_w >= 660 { 225 } else { 0 };
        let hero_w = content_w - guide_w - 16;
        let hero_h = if h < 600 { 320 } else { 350 };
        panel(c, Rect::new(x, y, hero_w as u32, hero_h), PAPER)?;
        c.set_draw_color(Color::RGB(231, 242, 230));
        c.fill_rect(Rect::new(x + 1, y + 1, hero_w as u32 - 2, 9))
            .map_err(|e| e.to_string())?;
        draw_logo(c, logo, x + 24, y + 28, 78)?;
        text(c, pixel, "A place for every", x + 126, y + 42, 23, INK);
        text(c, pixel, "adventure.", x + 126, y + 70, 23, INK);
        text(
            c,
            body,
            "Choose a ROM to add it to your library.",
            x + 25,
            y + 143,
            16,
            MUTED,
        );
        text(
            c,
            body,
            "The launcher accepts every GB, GBC, GBA and NDS",
            x + 25,
            y + 172,
            14,
            INK,
        );
        text(
            c,
            body,
            "file, including hacks. PokeDaisy tools follow the",
            x + 25,
            y + 195,
            14,
            INK,
        );
        text(
            c,
            body,
            "game profiles provided by its core.",
            x + 25,
            y + 218,
            14,
            INK,
        );
        draw_button(
            c,
            strong,
            Rect::new(x + 24, y + 262, 176, 46),
            "BROWSE ROMS",
        )?;
        draw_chip(
            c,
            strong,
            x + 218,
            y + 271,
            "GB / GBC / GBA / NDS",
            Color::RGB(235, 241, 229),
        )?;
        if guide_w > 0 {
            let gx = x + hero_w + 16;
            panel(
                c,
                Rect::new(gx, y, guide_w as u32, hero_h),
                Color::RGB(229, 238, 225),
            )?;
            text(c, strong, "GETTING STARTED", gx + 20, y + 27, 14, GREEN);
            for (i, (title, detail)) in [
                ("Add a game", "Browse or drop a ROM."),
                ("Place its core", "Add the matching core."),
                ("Keep your saves", "Saves stay in ./data."),
            ]
            .iter()
            .enumerate()
            {
                let sy = y + 75 + i as i32 * 82;
                c.set_draw_color(if i == 0 { CORAL } else { GREEN });
                c.fill_rect(Rect::new(gx + 20, sy, 30, 30))
                    .map_err(|e| e.to_string())?;
                text(c, strong, &(i + 1).to_string(), gx + 31, sy + 5, 16, PAPER);
                text(c, strong, title, gx + 63, sy + 1, 14, INK);
                text(c, body, detail, gx + 63, sy + 24, 12, MUTED);
            }
        }
        text(
            c,
            body,
            "RADICAL RED  ·  UNBOUND  ·  GAIA  ·  AND MORE",
            202,
            (h as i32) - 34,
            13,
            GREEN,
        );
    } else {
        let layout = library_grid_metrics(w, h);
        text(
            c,
            strong,
            &format!("{} games", games.len()),
            202,
            139,
            15,
            INK,
        );
        text(
            c,
            body,
            "Choose a game. Double-click or press Enter to play.",
            (w as i32) - 440,
            140,
            13,
            MUTED,
        );
        let page_size = layout.columns * layout.rows;
        let start = (selected / page_size) * page_size;
        for i in start..(start + page_size).min(games.len()) {
            let slot = i - start;
            let col = slot % layout.columns;
            let row = slot / layout.columns;
            let x = layout.left + col as i32 * (layout.card_w + layout.gap);
            let y = layout.top + row as i32 * (layout.card_h + layout.gap);
            let active = i == selected;
            let fill = if active {
                Color::RGB(229, 239, 222)
            } else {
                PAPER
            };
            panel(
                c,
                Rect::new(x, y, layout.card_w as u32, layout.card_h as u32),
                fill,
            )?;
            let accent = system_color(games[i].system);
            c.set_draw_color(accent);
            c.fill_rect(Rect::new(x + 1, y + 1, layout.card_w as u32 - 2, 7))
                .map_err(|e| e.to_string())?;
            draw_cartridge(c, x + 18, y + 24, games[i].system)?;
            let title_x = x + 91;
            text(c, strong, &games[i].title, title_x, y + 28, 18, INK);
            text(c, body, games[i].system, title_x, y + 59, 14, MUTED);
            let ext = games[i]
                .path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_uppercase();
            draw_chip(
                c,
                strong,
                title_x,
                y + 91,
                &format!("{ext} ROM"),
                Color::RGB(238, 240, 229),
            )?;
            c.set_draw_color(BORDER);
            c.draw_line(
                Point::new(x + 18, y + 132),
                Point::new(x + layout.card_w - 18, y + 132),
            )
            .map_err(|e| e.to_string())?;
            text(
                c,
                strong,
                if active {
                    "ENTER TO PLAY  →"
                } else {
                    "DOUBLE-CLICK TO PLAY"
                },
                x + 18,
                y + 148,
                12,
                if active { GREEN } else { MUTED },
            );
        }
        text(
            c,
            body,
            "↑ ↓ navigate     F5 rescan     Drop ROM files anywhere",
            202,
            (h as i32) - 28,
            13,
            MUTED,
        );
    }
    if let Some(msg) = error {
        let yy = (h as i32) - 57;
        panel(
            c,
            Rect::new(202, yy, (w - 234).max(300), 34),
            Color::RGB(250, 229, 220),
        )?;
        text(c, strong, msg, 214, yy + 8, 12, Color::RGB(142, 52, 40));
    }
    Ok(())
}

fn draw_sidebar(
    c: &mut Canvas<Window>,
    body: &Font,
    logo: &Texture<'_>,
    h: u32,
    active: u8,
) -> Result<(), String> {
    c.set_draw_color(SIDEBAR);
    c.fill_rect(Rect::new(0, 0, 176, h))
        .map_err(|e| e.to_string())?;
    draw_logo(c, logo, 23, 24, 48)?;
    text(c, &body, "PokeDaisy", 24, 82, 20, PAPER);
    text(c, body, "DESKTOP EDITION", 25, 109, 11, AQUA);
    for (i, label, y) in [(0, "Games", 134), (1, "Tools", 200), (2, "Settings", 266)] {
        if active == i {
            c.set_draw_color(Color::RGB(47, 91, 72));
            c.fill_rect(Rect::new(12, y - 7, 152, 48))
                .map_err(|e| e.to_string())?;
        }
        draw_nav_icon(c, i, 27, y + 7, if active == i { PAPER } else { AQUA })?;
        text(
            c,
            body,
            label,
            58,
            y + 7,
            16,
            if active == i {
                PAPER
            } else {
                Color::RGB(191, 212, 198)
            },
        );
    }
    c.set_draw_color(Color::RGB(60, 91, 76));
    c.draw_line(Point::new(22, 352), Point::new(154, 352))
        .map_err(|e| e.to_string())?;
    text(
        c,
        body,
        "Made for your",
        24,
        378,
        13,
        Color::RGB(191, 212, 198),
    );
    text(
        c,
        body,
        "Pokémon library.",
        24,
        399,
        13,
        Color::RGB(191, 212, 198),
    );
    text(
        c,
        body,
        "v0.1 · WINDOWS",
        24,
        h as i32 - 28,
        11,
        Color::RGB(150, 182, 163),
    );
    Ok(())
}

fn draw_info_page(
    c: &mut Canvas<Window>,
    pixel: &Font,
    body: &Font,
    strong: &Font,
    logo: &Texture<'_>,
    w: u32,
    h: u32,
    settings: bool,
) -> Result<(), String> {
    draw_sidebar(c, body, logo, h, if settings { 2 } else { 1 })?;
    let title = if settings {
        "Desktop settings"
    } else {
        "Companion tools"
    };
    text(c, pixel, title, 203, 45, 26, INK);
    text(
        c,
        body,
        if settings {
            "Controls and storage for this Windows build."
        } else {
            "Your game-aware tools stay beside the game."
        },
        204,
        86,
        15,
        MUTED,
    );
    if settings {
        let rows = [
            ("CORE FOLDER", "./cores or POKEDAISY_CORE_DIR"),
            ("GAME LIBRARY", "./data/roms · F5 rescans this folder"),
            (
                "BATTERY SAVES",
                "./data/saves · saved as standard .sav files",
            ),
            ("SAVE STATES", "./data/states · F2 saves, F4 loads"),
            ("CONTROLS", "Keyboard and SDL GameController gamepads"),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let y = 137 + i as i32 * 74;
            panel(
                c,
                Rect::new(202, y, (w.saturating_sub(234)).max(400), 60),
                PAPER,
            )?;
            text(c, strong, label, 220, y + 11, 13, GREEN);
            text(c, body, value, 388, y + 11, 15, INK);
        }
    } else {
        let rows = [
            (
                "Party & boxes",
                "Party details, summaries, deposit and withdraw",
            ),
            ("Bag & items", "Game-aware inventory and item actions"),
            ("Map & flight", "Region maps and searchable places"),
            ("Pokédex & guide", "Game data, moves and guides"),
            ("Battle HUD", "Battle information and quick actions"),
            ("Quick menu", "Fast access to companion actions"),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let y = 143 + i as i32 * 67;
            panel(
                c,
                Rect::new(202, y, (w.saturating_sub(234)).max(400), 54),
                if i % 2 == 0 {
                    PAPER
                } else {
                    Color::RGB(246, 245, 231)
                },
            )?;
            text(c, strong, label, 220, y + 8, 14, INK);
            text(c, body, value, 430, y + 8, 14, MUTED);
        }
        text(
            c,
            body,
            "Start a game, then press Tab for the companion or F for the quick menu.",
            202,
            h as i32 - 31,
            13,
            GREEN,
        );
    }
    Ok(())
}

fn draw_logo(
    c: &mut Canvas<Window>,
    logo: &Texture<'_>,
    x: i32,
    y: i32,
    size: u32,
) -> Result<(), String> {
    c.copy(logo, None, Rect::new(x, y, size, size))
        .map_err(|e| e.to_string())
}
fn draw_nav_icon(
    c: &mut Canvas<Window>,
    which: u8,
    x: i32,
    y: i32,
    color: Color,
) -> Result<(), String> {
    c.set_draw_color(color);
    match which {
        0 => {
            c.draw_rect(Rect::new(x, y, 22, 19))
                .map_err(|e| e.to_string())?;
            c.draw_line(Point::new(x + 4, y + 6), Point::new(x + 18, y + 6))
                .map_err(|e| e.to_string())?;
            c.draw_line(Point::new(x + 4, y + 12), Point::new(x + 18, y + 12))
                .map_err(|e| e.to_string())?;
        }
        1 => {
            c.draw_rect(Rect::new(x, y, 9, 9))
                .map_err(|e| e.to_string())?;
            c.draw_rect(Rect::new(x + 13, y + 10, 9, 9))
                .map_err(|e| e.to_string())?;
            c.draw_line(Point::new(x + 10, y + 5), Point::new(x + 20, y + 15))
                .map_err(|e| e.to_string())?;
        }
        _ => {
            for yy in [y, y + 8, y + 16] {
                c.draw_line(Point::new(x, yy), Point::new(x + 22, yy))
                    .map_err(|e| e.to_string())?;
                c.fill_rect(Rect::new(x + 5, yy - 2, 4, 4))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
fn draw_button(c: &mut Canvas<Window>, font: &Font, r: Rect, label: &str) -> Result<(), String> {
    c.set_draw_color(GREEN);
    c.fill_rect(r).map_err(|e| e.to_string())?;
    text(c, font, label, r.x() + 16, r.y() + 13, 14, PAPER);
    Ok(())
}
fn draw_chip(
    c: &mut Canvas<Window>,
    font: &Font,
    x: i32,
    y: i32,
    label: &str,
    color: Color,
) -> Result<(), String> {
    let width = (label.chars().count() as u32 * 8 + 22).max(58);
    c.set_draw_color(color);
    c.fill_rect(Rect::new(x, y, width, 28))
        .map_err(|e| e.to_string())?;
    text(c, font, label, x + 11, y + 6, 11, GREEN);
    Ok(())
}
fn draw_cartridge(c: &mut Canvas<Window>, x: i32, y: i32, system: &str) -> Result<(), String> {
    let color = system_color(system);
    c.set_draw_color(Color::RGB(49, 64, 58));
    c.fill_rect(Rect::new(x, y + 4, 54, 62))
        .map_err(|e| e.to_string())?;
    c.set_draw_color(Color::RGB(223, 226, 211));
    c.fill_rect(Rect::new(x + 4, y, 46, 59))
        .map_err(|e| e.to_string())?;
    c.set_draw_color(color);
    c.fill_rect(Rect::new(x + 9, y + 8, 36, 24))
        .map_err(|e| e.to_string())?;
    c.set_draw_color(Color::RGB(255, 255, 248));
    c.fill_rect(Rect::new(x + 14, y + 14, 26, 3))
        .map_err(|e| e.to_string())?;
    c.set_draw_color(Color::RGB(178, 186, 169));
    c.fill_rect(Rect::new(x + 8, y + 43, 38, 3))
        .map_err(|e| e.to_string())?;
    c.fill_rect(Rect::new(x + 8, y + 50, 30, 3))
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn system_color(system: &str) -> Color {
    match system {
        "GBA" => Color::RGB(118, 89, 175),
        "DS" => Color::RGB(62, 125, 177),
        "GBC" => Color::RGB(210, 157, 55),
        "GB" => Color::RGB(83, 140, 105),
        _ => CORAL,
    }
}
fn panel(c: &mut Canvas<Window>, r: Rect, color: Color) -> Result<(), String> {
    c.set_draw_color(color);
    c.fill_rect(r).map_err(|e| e.to_string())?;
    c.set_draw_color(BORDER);
    c.draw_rect(r).map_err(|e| e.to_string())?;
    Ok(())
}
fn draw_play_header(c: &mut Canvas<Window>, f: &Font, w: u32) -> Result<(), String> {
    c.set_draw_color(Color::RGBA(24, 37, 39, 220));
    c.fill_rect(Rect::new(0, 0, w, 48))
        .map_err(|e| e.to_string())?;
    text(c, f, "POKEDAISY", 20, 16, 13, PAPER);
    text(
        c,
        f,
        "ESC  LIBRARY      F11  FULLSCREEN",
        (w as i32) - 325,
        16,
        11,
        AQUA,
    );
    Ok(())
}
fn draw_play(
    c: &mut Canvas<Window>,
    tc: &TextureCreator<WindowContext>,
    frame: &VideoFrame,
    w: u32,
    h: u32,
) -> Result<(), String> {
    if frame.width == 0 || frame.height == 0 {
        return Ok(());
    };
    let mut rgba = vec![0u8; (frame.width * frame.height * 4) as usize];
    for y in 0..frame.height as usize {
        for x in 0..frame.width as usize {
            let src = y * frame.pitch + x * frame.format.bytes_per_pixel();
            let dst = (y * frame.width as usize + x) * 4;
            if src + frame.format.bytes_per_pixel() > frame.pixels.len() {
                continue;
            }
            let (r, g, b) = match frame.format {
                PixelFormat::Xrgb8888 => (
                    frame.pixels[src + 2],
                    frame.pixels[src + 1],
                    frame.pixels[src],
                ),
                PixelFormat::Rgb565 => {
                    let p = u16::from_le_bytes([frame.pixels[src], frame.pixels[src + 1]]);
                    (
                        ((p >> 11 & 31) * 255 / 31) as u8,
                        (((p >> 5 & 63) * 255 / 63) as u8),
                        ((p & 31) * 255 / 31) as u8,
                    )
                }
                PixelFormat::Rgb1555 => {
                    let p = u16::from_le_bytes([frame.pixels[src], frame.pixels[src + 1]]);
                    (
                        ((p >> 10 & 31) * 255 / 31) as u8,
                        (((p >> 5 & 31) * 255 / 31) as u8),
                        ((p & 31) * 255 / 31) as u8,
                    )
                }
            };
            rgba[dst..dst + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    let mut tex = tc
        .create_texture_streaming(
            sdl2::pixels::PixelFormatEnum::RGBA32,
            frame.width,
            frame.height,
        )
        .map_err(|e| e.to_string())?;
    tex.update(None, &rgba, frame.width as usize * 4)
        .map_err(|e| e.to_string())?;
    let margin = 24;
    let availw = w.saturating_sub(margin * 2);
    let availh = h.saturating_sub(72);
    let scale = (availw as f32 / frame.width as f32).min(availh as f32 / frame.height as f32);
    let dw = (frame.width as f32 * scale).round().max(1.0) as u32;
    let dh = (frame.height as f32 * scale).round().max(1.0) as u32;
    let x = (w.saturating_sub(dw) / 2) as i32;
    let y = (48 + availh.saturating_sub(dh) / 2) as i32;
    c.set_draw_color(Color::RGB(14, 24, 22));
    c.fill_rect(Rect::new(0, 48, w, h - 48))
        .map_err(|e| e.to_string())?;
    c.copy(&tex, None, Rect::new(x, y, dw, dh))
        .map_err(|e| e.to_string())?;
    c.set_draw_color(AQUA);
    c.draw_rect(Rect::new(x - 5, y - 5, dw + 10, dh + 10))
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn text(c: &mut Canvas<Window>, font: &Font, s: &str, x: i32, y: i32, size: u32, color: Color) {
    let size_px = size as f32;
    let mut pen = x;
    for ch in s.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size_px);
        if metrics.width > 0 && metrics.height > 0 {
            c.set_draw_color(color);
            for by in 0..metrics.height {
                for bx in 0..metrics.width {
                    if bitmap[by * metrics.width + bx] > 110 {
                        let _ = c.draw_point(Point::new(
                            pen + metrics.xmin + bx as i32,
                            y + size as i32 - metrics.height as i32 - metrics.ymin + by as i32,
                        ));
                    }
                }
            }
        }
        pen += metrics.advance_width.round() as i32;
    }
}
