#![deny(unsafe_op_in_unsafe_fn)]

use libloading::Library;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, OnceLock};

const ENV_GET_CAN_DUPE: u32 = 3;
const ENV_GET_SYSTEM_DIRECTORY: u32 = 9;
const ENV_SET_PIXEL_FORMAT: u32 = 10;
const ENV_SET_INPUT_DESCRIPTORS: u32 = 11;
const ENV_GET_VARIABLE: u32 = 15;
const ENV_SET_VARIABLES: u32 = 16;
const ENV_GET_VARIABLE_UPDATE: u32 = 17;
const ENV_GET_SAVE_DIRECTORY: u32 = 31;
const ENV_SET_SYSTEM_AV_INFO: u32 = 32;
const ENV_SET_MEMORY_MAPS: u32 = 32_804;
const ENV_GET_FASTFORWARDING: u32 = 32_817;
const ENV_GET_INPUT_BITMASKS: u32 = 32_819;
const ENV_GET_CORE_OPTIONS_VERSION: u32 = 52;
const ENV_SET_CORE_OPTIONS: u32 = 53;
const ENV_SET_CORE_OPTIONS_INTL: u32 = 54;
const ENV_SET_CORE_OPTIONS_DISPLAY: u32 = 55;
const ENV_SET_CORE_OPTIONS_V2: u32 = 67;
const ENV_SET_CORE_OPTIONS_V2_INTL: u32 = 68;
const ENV_GET_THROTTLE_STATE: u32 = 32_839;
const ENV_SET_GEOMETRY: u32 = 37;

const RETRO_DEVICE_JOYPAD: u32 = 1;
const RETRO_DEVICE_MASK: u32 = 0xff;
const RETRO_DEVICE_ID_JOYPAD_MASK: u32 = 256;
const RETRO_MEMORY_SAVE_RAM: u32 = 0;
const RETRO_PIXEL_FORMAT_0RGB1555: u32 = 0;
const RETRO_PIXEL_FORMAT_XRGB8888: u32 = 1;
const RETRO_PIXEL_FORMAT_RGB565: u32 = 2;
const RETRO_THROTTLE_VSYNC: u32 = 5;
const RETRO_API_VERSION: u32 = 1;
const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;
const MAX_AUDIO_SAMPLES_PER_FRAME: usize = 65_536;
const MAX_OPTION_DEFINITIONS: usize = 2048;
const OPTION_VALUES_MAX: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Rgb1555,
    Xrgb8888,
    Rgb565,
}

