use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettingsOverride {
    pub smooth: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema: u32,
    pub reduced_motion: bool,
    pub library: PathBuf,
    pub saves: PathBuf,
    pub states: PathBuf,
    pub system: PathBuf,
    pub cores: [Option<PathBuf>; 3],
    pub recursive: bool,
    pub volume: u8,
    pub smooth: bool,
    pub fullscreen: bool,
    pub fast_forward: u32,
    pub autosave: u64,
    pub keys: Vec<String>,
    pub library_view: String,
    pub library_filter: String,
    pub library_selection: Option<String>,
    pub game_overrides: BTreeMap<String, GameSettingsOverride>,
    pub system_overrides: BTreeMap<String, GameSettingsOverride>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_survive_restart_and_preserve_previous_configuration() {
        let dir = std::env::temp_dir().join(format!("vyntra-config-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut settings = Settings::load(&path, &dir).unwrap();
        settings.library = dir.join("Pokémon y juegos");
        settings.volume = 30;
        settings.save(&path).unwrap();
        settings.volume = 50;
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path, &dir).unwrap();
        assert_eq!(loaded.volume, 50);
        assert_eq!(loaded.library, dir.join("Pokémon y juegos"));
        let backup = Settings::load(&path.with_extension("json.bak"), &dir).unwrap();
        assert_eq!(backup.volume, 30);
        fs::write(&path, b"{\"schema\":99}").unwrap();
        assert!(Settings::load(&path, &dir).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{\"schema\":99}");
        fs::remove_file(&path).unwrap();
        fs::remove_file(path.with_extension("json.bak")).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn library_view_filter_and_stable_selection_survive_restart() {
        let dir = std::env::temp_dir().join(format!("vyntra-library-view-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut settings = Settings::load(&path, &dir).unwrap();
        settings.library_view = "list".into();
        settings.library_filter = "Favoritos".into();
        settings.library_selection = Some("content-id-123".into());
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path, &dir).unwrap();
        assert_eq!(loaded.library_view, "list");
        assert_eq!(loaded.library_filter, "Favoritos");
        assert_eq!(loaded.library_selection.as_deref(), Some("content-id-123"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn per_game_smoothing_override_survives_restart_and_can_inherit_global() {
        let dir = std::env::temp_dir().join(format!("vyntra-game-settings-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut settings = Settings::load(&path, &dir).unwrap();
        settings.smooth = true;
        settings
            .set_game_smooth("a1b2c3d4e5f60718", Some(false))
            .unwrap();
        assert!(!settings.smooth_for_game("a1b2c3d4e5f60718", "GBA"));
        assert!(settings.smooth_for_game("another-game", "GBA"));
        settings.save(&path).unwrap();

        let mut loaded = Settings::load(&path, &dir).unwrap();
        assert!(!loaded.smooth_for_game("a1b2c3d4e5f60718", "GBA"));
        assert!(loaded.smooth_for_game("another-game", "GBA"));
        loaded.set_game_smooth("a1b2c3d4e5f60718", None).unwrap();
        assert!(loaded.smooth_for_game("a1b2c3d4e5f60718", "GBA"));
        assert!(loaded.set_game_smooth("bad id", Some(false)).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn per_system_smoothing_is_fallback_between_game_and_global() {
        let mut settings = Settings::default();
        settings.smooth = true;
        settings.set_system_smooth("GBA", Some(false)).unwrap();
        assert!(!settings.smooth_for_game("game-one", "GBA"));
        assert!(settings.smooth_for_game("game-two", "GB"));
        settings.set_game_smooth("game-one", Some(true)).unwrap();
        assert!(settings.smooth_for_game("game-one", "GBA"));
        settings.set_game_smooth("game-one", None).unwrap();
        assert!(!settings.smooth_for_game("game-one", "GBA"));
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(!restored.smooth_for_game("unscoped-game", "GBA"));
        assert!(settings.set_system_smooth("unknown", Some(false)).is_err());
    }

    #[test]
    fn section_reset_changes_only_its_fields_and_preserves_library_files() {
        let root =
            std::env::temp_dir().join(format!("vyntra-reset-section-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let keep = root.join("keep.sav");
        fs::write(&keep, b"save").unwrap();
        let mut settings = Settings::default();
        settings.reduced_motion = true;
        settings.volume = 25;
        settings.library = root.join("custom-roms");

        settings.reset_section("general", &root).unwrap();
        assert!(!settings.reduced_motion);
        assert_eq!(settings.volume, 25);
        assert_eq!(settings.library, root.join("custom-roms"));
        settings.reset_section("av", &root).unwrap();
        assert_eq!(settings.volume, 80);
        assert_eq!(settings.library, root.join("custom-roms"));
        assert!(settings.reset_section("unknown", &root).is_err());
        assert_eq!(fs::read(&keep).unwrap(), b"save");
        let _ = fs::remove_dir_all(root);
    }
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 1,
            reduced_motion: false,
            library: "data/roms".into(),
            saves: "data/saves".into(),
            states: "data/states".into(),
            system: "data/system".into(),
            cores: [None, None, None],
            recursive: true,
            volume: 80,
            smooth: true,
            fullscreen: false,
            fast_forward: 2,
            autosave: 20,
            library_view: "grid".into(),
            library_filter: "Todos".into(),
            library_selection: None,
            game_overrides: BTreeMap::new(),
            system_overrides: BTreeMap::new(),
            keys: [
                "Z",
                "X",
                "F",
                "Tab",
                "Return",
                "Backspace",
                "Up",
                "Down",
                "Left",
                "Right",
                "Q",
                "E",
                "1",
                "2",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}
impl Settings {
    pub fn reset_section(&mut self, section: &str, root: &Path) -> Result<(), String> {
        let defaults = Self::default();
        match section {
            "general" => self.reduced_motion = defaults.reduced_motion,
            "folders" => {
                self.library = root.join(defaults.library);
                self.saves = root.join(defaults.saves);
                self.states = root.join(defaults.states);
                self.system = root.join(defaults.system);
                self.recursive = defaults.recursive;
            }
            "cores" => self.cores = defaults.cores,
            "av" => {
                self.volume = defaults.volume;
                self.smooth = defaults.smooth;
                self.fullscreen = defaults.fullscreen;
                self.fast_forward = defaults.fast_forward;
                self.autosave = defaults.autosave;
            }
            "input" => self.keys = defaults.keys,
            _ => return Err("Sección de configuración no válida".into()),
        }
        Ok(())
    }

    pub fn load(path: &Path, root: &Path) -> Result<Self, String> {
        let mut value = match fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes)
                .map_err(|e| format!("Configuración inválida; se conserva el archivo: {e}"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.to_string()),
        };
        if value.schema != 1 {
            return Err("Versión de configuración no compatible".into());
        }
        if value.keys.len() != 14 || value.keys.iter().any(|k| Keycode::from_name(k).is_none()) {
            return Err("Asignación de teclado inválida".into());
        }
        value.volume = value.volume.min(100);
        value.fast_forward = value.fast_forward.clamp(2, 4);
        value.autosave = value.autosave.clamp(10, 120);
        if !matches!(value.library_view.as_str(), "grid" | "list") {
            value.library_view = "grid".into();
        }
        if !matches!(
            value.library_filter.as_str(),
            "Todos" | "Favoritos" | "GB" | "GBC" | "GBA" | "DS"
        ) {
            value.library_filter = "Todos".into();
        }
        if value.library_selection.as_ref().is_some_and(|id| {
            id.len() > 64
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        }) {
            value.library_selection = None;
        }
        if value.game_overrides.len() > 10_000 {
            return Err("Demasiados perfiles por juego en la configuración".into());
        }
        value.game_overrides.retain(|id, profile| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
                && profile.smooth.is_some()
        });
        value.system_overrides.retain(|system, profile| {
            matches!(system.as_str(), "GB" | "GBC" | "GBA" | "DS") && profile.smooth.is_some()
        });
        for dir in [
            &mut value.library,
            &mut value.saves,
            &mut value.states,
            &mut value.system,
        ] {
            if dir.is_relative() {
                *dir = root.join(&*dir);
            }
        }
        Ok(value)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        use std::io::Write;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let temp = path.with_extension("tmp");
        let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        // Preserve the last valid configuration. Never touch saves or ROMs.
        if path.exists() {
            fs::copy(path, path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        }
        fs::rename(&temp, path).map_err(|e| e.to_string())
    }
    pub fn core(&self, system: &str) -> Result<PathBuf, String> {
        let i = match system {
            "GBA" => 1,
            "DS" => 2,
            _ => 0,
        };
        self.cores[i]
            .clone()
            .map(Ok)
            .unwrap_or_else(|| find_core(system))
    }
    pub fn smooth_for_game(&self, storage_id: &str, system: &str) -> bool {
        self.game_overrides
            .get(storage_id)
            .and_then(|profile| profile.smooth)
            .or_else(|| {
                self.system_overrides
                    .get(system)
                    .and_then(|profile| profile.smooth)
            })
            .unwrap_or(self.smooth)
    }

    pub fn set_system_smooth(&mut self, system: &str, smooth: Option<bool>) -> Result<(), String> {
        if !matches!(system, "GB" | "GBC" | "GBA" | "DS") {
            return Err("Sistema no válido para el perfil".into());
        }
        match smooth {
            Some(smooth) => {
                self.system_overrides.insert(
                    system.into(),
                    GameSettingsOverride {
                        smooth: Some(smooth),
                    },
                );
            }
            None => {
                self.system_overrides.remove(system);
            }
        }
        Ok(())
    }
    pub fn set_game_smooth(&mut self, storage_id: &str, value: Option<bool>) -> Result<(), String> {
        if storage_id.is_empty()
            || storage_id.len() > 64
            || !storage_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        {
            return Err("Identificador de juego inválido".into());
        }
        if let Some(smooth) = value {
            if !self.game_overrides.contains_key(storage_id) && self.game_overrides.len() >= 10_000
            {
                return Err("Se alcanzó el límite de perfiles por juego".into());
            }
            self.game_overrides.insert(
                storage_id.to_owned(),
                GameSettingsOverride {
                    smooth: Some(smooth),
                },
            );
        } else {
            self.game_overrides.remove(storage_id);
        }
        Ok(())
    }
    pub fn rows(&self, tab: usize) -> Vec<(String, String)> {
        let row = |label: &str, value: String| (label.to_owned(), value);
        match tab {
            0 => vec![
                row(
                    "Biblioteca · elegir carpeta",
                    self.library.display().to_string(),
                ),
                row(
                    "Buscar en subcarpetas",
                    if self.recursive { "Sí" } else { "No" }.into(),
                ),
                row(
                    "Guardados del juego · elegir carpeta",
                    self.saves.display().to_string(),
                ),
                row(
                    "Estados · elegir carpeta",
                    self.states.display().to_string(),
                ),
                row(
                    "BIOS / sistema · elegir carpeta",
                    self.system.display().to_string(),
                ),
                row(
                    "Abrir carpeta de configuración",
                    "Preferencias JSON y copia anterior".into(),
                ),
            ],
            1 => {
                let mut rows: Vec<_> = ["GB / GBC", "GBA", "Nintendo DS"]
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        row(
                            &format!("{name} · elegir núcleo"),
                            self.cores[i]
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| {
                                    "Descubrimiento automático · carga validada al jugar".into()
                                }),
                        )
                    })
                    .collect();
                rows.push(row(
                    "Restaurar descubrimiento automático",
                    "Conserva los archivos de núcleos".into(),
                ));
                rows
            }
            2 => vec![
                row(
                    "Volumen · pasos de 10 %",
                    format!("{} % (0 = silencio)", self.volume),
                ),
                row(
                    "Filtro de vídeo",
                    if self.smooth {
                        "Suavizado lineal"
                    } else {
                        "Píxeles nítidos"
                    }
                    .into(),
                ),
                row(
                    "Pantalla completa",
                    if self.fullscreen {
                        "Activada"
                    } else {
                        "Desactivada"
                    }
                    .into(),
                ),
                row(
                    "Avance rápido · mantener Espacio",
                    format!("{}×", self.fast_forward),
                ),
                row(
                    "Guardado automático del juego",
                    format!("Cada {} segundos", self.autosave),
                ),
            ],
            _ => {
                let mut rows: Vec<_> = [
                    "A",
                    "B",
                    "Y",
                    "X",
                    "Start",
                    "Select",
                    "Arriba",
                    "Abajo",
                    "Izquierda",
                    "Derecha",
                    "L",
                    "R",
                    "L2",
                    "R2",
                ]
                .iter()
                .enumerate()
                .map(|(i, label)| row(label, self.keys[i].clone()))
                .collect();
                rows.push(row(
                    "Restaurar teclado",
                    "Recupera las teclas predeterminadas".into(),
                ));
                rows
            }
        }
    }
    pub fn activate(
        &mut self,
        tab: usize,
        row: usize,
        direction: i32,
        config: &Path,
    ) -> Result<bool, String> {
        match (tab, row) {
            (0, 0 | 2 | 3 | 4) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Selecciona una carpeta · los archivos anteriores se conservan")
                    .pick_folder()
                {
                    if !path.is_dir() {
                        return Err("La carpeta no existe".into());
                    }
                    *match row {
                        0 => &mut self.library,
                        2 => &mut self.saves,
                        3 => &mut self.states,
                        _ => &mut self.system,
                    } = path;
                } else {
                    return Ok(false);
                }
            }
            (0, 1) => self.recursive = !self.recursive,
            (0, 5) => {
                #[cfg(target_os = "windows")]
                std::process::Command::new("explorer.exe")
                    .arg(config.parent().unwrap_or(Path::new(".")))
                    .spawn()
                    .map_err(|e| e.to_string())?;
                return Ok(false);
            }
            (1, 0..=2) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Núcleo libretro · el archivo se valida al iniciar")
                    .add_filter("Biblioteca nativa", &["dll", "so", "dylib"])
                    .pick_file()
                {
                    self.cores[row] = Some(path);
                } else {
                    return Ok(false);
                }
            }
            (1, 3) => self.cores = [None, None, None],
            (2, 0) => self.volume = (self.volume as i32 + direction * 10).clamp(0, 100) as u8,
            (2, 1) => self.smooth = !self.smooth,
            (2, 2) => self.fullscreen = !self.fullscreen,
            (2, 3) => {
                self.fast_forward =
                    (self.fast_forward as i32 - 2 + direction).rem_euclid(3) as u32 + 2
            }
            (2, 4) => self.autosave = (self.autosave as i32 + direction * 10).clamp(10, 120) as u64,
            (3, 14) => self.keys = Self::default().keys,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

pub fn draw(
    c: &mut Canvas<Window>,
    body: &Font,
    bold: &Font,
    config: &Settings,
    tab: usize,
    selected: usize,
    w: u32,
    h: u32,
    message: &str,
) -> Result<(), String> {
    draw_sidebar(c, body, h, 2)?;
    text(c, bold, "Configuración", 204, 32, 30, INK);
    text(
        c,
        body,
        "Los cambios se guardan automáticamente en este equipo.",
        204,
        78,
        16,
        MUTED,
    );
    let width = (w.saturating_sub(236) / 4) as i32;
    for (i, label) in ["Carpetas", "Núcleos", "Audio y vídeo", "Teclado"]
        .iter()
        .enumerate()
    {
        let rect = Rect::new(204 + i as i32 * width, 116, width as u32 - 6, 44);
        panel(c, rect, if i == tab { DARK_CRIMSON } else { PAPER })?;
        text(c, bold, label, rect.x() + 12, 128, 16, INK);
    }
    let visible = ((h.saturating_sub(250)) / 72).max(1) as usize;
    let rows = config.rows(tab);
    let start = selected / visible * visible;
    for (i, (label, value)) in rows.iter().enumerate().skip(start).take(visible) {
        let y = 178 + (i - start) as i32 * 72;
        panel(
            c,
            Rect::new(204, y, w.saturating_sub(236), 64),
            if i == selected { DARK_CRIMSON } else { PAPER },
        )?;
        if i == selected {
            c.set_draw_color(GREEN);
            c.fill_rect(Rect::new(204, y, 3, 64))?;
        }
        text(c, bold, label, 220, y + 8, 17, INK);
        text(c, body, value, 220, y + 35, 15, MUTED);
    }
    text(
        c,
        body,
        &format!(
            "{} / {} · Tab: sección · ↑ ↓: opción · Intro: cambiar · ← →: ajustar",
            selected + 1,
            rows.len()
        ),
        204,
        h as i32 - 66,
        14,
        MUTED,
    );
    text(c, body, message, 204, h as i32 - 38, 15, INK);
    Ok(())
}
