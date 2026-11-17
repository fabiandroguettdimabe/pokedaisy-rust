use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanionWindowBounds {
    pub monitor_name: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettingsOverride {
    pub smooth: Option<bool>,
    pub volume: Option<u8>,
    pub fast_forward: Option<u32>,
    pub autosave: Option<u64>,
    pub fullscreen: Option<bool>,
}

impl GameSettingsOverride {
    fn is_empty(&self) -> bool {
        self.smooth.is_none()
            && self.volume.is_none()
            && self.fast_forward.is_none()
            && self.autosave.is_none()
            && self.fullscreen.is_none()
    }

    fn sanitize_values(&mut self) {
        if self.volume.is_some_and(|value| value > 100) {
            self.volume = None;
        }
        if self
            .fast_forward
            .is_some_and(|value| !(2..=4).contains(&value))
        {
            self.fast_forward = None;
        }
        if self
            .autosave
            .is_some_and(|value| !matches!(value, 10 | 20 | 30 | 60 | 120))
        {
            self.autosave = None;
        }
    }
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
    pub companion_window: Option<CompanionWindowBounds>,
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
    fn companion_window_bounds_survive_restart_and_invalid_values_are_discarded() {
        let dir =
            std::env::temp_dir().join(format!("vyntra-companion-bounds-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut settings = Settings::load(&path, &dir).unwrap();
        settings.companion_window = Some(CompanionWindowBounds {
            monitor_name: Some("DISPLAY2".into()),
            x: 24.0,
            y: 48.0,
            width: 960.0,
            height: 720.0,
        });
        settings.save(&path).unwrap();
        assert_eq!(
            Settings::load(&path, &dir).unwrap().companion_window,
            settings.companion_window
        );
        fs::write(&path, br#"{"schema":1,"companion_window":{"monitor_name":"DISPLAY2","x":0.0,"y":0.0,"width":1e99,"height":720.0}}"#).unwrap();
        assert!(
            Settings::load(&path, &dir)
                .unwrap()
                .companion_window
                .is_none()
        );
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
        settings
            .set_game_fullscreen("a1b2c3d4e5f60718", Some(true))
            .unwrap();
        assert!(!settings.smooth_for_game("a1b2c3d4e5f60718", "GBA"));
        assert!(settings.smooth_for_game("another-game", "GBA"));
        settings.save(&path).unwrap();

        let mut loaded = Settings::load(&path, &dir).unwrap();
        assert!(!loaded.smooth_for_game("a1b2c3d4e5f60718", "GBA"));
        assert!(loaded.fullscreen_for_game("a1b2c3d4e5f60718", "GBA"));
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
    fn fullscreen_profile_uses_game_system_then_global_and_can_inherit() {
        let mut settings = Settings::default();
        settings.fullscreen = false;
        settings.set_system_fullscreen("GBA", Some(true)).unwrap();
        assert!(settings.fullscreen_for_game("gba-one", "GBA"));
        assert!(!settings.fullscreen_for_game("other", "DS"));
        settings
            .set_game_fullscreen("gba-one", Some(false))
            .unwrap();
        assert!(!settings.fullscreen_for_game("gba-one", "GBA"));
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(!restored.fullscreen_for_game("gba-one", "GBA"));
        assert!(restored.fullscreen_for_game("another-game", "GBA"));
        settings.set_game_fullscreen("gba-one", None).unwrap();
        assert!(settings.fullscreen_for_game("gba-one", "GBA"));
        settings.set_system_fullscreen("GBA", None).unwrap();
        assert!(!settings.fullscreen_for_game("gba-one", "GBA"));
        assert!(settings.game_overrides.is_empty());
        assert!(settings.system_overrides.is_empty());
        assert!(settings.set_system_fullscreen("ROM", Some(true)).is_err());
    }

    #[test]
    fn load_discards_invalid_profile_values_and_keeps_valid_fields() {
        let dir =
            std::env::temp_dir().join(format!("vyntra-profile-validation-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let raw = serde_json::json!({
            "schema": 1,
            "game_overrides": {
                "game-one": {"volume": 200, "fast_forward": 9, "autosave": 15, "fullscreen": true}
            },
            "system_overrides": {
                "GBA": {"smooth": false, "volume": 45, "autosave": 15, "fullscreen": false}
            }
        });
        fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();

        let settings = Settings::load(&path, &dir).unwrap();
        let game = settings.game_overrides.get("game-one").unwrap();
        assert_eq!(game.volume, None);
        assert_eq!(game.fast_forward, None);
        assert_eq!(game.autosave, None);
        assert_eq!(game.fullscreen, Some(true));
        let system = settings.system_overrides.get("GBA").unwrap();
        assert_eq!(system.smooth, Some(false));
        assert_eq!(system.volume, Some(45));
        assert_eq!(system.autosave, None);
        assert_eq!(system.fullscreen, Some(false));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn per_game_volume_override_uses_profile_then_global_and_persists() {
        let mut settings = Settings::default();
        settings.volume = 70;
        settings.set_game_volume("game-one", Some(25)).unwrap();
        settings.set_system_volume("GBA", Some(45)).unwrap();
        assert_eq!(settings.volume_for_game("game-one", "GBA"), 25);
        assert_eq!(settings.volume_for_game("other", "GBA"), 45);
        assert_eq!(settings.volume_for_game("other", "DS"), 70);
        settings.set_game_volume("game-one", None).unwrap();
        assert_eq!(settings.volume_for_game("game-one", "GBA"), 45);
    }

    #[test]
    fn per_game_fast_forward_uses_game_system_then_global() {
        let mut settings = Settings::default();
        settings.fast_forward = 3;
        settings.set_system_fast_forward("DS", Some(4)).unwrap();
        settings.set_game_fast_forward("ds-game", Some(2)).unwrap();
        assert_eq!(settings.fast_forward_for_game("ds-game", "DS"), 2);
        assert_eq!(settings.fast_forward_for_game("other", "DS"), 4);
        assert_eq!(settings.fast_forward_for_game("other", "GBA"), 3);
        settings.set_game_fast_forward("ds-game", None).unwrap();
        assert_eq!(settings.fast_forward_for_game("ds-game", "DS"), 4);
        assert!(settings.set_system_fast_forward("DS", Some(8)).is_err());
    }

    #[test]
    fn autosave_interval_uses_game_system_then_global_and_rejects_unsupported_values() {
        let mut settings = Settings::default();
        settings.autosave = 20;
        settings.set_system_autosave("GB", Some(60)).unwrap();
        settings.set_game_autosave("gb-game", Some(10)).unwrap();
        assert_eq!(settings.autosave_for_game("gb-game", "GB"), 10);
        assert_eq!(settings.autosave_for_game("other", "GB"), 60);
        assert_eq!(settings.autosave_for_game("other", "DS"), 20);
        settings.set_game_autosave("gb-game", None).unwrap();
        assert_eq!(settings.autosave_for_game("gb-game", "GB"), 60);
        assert!(settings.set_system_autosave("GB", Some(15)).is_err());
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
        settings
            .set_game_fullscreen("game-one", Some(true))
            .unwrap();
        settings.set_system_fullscreen("GBA", Some(true)).unwrap();

        settings.reset_section("general", &root).unwrap();
        assert!(!settings.reduced_motion);
        assert_eq!(settings.volume, 25);
        assert_eq!(settings.library, root.join("custom-roms"));
        settings.reset_section("av", &root).unwrap();
        assert_eq!(settings.volume, 80);
        assert!(settings.game_overrides.is_empty());
        assert!(settings.system_overrides.is_empty());
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
            companion_window: None,
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
                self.game_overrides.clear();
                self.system_overrides.clear();
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
        for profile in value.game_overrides.values_mut() {
            profile.sanitize_values();
        }
        for profile in value.system_overrides.values_mut() {
            profile.sanitize_values();
        }
        value.game_overrides.retain(|id, profile| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
                && (profile.smooth.is_some()
                    || profile.volume.is_some()
                    || profile.fast_forward.is_some()
                    || profile.autosave.is_some()
                    || profile.fullscreen.is_some())
        });
        value.system_overrides.retain(|system, profile| {
            matches!(system.as_str(), "GB" | "GBC" | "GBA" | "DS")
                && (profile.smooth.is_some()
                    || profile.volume.is_some()
                    || profile.fast_forward.is_some()
                    || profile.autosave.is_some()
                    || profile.fullscreen.is_some())
        });
        if value.companion_window.as_ref().is_some_and(|bounds| {
            !bounds.x.is_finite()
                || !bounds.y.is_finite()
                || bounds.x.abs() > 100_000.0
                || bounds.y.abs() > 100_000.0
                || !bounds.width.is_finite()
                || !bounds.height.is_finite()
                || !(300.0..=4096.0).contains(&bounds.width)
                || !(240.0..=4096.0).contains(&bounds.height)
                || bounds
                    .monitor_name
                    .as_ref()
                    .is_some_and(|name| name.len() > 128 || name.chars().any(char::is_control))
        }) {
            value.companion_window = None;
        }
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

    pub fn fullscreen_for_game(&self, storage_id: &str, system: &str) -> bool {
        self.game_overrides
            .get(storage_id)
            .and_then(|profile| profile.fullscreen)
            .or_else(|| {
                self.system_overrides
                    .get(system)
                    .and_then(|profile| profile.fullscreen)
            })
            .unwrap_or(self.fullscreen)
    }

    pub fn set_system_fullscreen(
        &mut self,
        system: &str,
        value: Option<bool>,
    ) -> Result<(), String> {
        if !matches!(system, "GB" | "GBC" | "GBA" | "DS") {
            return Err("Sistema no válido para el perfil".into());
        }
        let profile = self.system_overrides.entry(system.into()).or_default();
        profile.fullscreen = value;
        if profile.is_empty() {
            self.system_overrides.remove(system);
        }
        Ok(())
    }

    pub fn set_game_fullscreen(
        &mut self,
        storage_id: &str,
        value: Option<bool>,
    ) -> Result<(), String> {
        self.validate_game_profile_id(storage_id)?;
        if let Some(value) = value {
            if !self.game_overrides.contains_key(storage_id) && self.game_overrides.len() >= 10_000
            {
                return Err("Se alcanzó el límite de perfiles por juego".into());
            }
            self.game_overrides
                .entry(storage_id.to_owned())
                .or_default()
                .fullscreen = Some(value);
        } else if let Some(profile) = self.game_overrides.get_mut(storage_id) {
            profile.fullscreen = None;
            if profile.is_empty() {
                self.game_overrides.remove(storage_id);
            }
        }
        Ok(())
    }

    pub fn set_system_smooth(&mut self, system: &str, smooth: Option<bool>) -> Result<(), String> {
        if !matches!(system, "GB" | "GBC" | "GBA" | "DS") {
            return Err("Sistema no válido para el perfil".into());
        }
        let profile = self.system_overrides.entry(system.into()).or_default();
        profile.smooth = smooth;
        if profile.is_empty() {
            self.system_overrides.remove(system);
        }
        Ok(())
    }
    pub fn set_game_smooth(&mut self, storage_id: &str, value: Option<bool>) -> Result<(), String> {
        self.validate_game_profile_id(storage_id)?;
        if let Some(smooth) = value {
            if !self.game_overrides.contains_key(storage_id) && self.game_overrides.len() >= 10_000
            {
                return Err("Se alcanzó el límite de perfiles por juego".into());
            }
            self.game_overrides
                .entry(storage_id.to_owned())
                .or_default()
                .smooth = Some(smooth);
        } else {
            if let Some(profile) = self.game_overrides.get_mut(storage_id) {
                profile.smooth = None;
                if profile.is_empty() {
                    self.game_overrides.remove(storage_id);
                }
            }
        }
        Ok(())
    }
    pub fn volume_for_game(&self, storage_id: &str, system: &str) -> u8 {
        self.game_overrides
            .get(storage_id)
            .and_then(|profile| profile.volume)
            .or_else(|| {
                self.system_overrides
                    .get(system)
                    .and_then(|profile| profile.volume)
            })
            .unwrap_or(self.volume)
    }
    pub fn set_game_volume(&mut self, storage_id: &str, volume: Option<u8>) -> Result<(), String> {
        self.validate_game_profile_id(storage_id)?;
        if volume.is_some_and(|value| value > 100) {
            return Err("Volumen por juego fuera de rango".into());
        }
        if let Some(value) = volume {
            if !self.game_overrides.contains_key(storage_id) && self.game_overrides.len() >= 10_000
            {
                return Err("Se alcanzó el límite de perfiles por juego".into());
            }
            self.game_overrides
                .entry(storage_id.to_owned())
                .or_default()
                .volume = Some(value);
        } else if let Some(profile) = self.game_overrides.get_mut(storage_id) {
            profile.volume = None;
            if profile.is_empty() {
                self.game_overrides.remove(storage_id);
            }
        }
        Ok(())
    }
    pub fn set_system_volume(&mut self, system: &str, volume: Option<u8>) -> Result<(), String> {
        if !matches!(system, "GB" | "GBC" | "GBA" | "DS") {
            return Err("Sistema no válido para el perfil".into());
        }
        if volume.is_some_and(|value| value > 100) {
            return Err("Volumen por sistema fuera de rango".into());
        }
        if let Some(value) = volume {
            self.system_overrides
                .entry(system.to_owned())
                .or_default()
                .volume = Some(value);
        } else if let Some(profile) = self.system_overrides.get_mut(system) {
            profile.volume = None;
            if profile.is_empty() {
                self.system_overrides.remove(system);
            }
        }
        Ok(())
    }
    fn validate_game_profile_id(&self, storage_id: &str) -> Result<(), String> {
        if storage_id.is_empty()
            || storage_id.len() > 64
            || !storage_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        {
            return Err("Identificador de juego inválido".into());
        }
        Ok(())
    }
    pub fn fast_forward_for_game(&self, storage_id: &str, system: &str) -> u32 {
        self.game_overrides
            .get(storage_id)
            .and_then(|profile| profile.fast_forward)
            .or_else(|| {
                self.system_overrides
                    .get(system)
                    .and_then(|profile| profile.fast_forward)
            })
            .unwrap_or(self.fast_forward)
    }
    pub fn set_game_fast_forward(
        &mut self,
        storage_id: &str,
        value: Option<u32>,
    ) -> Result<(), String> {
        self.validate_game_profile_id(storage_id)?;
        if value.is_some_and(|value| !(2..=4).contains(&value)) {
            return Err("Avance rápido por juego inválido".into());
        }
        if let Some(value) = value {
            if !self.game_overrides.contains_key(storage_id) && self.game_overrides.len() >= 10_000
            {
                return Err("Se alcanzó el límite de perfiles por juego".into());
            }
            self.game_overrides
                .entry(storage_id.to_owned())
                .or_default()
                .fast_forward = Some(value);
        } else if let Some(profile) = self.game_overrides.get_mut(storage_id) {
            profile.fast_forward = None;
            if profile.is_empty() {
                self.game_overrides.remove(storage_id);
            }
        }
        Ok(())
    }
    pub fn set_system_fast_forward(
        &mut self,
        system: &str,
        value: Option<u32>,
    ) -> Result<(), String> {
        if !matches!(system, "GB" | "GBC" | "GBA" | "DS") {
            return Err("Sistema no válido para el perfil".into());
        }
        if value.is_some_and(|value| !(2..=4).contains(&value)) {
            return Err("Avance rápido por sistema inválido".into());
        }
        if let Some(value) = value {
            self.system_overrides
                .entry(system.to_owned())
                .or_default()
                .fast_forward = Some(value);
        } else if let Some(profile) = self.system_overrides.get_mut(system) {
            profile.fast_forward = None;
            if profile.is_empty() {
                self.system_overrides.remove(system);
            }
        }
        Ok(())
    }
    pub fn autosave_for_game(&self, storage_id: &str, system: &str) -> u64 {
        self.game_overrides
            .get(storage_id)
            .and_then(|profile| profile.autosave)
            .or_else(|| {
                self.system_overrides
                    .get(system)
                    .and_then(|profile| profile.autosave)
            })
            .unwrap_or(self.autosave)
    }
    pub fn set_game_autosave(
        &mut self,
        storage_id: &str,
        value: Option<u64>,
    ) -> Result<(), String> {
        self.validate_game_profile_id(storage_id)?;
        if value.is_some_and(|value| !matches!(value, 10 | 20 | 30 | 60 | 120)) {
            return Err("Intervalo de autoguardado por juego inválido".into());
        }
        if let Some(value) = value {
            if !self.game_overrides.contains_key(storage_id) && self.game_overrides.len() >= 10_000
            {
                return Err("Se alcanzó el límite de perfiles por juego".into());
            }
            self.game_overrides
                .entry(storage_id.to_owned())
                .or_default()
                .autosave = Some(value);
        } else if let Some(profile) = self.game_overrides.get_mut(storage_id) {
            profile.autosave = None;
            if profile.is_empty() {
                self.game_overrides.remove(storage_id);
            }
        }
        Ok(())
    }
    pub fn set_system_autosave(&mut self, system: &str, value: Option<u64>) -> Result<(), String> {
        if !matches!(system, "GB" | "GBC" | "GBA" | "DS") {
            return Err("Sistema no válido para el perfil".into());
        }
        if value.is_some_and(|value| !matches!(value, 10 | 20 | 30 | 60 | 120)) {
            return Err("Intervalo de autoguardado por sistema inválido".into());
        }
        if let Some(value) = value {
            self.system_overrides
                .entry(system.to_owned())
                .or_default()
                .autosave = Some(value);
        } else if let Some(profile) = self.system_overrides.get_mut(system) {
            profile.autosave = None;
            if profile.is_empty() {
                self.system_overrides.remove(system);
            }
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