impl PixelFormat {
    fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            RETRO_PIXEL_FORMAT_0RGB1555 => Some(Self::Rgb1555),
            RETRO_PIXEL_FORMAT_XRGB8888 => Some(Self::Xrgb8888),
            RETRO_PIXEL_FORMAT_RGB565 => Some(Self::Rgb565),
            _ => None,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgb1555 | Self::Rgb565 => 2,
            Self::Xrgb8888 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum JoypadButton {
    #[default]
    B = 0,
    Y = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
    A = 8,
    X = 9,
    L = 10,
    R = 11,
    L2 = 12,
    R2 = 13,
    L3 = 14,
    R3 = 15,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JoypadState {
    bits: u16,
}

impl JoypadState {
    pub fn set(&mut self, button: JoypadButton, pressed: bool) {
        let mask = 1u16 << button as u32;
        if pressed {
            self.bits |= mask;
        } else {
            self.bits &= !mask;
        }
    }

    pub fn contains(self, button: JoypadButton) -> bool {
        self.bits & (1u16 << button as u32) != 0
    }

    pub fn bits(self) -> u16 {
        self.bits
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub base_width: u32,
    pub base_height: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub aspect_ratio: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SystemAvInfo {
    pub geometry: Geometry,
    pub fps: f64,
    pub sample_rate: f64,
}

fn normalize_av_info(av: RetroAvInfo) -> SystemAvInfo {
    let base_width = av.geometry.base_width.max(1);
    let base_height = av.geometry.base_height.max(1);
    SystemAvInfo {
        geometry: Geometry {
            base_width,
            base_height,
            max_width: av.geometry.max_width.max(base_width),
            max_height: av.geometry.max_height.max(base_height),
            aspect_ratio: if av.geometry.aspect_ratio.is_finite() && av.geometry.aspect_ratio > 0.0
            {
                av.geometry.aspect_ratio
            } else {
                base_width as f32 / base_height as f32
            },
        },
        fps: if av.timing.fps.is_finite() && av.timing.fps > 0.0 {
            av.timing.fps
        } else {
            60.0
        },
        sample_rate: if av.timing.sample_rate.is_finite() && av.timing.sample_rate > 0.0 {
            av.timing.sample_rate.clamp(8_000.0, 192_000.0)
        } else {
            32_768.0
        },
    }
}

fn accepted_audio_frames(current_samples: usize, requested_frames: usize) -> usize {
    let capacity = MAX_AUDIO_SAMPLES_PER_FRAME.saturating_sub(current_samples);
    requested_frames.min(capacity / 2)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreSystemInfo {
    pub library_name: Option<String>,
    pub library_version: Option<String>,
    pub valid_extensions: Option<String>,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

#[derive(Clone, Debug)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pitch: usize,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct CoreFrame {
    pub video: Option<VideoFrame>,
    pub audio: Vec<i16>,
}

#[derive(Clone, Debug)]
pub struct CorePaths {
    pub save_dir: PathBuf,
    pub system_dir: PathBuf,
    pub panel_width: u32,
    pub panel_height: u32,
}

#[derive(Debug)]
pub enum CoreError {
    LoadLibrary { path: PathBuf, message: String },
    MissingSymbol { path: PathBuf, symbol: &'static str },
    UnsupportedApi { path: PathBuf, version: u32 },
    ReadRom { path: PathBuf, message: String },
    InvalidPath(PathBuf),
    GameLoad(PathBuf),
    NoGameLoaded,
    StateUnavailable,
    StateSaveFailed,
    StateLoadFailed,
    SaveRamUnavailable,
    SaveRamSize { expected: usize, actual: usize },
    InvalidFrame,
    ContextBusy,
    PoisonedContext,
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadLibrary { path, message } => {
                write!(f, "Could not load core {}: {message}", path.display())
            }
            Self::MissingSymbol { path, symbol } => {
                write!(f, "Core {} does not export {symbol}", path.display())
            }
            Self::UnsupportedApi { path, version } => write!(
                f,
                "Core {} uses unsupported libretro API {version}",
                path.display()
            ),
            Self::ReadRom { path, message } => {
                write!(f, "Could not read ROM {}: {message}", path.display())
            }
            Self::InvalidPath(path) => write!(
                f,
                "The path is not valid UTF-8 for libretro: {}",
                path.display()
            ),
            Self::GameLoad(path) => write!(f, "The core rejected ROM {}", path.display()),
            Self::NoGameLoaded => f.write_str("No ROM is loaded"),
            Self::StateUnavailable => f.write_str("The core does not support save states"),
            Self::StateSaveFailed => f.write_str("The core could not save its state"),
            Self::StateLoadFailed => f.write_str("The core could not load that state"),
            Self::SaveRamUnavailable => f.write_str("The core does not expose battery save RAM"),
            Self::SaveRamSize { expected, actual } => write!(
                f,
                "Save RAM size mismatch: expected {expected}, got {actual}"
            ),
            Self::InvalidFrame => {
                f.write_str("The core produced a frame with invalid dimensions or pitch")
            }
            Self::ContextBusy => f.write_str("Another libretro core is already active"),
            Self::PoisonedContext => f.write_str("The libretro callback context is unavailable"),
        }
    }
}

impl std::error::Error for CoreError {}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroGameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroVariable {
    key: *const c_char,
    value: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroCoreOptionValue {
    value: *const c_char,
    label: *const c_char,
}

#[repr(C)]
struct RetroCoreOptionDefinition {
    key: *const c_char,
    desc: *const c_char,
    info: *const c_char,
    values: [RetroCoreOptionValue; OPTION_VALUES_MAX],
    default_value: *const c_char,
}

#[repr(C)]
struct RetroCoreOptionV2Definition {
    key: *const c_char,
    desc: *const c_char,
    desc_categorized: *const c_char,
    info: *const c_char,
    info_categorized: *const c_char,
    category_key: *const c_char,
    values: [RetroCoreOptionValue; OPTION_VALUES_MAX],
    default_value: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroCoreOptionsV2 {
    categories: *mut c_void,
    definitions: *mut RetroCoreOptionV2Definition,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroCoreOptionsV2Intl {
    us: *mut RetroCoreOptionsV2,
    local: *mut RetroCoreOptionsV2,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroCoreOptionsIntl {
    us: *mut RetroCoreOptionDefinition,
    local: *mut RetroCoreOptionDefinition,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroGeometry {
    base_width: u32,
    base_height: u32,
    max_width: u32,
    max_height: u32,
    aspect_ratio: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroTiming {
    fps: f64,
    sample_rate: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RetroAvInfo {
    geometry: RetroGeometry,
    timing: RetroTiming,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RetroSystemInfo {
    library_name: *const c_char,
    library_version: *const c_char,
    valid_extensions: *const c_char,
    need_fullpath: bool,
    block_extract: bool,
}

impl Default for RetroSystemInfo {
    fn default() -> Self {
        Self {
            library_name: ptr::null(),
            library_version: ptr::null(),
            valid_extensions: ptr::null(),
            need_fullpath: false,
            block_extract: false,
        }
    }
}

#[repr(C)]
struct RetroThrottleState {
    mode: u32,
    rate: f32,
}

type EnvironmentCallback = unsafe extern "C" fn(u32, *mut c_void) -> bool;
type VideoRefreshCallback = unsafe extern "C" fn(*const c_void, u32, u32, usize);
type AudioSampleCallback = unsafe extern "C" fn(i16, i16);
type AudioBatchCallback = unsafe extern "C" fn(*const i16, usize) -> usize;
type InputPollCallback = unsafe extern "C" fn();
type InputStateCallback = unsafe extern "C" fn(u32, u32, u32, u32) -> i16;

type SetEnvironment = unsafe extern "C" fn(Option<EnvironmentCallback>);
type SetVideoRefresh = unsafe extern "C" fn(Option<VideoRefreshCallback>);
type SetAudioSample = unsafe extern "C" fn(Option<AudioSampleCallback>);
type SetAudioBatch = unsafe extern "C" fn(Option<AudioBatchCallback>);
type SetInputPoll = unsafe extern "C" fn(Option<InputPollCallback>);
type SetInputState = unsafe extern "C" fn(Option<InputStateCallback>);
type ApiVersion = unsafe extern "C" fn() -> u32;
type Init = unsafe extern "C" fn();
type Deinit = unsafe extern "C" fn();
type LoadGame = unsafe extern "C" fn(*const RetroGameInfo) -> bool;
type UnloadGame = unsafe extern "C" fn();
type Run = unsafe extern "C" fn();
type GetAvInfo = unsafe extern "C" fn(*mut RetroAvInfo);
type GetSystemInfo = unsafe extern "C" fn(*mut RetroSystemInfo);
type SerializeSize = unsafe extern "C" fn() -> usize;
type Serialize = unsafe extern "C" fn(*mut c_void, usize) -> bool;
type Unserialize = unsafe extern "C" fn(*const c_void, usize) -> bool;
type GetMemoryData = unsafe extern "C" fn(u32) -> *mut c_void;
type GetMemorySize = unsafe extern "C" fn(u32) -> usize;
type SetController = unsafe extern "C" fn(u32, u32);

struct Api {
    set_environment: SetEnvironment,
    set_video_refresh: SetVideoRefresh,
    set_audio_sample: SetAudioSample,
    set_audio_batch: SetAudioBatch,
    set_input_poll: SetInputPoll,
    set_input_state: SetInputState,
    api_version: ApiVersion,
    init: Init,
    deinit: Deinit,
    load_game: LoadGame,
    unload_game: UnloadGame,
    run: Run,
    get_av_info: GetAvInfo,
    get_system_info: GetSystemInfo,
    serialize_size: SerializeSize,
    serialize: Serialize,
    unserialize: Unserialize,
    get_memory_data: GetMemoryData,
    get_memory_size: GetMemorySize,
    set_controller: SetController,
}

#[derive(Clone)]
struct CallbackState {
    active: bool,
    game_loaded: bool,
    input: JoypadState,
    video: Option<VideoFrame>,
    audio: Vec<i16>,
    options: HashMap<String, CString>,
    panel: CString,
    save_dir: CString,
    system_dir: CString,
    pixel_format: PixelFormat,
    av: RetroAvInfo,
    fast_forward: bool,
}

impl CallbackState {
    fn new(paths: &CorePaths) -> Result<Self, CoreError> {
        let panel = CString::new(format!(
            "{}x{}",
            paths.panel_width.max(320),
            paths.panel_height.max(240)
        ))
        .map_err(|_| CoreError::InvalidPath(PathBuf::from("panel size")))?;
        Ok(Self {
            active: true,
            game_loaded: false,
            input: JoypadState::default(),
            video: None,
            audio: Vec::with_capacity(8192),
            options: HashMap::new(),
            panel,
            save_dir: path_cstring(&paths.save_dir)?,
            system_dir: path_cstring(&paths.system_dir)?,
            pixel_format: PixelFormat::Rgb1555,
            av: RetroAvInfo::default(),
            fast_forward: false,
        })
    }
}

fn callback_state() -> &'static Mutex<Option<CallbackState>> {
    static STATE: OnceLock<Mutex<Option<CallbackState>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

fn path_cstring(path: &Path) -> Result<CString, CoreError> {
    let display = path.to_string_lossy().replace('\\', "/");
    CString::new(display).map_err(|_| CoreError::InvalidPath(path.to_path_buf()))
}

unsafe fn required_symbol<T: Copy>(
    library: &Library,
    path: &Path,
    name: &'static [u8],
    display: &'static str,
) -> Result<T, CoreError> {
    // SAFETY: The libretro ABI requires these exported names and function signatures.
    unsafe { library.get::<T>(name) }
        .map(|symbol| *symbol)
        .map_err(|_| CoreError::MissingSymbol {
            path: path.to_path_buf(),
            symbol: display,
        })
}

impl Api {
    unsafe fn load(library: &Library, path: &Path) -> Result<Self, CoreError> {
        Ok(Self {
            // SAFETY: Each type matches the function's declaration in the pinned libretro.h.
            set_environment: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_set_environment\0",
                    "retro_set_environment",
                )?
            },
            set_video_refresh: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_set_video_refresh\0",
                    "retro_set_video_refresh",
                )?
            },
            set_audio_sample: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_set_audio_sample\0",
                    "retro_set_audio_sample",
                )?
            },
            set_audio_batch: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_set_audio_sample_batch\0",
                    "retro_set_audio_sample_batch",
                )?
            },
            set_input_poll: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_set_input_poll\0",
                    "retro_set_input_poll",
                )?
            },
            set_input_state: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_set_input_state\0",
                    "retro_set_input_state",
                )?
            },
            api_version: unsafe {
                required_symbol(library, path, b"retro_api_version\0", "retro_api_version")?
            },
            init: unsafe { required_symbol(library, path, b"retro_init\0", "retro_init")? },
            deinit: unsafe { required_symbol(library, path, b"retro_deinit\0", "retro_deinit")? },
            load_game: unsafe {
                required_symbol(library, path, b"retro_load_game\0", "retro_load_game")?
            },
            unload_game: unsafe {
                required_symbol(library, path, b"retro_unload_game\0", "retro_unload_game")?
            },
            run: unsafe { required_symbol(library, path, b"retro_run\0", "retro_run")? },
            get_av_info: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_get_system_av_info\0",
                    "retro_get_system_av_info",
                )?
            },
            get_system_info: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_get_system_info\0",
                    "retro_get_system_info",
                )?
            },
            serialize_size: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_serialize_size\0",
                    "retro_serialize_size",
                )?
            },
            serialize: unsafe {
                required_symbol(library, path, b"retro_serialize\0", "retro_serialize")?
            },
            unserialize: unsafe {
                required_symbol(library, path, b"retro_unserialize\0", "retro_unserialize")?
            },
            get_memory_data: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_get_memory_data\0",
                    "retro_get_memory_data",
                )?
            },
            get_memory_size: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_get_memory_size\0",
                    "retro_get_memory_size",
                )?
            },
            set_controller: unsafe {
                required_symbol(
                    library,
                    path,
                    b"retro_set_controller_port_device\0",
                    "retro_set_controller_port_device",
                )?
            },
        })
    }
}

