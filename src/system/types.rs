use std::time::SystemTime;

#[derive(Debug, Clone, Default)]
pub(crate) struct SystemSnapshot {
    pub(crate) active_window_title: String,
    pub(crate) cpu: CpuSnapshot,
    pub(crate) memory: MemorySnapshot,
    pub(crate) gpus: Vec<GpuAdapterSnapshot>,
    pub(crate) network: NetworkSnapshot,
    pub(crate) audio: Option<AudioSnapshot>,
    pub(crate) media: Option<MediaSnapshot>,
    pub(crate) errors: Vec<String>,
    pub(crate) last_updated: Option<SystemTime>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CpuSnapshot {
    pub(crate) usage_percent: f32,
    pub(crate) logical_cores: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MemorySnapshot {
    pub(crate) used_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) available_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GpuAdapterSnapshot {
    pub(crate) name: String,
    pub(crate) is_software: bool,
    pub(crate) dedicated_memory_bytes: u64,
    pub(crate) shared_memory_bytes: u64,
    pub(crate) local_usage_bytes: u64,
    pub(crate) local_budget_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NetworkSnapshot {
    pub(crate) received_bps: u64,
    pub(crate) transmitted_bps: u64,
    pub(crate) interfaces: Vec<NetworkInterfaceSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NetworkInterfaceSnapshot {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) kind: NetworkKind,
    pub(crate) connected: bool,
    pub(crate) is_primary: bool,
    pub(crate) detail_name: Option<String>,
    pub(crate) signal_percent: Option<u32>,
    pub(crate) received_bps: u64,
    pub(crate) transmitted_bps: u64,
    pub(crate) available_networks: Vec<AvailableNetworkSnapshot>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum NetworkKind {
    #[default]
    Other,
    Ethernet,
    Wifi,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AvailableNetworkSnapshot {
    pub(crate) ssid: String,
    pub(crate) connected: bool,
    pub(crate) secure: bool,
    pub(crate) saved: bool,
    pub(crate) signal_percent: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AudioSnapshot {
    pub(crate) device_name: String,
    pub(crate) volume: f32,
    pub(crate) muted: bool,
    pub(crate) devices: Vec<AudioDeviceSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AudioDeviceSnapshot {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MediaSnapshot {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) source_app_id: String,
    pub(crate) playback: MediaPlaybackState,
    pub(crate) position_seconds: Option<f64>,
    pub(crate) duration_seconds: Option<f64>,
    pub(crate) can_toggle_play_pause: bool,
    pub(crate) can_go_next: bool,
    pub(crate) can_go_previous: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum MediaPlaybackState {
    #[default]
    Unknown,
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
}

impl MediaPlaybackState {
    pub(crate) fn as_label(self) -> &'static str {
        match self {
            MediaPlaybackState::Unknown => "Unknown",
            MediaPlaybackState::Closed => "Closed",
            MediaPlaybackState::Opened => "Opened",
            MediaPlaybackState::Changing => "Changing",
            MediaPlaybackState::Stopped => "Stopped",
            MediaPlaybackState::Playing => "Playing",
            MediaPlaybackState::Paused => "Paused",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SystemCommand {
    Audio(AudioCommand),
    Media(MediaCommand),
    Device(DeviceCommand),
}

#[derive(Debug, Clone)]
pub(crate) enum AudioCommand {
    SetVolume(f32),
    SetMuted(bool),
    SetDefaultOutputDevice(String),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MediaCommand {
    PlayPause,
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DeviceCommand {
    OpenSettings,
    LogOut,
    Lock,
    Shutdown,
    Sleep,
}