struct LoadedRom {
    _path: CString,
    _bytes: Vec<u8>,
}

pub struct LibretroCore {
    _library: Library,
    api: Api,
    loaded_rom: Option<LoadedRom>,
    av: SystemAvInfo,
}

impl LibretroCore {
    pub fn load(library_path: &Path, paths: CorePaths) -> Result<Self, CoreError> {
        let library =
            unsafe { Library::new(library_path) }.map_err(|error| CoreError::LoadLibrary {
                path: library_path.to_path_buf(),
                message: error.to_string(),
            })?;
        let api = unsafe { Api::load(&library, library_path)? };
        let version = unsafe { (api.api_version)() };
        if version != RETRO_API_VERSION {
            return Err(CoreError::UnsupportedApi {
                path: library_path.to_path_buf(),
                version,
            });
        }
        {
            let mut state = callback_state()
                .lock()
                .map_err(|_| CoreError::PoisonedContext)?;
            if state.as_ref().is_some_and(|existing| existing.active) {
                return Err(CoreError::ContextBusy);
            }
            *state = Some(CallbackState::new(&paths)?);
        }
        unsafe {
            (api.set_environment)(Some(environment));
            (api.init)();
            (api.set_video_refresh)(Some(video_refresh));
            (api.set_audio_sample)(Some(audio_sample));
            (api.set_audio_batch)(Some(audio_batch));
            (api.set_input_poll)(Some(input_poll));
            (api.set_input_state)(Some(input_state));
            (api.set_controller)(0, RETRO_DEVICE_JOYPAD);
        }
        Ok(Self {
            _library: library,
            api,
            loaded_rom: None,
            av: SystemAvInfo {
                geometry: Geometry {
                    base_width: 320,
                    base_height: 240,
                    max_width: 1024,
                    max_height: 768,
                    aspect_ratio: 4.0 / 3.0,
                },
                fps: 60.0,
                sample_rate: 32768.0,
            },
        })
    }

    pub fn load_game(&mut self, rom_path: &Path) -> Result<SystemAvInfo, CoreError> {
        let bytes = fs::read(rom_path).map_err(|error| CoreError::ReadRom {
            path: rom_path.to_path_buf(),
            message: error.to_string(),
        })?;
        let path = path_cstring(rom_path)?;
        let info = RetroGameInfo {
            path: path.as_ptr(),
            data: bytes.as_ptr().cast(),
            size: bytes.len(),
            meta: ptr::null(),
        };
        if !unsafe { (self.api.load_game)(&info) } {
            return Err(CoreError::GameLoad(rom_path.to_path_buf()));
        }
        self.loaded_rom = Some(LoadedRom {
            _path: path,
            _bytes: bytes,
        });
        let mut av = RetroAvInfo::default();
        unsafe { (self.api.get_av_info)(&mut av) };
        if let Ok(mut state) = callback_state().lock() {
            if let Some(state) = state.as_mut() {
                state.game_loaded = true;
                if state.av.geometry.base_width != 0 {
                    av = state.av;
                } else {
                    state.av = av;
                }
            }
        }
        self.av = normalize_av_info(av);
        Ok(self.av)
    }

    pub fn system_info(&self) -> CoreSystemInfo {
        let mut info = RetroSystemInfo::default();
        unsafe { (self.api.get_system_info)(&mut info) };
        CoreSystemInfo {
            library_name: parse_cstr(info.library_name),
            library_version: parse_cstr(info.library_version),
            valid_extensions: parse_cstr(info.valid_extensions),
            need_fullpath: info.need_fullpath,
            block_extract: info.block_extract,
        }
    }

    pub fn api_version(&self) -> u32 {
        unsafe { (self.api.api_version)() }
    }

    pub fn run_frame(&mut self, input: JoypadState) -> Result<CoreFrame, CoreError> {
        if self.loaded_rom.is_none() {
            return Err(CoreError::NoGameLoaded);
        }
        {
            let mut state = callback_state()
                .lock()
                .map_err(|_| CoreError::PoisonedContext)?;
            let state = state.as_mut().ok_or(CoreError::PoisonedContext)?;
            state.input = input;
            state.audio.clear();
        }
        unsafe { (self.api.run)() };
        let mut state = callback_state()
            .lock()
            .map_err(|_| CoreError::PoisonedContext)?;
        let state = state.as_mut().ok_or(CoreError::PoisonedContext)?;
        self.av = normalize_av_info(state.av);
        Ok(CoreFrame {
            video: state.video.clone(),
            audio: std::mem::take(&mut state.audio),
        })
    }

    pub fn av_info(&self) -> SystemAvInfo {
        self.av
    }

    pub fn set_fast_forward(&mut self, enabled: bool) {
        if let Ok(mut state) = callback_state().lock() {
            if let Some(state) = state.as_mut() {
                state.fast_forward = enabled;
            }
        }
    }

    pub fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        if self.loaded_rom.is_none() {
            return Err(CoreError::NoGameLoaded);
        }
        let size = unsafe { (self.api.serialize_size)() };
        if size == 0 || size > MAX_FRAME_BYTES {
            return Err(CoreError::StateUnavailable);
        }
        let mut state = vec![0u8; size];
        if !unsafe { (self.api.serialize)(state.as_mut_ptr().cast(), state.len()) } {
            return Err(CoreError::StateSaveFailed);
        }
        Ok(state)
    }

    pub fn supports_save_states(&self) -> bool {
        let size = unsafe { (self.api.serialize_size)() };
        size > 0 && size <= MAX_FRAME_BYTES
    }

    pub fn unserialize(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        if self.loaded_rom.is_none() {
            return Err(CoreError::NoGameLoaded);
        }
        if !unsafe { (self.api.unserialize)(bytes.as_ptr().cast(), bytes.len()) } {
            return Err(CoreError::StateLoadFailed);
        }
        Ok(())
    }

    pub fn read_save_ram(&mut self) -> Result<Vec<u8>, CoreError> {
        if self.loaded_rom.is_none() {
            return Err(CoreError::NoGameLoaded);
        }
        let ptr = unsafe { (self.api.get_memory_data)(RETRO_MEMORY_SAVE_RAM) }.cast::<u8>();
        let size = unsafe { (self.api.get_memory_size)(RETRO_MEMORY_SAVE_RAM) };
        if ptr.is_null() || size == 0 || size > MAX_FRAME_BYTES {
            return Err(CoreError::SaveRamUnavailable);
        }
        Ok(unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec())
    }

    pub fn write_save_ram(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        if self.loaded_rom.is_none() {
            return Err(CoreError::NoGameLoaded);
        }
        let ptr = unsafe { (self.api.get_memory_data)(RETRO_MEMORY_SAVE_RAM) }.cast::<u8>();
        let size = unsafe { (self.api.get_memory_size)(RETRO_MEMORY_SAVE_RAM) };
        if ptr.is_null() || size == 0 {
            return Err(CoreError::SaveRamUnavailable);
        }
        if bytes.len() != size {
            return Err(CoreError::SaveRamSize {
                expected: size,
                actual: bytes.len(),
            });
        }
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, size) };
        Ok(())
    }
}

impl Drop for LibretroCore {
    fn drop(&mut self) {
        if self.loaded_rom.take().is_some() {
            unsafe { (self.api.unload_game)() };
        }
        unsafe { (self.api.deinit)() };
        if let Ok(mut state) = callback_state().lock() {
            *state = None;
        }
    }
}

fn parse_cstr(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

unsafe fn collect_v2_options(
    definitions: *const RetroCoreOptionV2Definition,
    options: &mut HashMap<String, CString>,
) {
    if definitions.is_null() {
        return;
    }
    for index in 0..MAX_OPTION_DEFINITIONS {
        // SAFETY: The libretro ABI provides a NULL-key-terminated array during this callback.
        let definition = unsafe { &*definitions.add(index) };
        if definition.key.is_null() {
            break;
        }
        if let (Some(key), Some(value)) = (
            parse_cstr(definition.key),
            parse_cstr(definition.default_value),
        ) {
            if let Ok(value) = CString::new(value) {
                options.insert(key, value);
            }
        }
    }
}

unsafe fn collect_v1_options(
    definitions: *const RetroCoreOptionDefinition,
    options: &mut HashMap<String, CString>,
) {
    if definitions.is_null() {
        return;
    }
    for index in 0..MAX_OPTION_DEFINITIONS {
        let definition = unsafe { &*definitions.add(index) };
        if definition.key.is_null() {
            break;
        }
        if let (Some(key), Some(value)) = (
            parse_cstr(definition.key),
            parse_cstr(definition.default_value),
        ) {
            if let Ok(value) = CString::new(value) {
                options.insert(key, value);
            }
        }
    }
}

unsafe fn collect_variables(
    variables: *const RetroVariable,
    options: &mut HashMap<String, CString>,
) {
    if variables.is_null() {
        return;
    }
    for index in 0..MAX_OPTION_DEFINITIONS {
        let variable = unsafe { &*variables.add(index) };
        if variable.key.is_null() {
            break;
        }
        let Some(key) = parse_cstr(variable.key) else {
            continue;
        };
        let Some(definition) = parse_cstr(variable.value) else {
            continue;
        };
        let Some((_, values)) = definition.split_once(';') else {
            continue;
        };
        let default = values.trim().split('|').next().unwrap_or_default().trim();
        if let Ok(value) = CString::new(default) {
            options.insert(key, value);
        }
    }
}

unsafe extern "C" fn environment(command: u32, data: *mut c_void) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        environment_inner(command, data)
    }))
    .unwrap_or(false)
}

fn environment_inner(command: u32, data: *mut c_void) -> bool {
    let Ok(mut guard) = callback_state().lock() else {
        return false;
    };
    let Some(state) = guard.as_mut().filter(|state| state.active) else {
        return false;
    };
    unsafe {
        match command {
            ENV_GET_CAN_DUPE => {
                if !data.is_null() {
                    *(data.cast::<bool>()) = true;
                    true
                } else {
                    false
                }
            }
            ENV_GET_SYSTEM_DIRECTORY => {
                if !data.is_null() {
                    *(data.cast::<*const c_char>()) = state.system_dir.as_ptr();
                    true
                } else {
                    false
                }
            }
            ENV_GET_SAVE_DIRECTORY => {
                if !data.is_null() {
                    *(data.cast::<*const c_char>()) = state.save_dir.as_ptr();
                    true
                } else {
                    false
                }
            }
            ENV_SET_PIXEL_FORMAT => {
                if data.is_null() {
                    return false;
                }
                let raw = *(data.cast::<u32>());
                if let Some(format) = PixelFormat::from_raw(raw) {
                    state.pixel_format = format;
                    true
                } else {
                    false
                }
            }
            ENV_SET_INPUT_DESCRIPTORS | ENV_SET_MEMORY_MAPS | ENV_SET_CORE_OPTIONS_DISPLAY => true,
            ENV_GET_VARIABLE => {
                if data.is_null() {
                    return false;
                }
                let variable = &mut *data.cast::<RetroVariable>();
                variable.value = ptr::null();
                if let Some(key) = parse_cstr(variable.key) {
                    if key == "pokedaisy_panel" {
                        variable.value = state.panel.as_ptr();
                        return true;
                    }
                    if key == "pokedaisy_ds_select_modifier" {
                        variable.value = b"enabled\0".as_ptr().cast();
                        return true;
                    }
                    if let Some(value) = state.options.get(&key) {
                        variable.value = value.as_ptr();
                        return true;
                    }
                }
                false
            }
            ENV_SET_VARIABLES => {
                collect_variables(data.cast(), &mut state.options);
                true
            }
            ENV_GET_VARIABLE_UPDATE => {
                if !data.is_null() {
                    *(data.cast::<bool>()) = false;
                    true
                } else {
                    false
                }
            }
            ENV_SET_SYSTEM_AV_INFO => {
                if data.is_null() {
                    return false;
                }
                state.av = *data.cast::<RetroAvInfo>();
                true
            }
            ENV_SET_GEOMETRY => {
                if data.is_null() {
                    return false;
                }
                state.av.geometry = *data.cast::<RetroGeometry>();
                true
            }
            ENV_GET_FASTFORWARDING => {
                if !data.is_null() {
                    *(data.cast::<bool>()) = state.fast_forward;
                    true
                } else {
                    false
                }
            }
            ENV_GET_INPUT_BITMASKS => true,
            ENV_GET_CORE_OPTIONS_VERSION => {
                if !data.is_null() {
                    *(data.cast::<u32>()) = 2;
                    true
                } else {
                    false
                }
            }
            ENV_SET_CORE_OPTIONS => {
                collect_v1_options(data.cast(), &mut state.options);
                true
            }
            ENV_SET_CORE_OPTIONS_INTL => {
                if data.is_null() {
                    return false;
                }
                let intl = &*data.cast::<RetroCoreOptionsIntl>();
                collect_v1_options(intl.us, &mut state.options);
                true
            }
            ENV_SET_CORE_OPTIONS_V2 => {
                if data.is_null() {
                    return false;
                }
                let options = &*data.cast::<RetroCoreOptionsV2>();
                collect_v2_options(options.definitions, &mut state.options);
                true
            }
            ENV_SET_CORE_OPTIONS_V2_INTL => {
                if data.is_null() {
                    return false;
                }
                let intl = &*data.cast::<RetroCoreOptionsV2Intl>();
                if intl.us.is_null() {
                    return false;
                }
                collect_v2_options((*intl.us).definitions, &mut state.options);
                true
            }
            ENV_GET_THROTTLE_STATE => {
                if data.is_null() {
                    return false;
                }
                let throttle = &mut *data.cast::<RetroThrottleState>();
                throttle.mode = RETRO_THROTTLE_VSYNC;
                throttle.rate = if state.av.timing.fps > 0.0 {
                    state.av.timing.fps as f32
                } else {
                    60.0
                };
                true
            }
            _ => false,
        }
    }
}

unsafe extern "C" fn video_refresh(data: *const c_void, width: u32, height: u32, pitch: usize) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if data.is_null() || width == 0 || height == 0 || width > 8192 || height > 8192 {
            return;
        }
        let Ok(mut guard) = callback_state().lock() else {
            return;
        };
        let Some(state) = guard.as_mut().filter(|state| state.active) else {
            return;
        };
        let row_min = width as usize * state.pixel_format.bytes_per_pixel();
        let total = pitch.saturating_mul(height as usize);
        if pitch < row_min || total > MAX_FRAME_BYTES {
            return;
        }
        let pixels = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), total) }.to_vec();
        state.video = Some(VideoFrame {
            width,
            height,
            pitch,
            format: state.pixel_format,
            pixels,
        });
    }));
}

unsafe extern "C" fn audio_sample(left: i16, right: i16) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Ok(mut guard) = callback_state().lock() {
            if let Some(state) = guard.as_mut().filter(|state| state.active) {
                if accepted_audio_frames(state.audio.len(), 1) == 1 {
                    state.audio.extend([left, right]);
                }
            }
        }
    }));
}

unsafe extern "C" fn audio_batch(data: *const i16, frames: usize) -> usize {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if data.is_null() {
            return 0;
        }
        if let Ok(mut guard) = callback_state().lock() {
            if let Some(state) = guard.as_mut().filter(|state| state.active) {
                let accepted = accepted_audio_frames(state.audio.len(), frames);
                let samples =
                    unsafe { std::slice::from_raw_parts(data, accepted.saturating_mul(2)) };
                state.audio.extend_from_slice(samples);
                return accepted;
            }
        }
        0
    }))
    .unwrap_or(0)
}

unsafe extern "C" fn input_poll() {}

unsafe extern "C" fn input_state(port: u32, device: u32, _index: u32, id: u32) -> i16 {
    if port != 0 || device & RETRO_DEVICE_MASK != RETRO_DEVICE_JOYPAD {
        return 0;
    }
    let Ok(guard) = callback_state().lock() else {
        return 0;
    };
    let Some(state) = guard.as_ref().filter(|state| state.active) else {
        return 0;
    };
    if id == RETRO_DEVICE_ID_JOYPAD_MASK {
        return state.input.bits() as i16;
    }
    if id < 16 && state.input.bits() & (1 << id) != 0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod av_contract_tests {
    use super::{
        RetroAvInfo, RetroGeometry, RetroTiming, accepted_audio_frames, normalize_av_info,
    };

    #[test]
    fn runtime_av_updates_preserve_core_frame_and_sample_rates() {
        let av = normalize_av_info(RetroAvInfo {
            geometry: RetroGeometry {
                base_width: 320,
                base_height: 240,
                max_width: 640,
                max_height: 480,
                aspect_ratio: 4.0 / 3.0,
            },
            timing: RetroTiming {
                fps: 50.0,
                sample_rate: 22_050.0,
            },
        });
        assert_eq!(av.fps, 50.0);
        assert_eq!(av.sample_rate, 22_050.0);
        assert_eq!(av.geometry.base_width, 320);
    }

    #[test]
    fn invalid_runtime_timing_uses_safe_defaults() {
        let av = normalize_av_info(RetroAvInfo::default());
        assert_eq!(av.fps, 60.0);
        assert_eq!(av.sample_rate, 32_768.0);
        assert_eq!(av.geometry.base_width, 1);
    }

    #[test]
    fn unreasonable_sample_rate_is_clamped_for_safe_resampling() {
        let av = normalize_av_info(RetroAvInfo {
            timing: RetroTiming {
                sample_rate: 1_000_000.0,
                ..RetroTiming::default()
            },
            ..RetroAvInfo::default()
        });
        assert_eq!(av.sample_rate, 192_000.0);
    }

    #[test]
    fn audio_batch_is_capped_to_remaining_per_frame_budget() {
        assert_eq!(accepted_audio_frames(0, 100_000), 32_768);
        assert_eq!(accepted_audio_frames(65_536, 1), 0);
        assert_eq!(accepted_audio_frames(0, 100), 100);
    }
}
