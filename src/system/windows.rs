use crate::system::types::{
    AudioCommand, AudioDeviceSnapshot, AudioSnapshot, AvailableNetworkSnapshot, CpuSnapshot,
    DeviceCommand, GpuAdapterSnapshot, MediaCommand, MediaPlaybackState, MediaSnapshot,
    MemorySnapshot, NetworkInterfaceSnapshot, NetworkKind, NetworkSnapshot, SystemCommand,
    SystemSnapshot,
};
use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime},
};
use sysinfo::{Networks, System};
use windows::{
    Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    },
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::{BOOL, HANDLE, LUID},
        Graphics::Dxgi::{
            CreateDXGIFactory1, DXGI_ADAPTER_DESC1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
            IDXGIAdapter3, IDXGIFactory1,
        },
        Media::Audio::{
            DEVICE_STATE_ACTIVE, ERole, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator,
            MMDeviceEnumerator, eCommunications, eConsole, eMultimedia, eRender,
        },
        Media::Audio::Endpoints::IAudioEndpointVolume,
        NetworkManagement::WiFi::{
            WLAN_AVAILABLE_NETWORK_CONNECTED, WLAN_AVAILABLE_NETWORK_HAS_PROFILE,
            WLAN_AVAILABLE_NETWORK_LIST,
            WLAN_INTERFACE_INFO, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle, WlanEnumInterfaces,
            WlanFreeMemory, WlanGetAvailableNetworkList, WlanOpenHandle,
        },
        System::{
            Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ},
            Power::SetSuspendState,
            Shutdown::{
                EWX_LOGOFF, EWX_POWEROFF, EWX_SHUTDOWN, ExitWindowsEx, LockWorkStation,
                SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER,
            },
            Threading::{GetCurrentProcess, OpenProcessToken},
            WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
        },
        Security::{
            AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES,
            SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME, TOKEN_ACCESS_MASK, TOKEN_ADJUST_PRIVILEGES,
            TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
        UI::{
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, SW_SHOWNORMAL,
            },
        },
    },
    core::{
        Error as WindowsError, GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface, PCWSTR, PWSTR,
    },
};

struct SystemSampler {
    system: System,
    networks: Networks,
    last_network_refresh: Option<Instant>,
}

#[derive(Debug, Clone, Default)]
struct WifiInterfaceMetadata {
    interface_name: String,
    connected: bool,
    current_network: Option<String>,
    signal_percent: Option<u32>,
    available_networks: Vec<AvailableNetworkSnapshot>,
}

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq)]
struct IPolicyConfig(IUnknown);

unsafe impl Interface for IPolicyConfig {
    type Vtable = IPolicyConfig_Vtbl;
    const IID: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
}

#[repr(C)]
#[allow(non_snake_case)]
struct IPolicyConfig_Vtbl {
    pub base__: IUnknown_Vtbl,
    pub GetMixFormat: usize,
    pub GetDeviceFormat: usize,
    pub ResetDeviceFormat: usize,
    pub SetDeviceFormat: usize,
    pub GetProcessingPeriod: usize,
    pub SetProcessingPeriod: usize,
    pub GetShareMode: usize,
    pub SetShareMode: usize,
    pub GetPropertyValue: usize,
    pub SetPropertyValue: usize,
    pub SetDefaultEndpoint:
        unsafe extern "system" fn(this: *mut c_void, device_id: PCWSTR, role: ERole) -> HRESULT,
    pub SetEndpointVisibility: usize,
}

const CLSID_POLICY_CONFIG_CLIENT: GUID =
    GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

impl SystemSampler {
    fn new() -> Self {
        let mut system = System::new();
        system.refresh_memory();
        system.refresh_cpu_usage();

        Self {
            system,
            networks: Networks::new_with_refreshed_list(),
            last_network_refresh: None,
        }
    }

    fn refresh(&mut self) -> (CpuSnapshot, MemorySnapshot, NetworkSnapshot) {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh();

        let now = Instant::now();
        let elapsed = self
            .last_network_refresh
            .map(|last| now.saturating_duration_since(last).as_secs_f64())
            .unwrap_or(1.0)
            .max(0.001);
        self.last_network_refresh = Some(now);

        let cpu = CpuSnapshot {
            usage_percent: self.system.global_cpu_info().cpu_usage(),
            logical_cores: self.system.cpus().len(),
        };

        let memory = MemorySnapshot {
            used_bytes: self.system.used_memory(),
            total_bytes: self.system.total_memory(),
            available_bytes: self.system.available_memory(),
        };

        let mut network = NetworkSnapshot::default();

        for (name, data) in &self.networks {
            let received_bps = (data.received() as f64 / elapsed).round() as u64;
            let transmitted_bps = (data.transmitted() as f64 / elapsed).round() as u64;

            network.received_bps += received_bps;
            network.transmitted_bps += transmitted_bps;
            network.interfaces.push(NetworkInterfaceSnapshot {
                name: name.clone(),
                description: name.clone(),
                kind: infer_network_kind(name),
                connected: received_bps > 0 || transmitted_bps > 0,
                is_primary: false,
                detail_name: None,
                signal_percent: None,
                received_bps,
                transmitted_bps,
                available_networks: Vec::new(),
            });
        }

        network.interfaces.sort_by(|left, right| {
            (right.received_bps + right.transmitted_bps)
                .cmp(&(left.received_bps + left.transmitted_bps))
        });

        (cpu, memory, network)
    }
}

fn sampler() -> &'static Mutex<SystemSampler> {
    static SAMPLER: OnceLock<Mutex<SystemSampler>> = OnceLock::new();
    SAMPLER.get_or_init(|| Mutex::new(SystemSampler::new()))
}

pub(crate) fn refresh_snapshot() -> Result<SystemSnapshot, String> {
    let mut snapshot = SystemSnapshot::default();

    let (cpu, memory, mut network) = sampler()
        .lock()
        .map_err(|_| String::from("System sampler state lock was poisoned."))?
        .refresh();

    snapshot.active_window_title = query_active_window_title().unwrap_or_default();
    if let Err(error) = apply_wifi_metadata(&mut network) {
        snapshot.errors.push(error);
    }

    snapshot.cpu = cpu;
    snapshot.memory = memory;
    snapshot.network = network;

    match query_gpu_snapshots() {
        Ok(gpus) => snapshot.gpus = gpus,
        Err(error) => snapshot.errors.push(error),
    }

    match query_audio_snapshot() {
        Ok(audio) => snapshot.audio = Some(audio),
        Err(error) => snapshot.errors.push(error),
    }

    match query_media_snapshot() {
        Ok(media) => snapshot.media = media,
        Err(error) => snapshot.errors.push(error),
    }

    snapshot.last_updated = Some(SystemTime::now());

    Ok(snapshot)
}

pub(crate) fn apply_command(command: SystemCommand) -> Result<(), String> {
    match command {
        SystemCommand::Audio(command) => apply_audio_command(command),
        SystemCommand::Media(command) => apply_media_command(command),
        SystemCommand::Device(command) => apply_device_command(command),
    }
}

fn query_gpu_snapshots() -> Result<Vec<GpuAdapterSnapshot>, String> {
    initialize_runtime()?;

    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
        .map_err(|error| format!("CreateDXGIFactory1 failed: {error}"))?;
    let mut gpus = Vec::new();
    let mut index = 0;

    loop {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        index += 1;

        let desc = unsafe { adapter.GetDesc1() }
            .map_err(|error| format!("DXGI adapter description query failed: {error}"))?;
        let adapter3: IDXGIAdapter3 = adapter
            .cast()
            .map_err(|error| format!("DXGI adapter cast failed: {error}"))?;

        let mut local_info = Default::default();
        unsafe { adapter3.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut local_info) }
            .map_err(|error| format!("DXGI video memory query failed: {error}"))?;

        gpus.push(GpuAdapterSnapshot {
            name: dxgi_description_to_string(&desc),
            is_software: desc.Flags & 0x2 != 0,
            dedicated_memory_bytes: desc.DedicatedVideoMemory as u64,
            shared_memory_bytes: desc.SharedSystemMemory as u64,
            local_usage_bytes: local_info.CurrentUsage,
            local_budget_bytes: local_info.Budget,
        });
    }

    Ok(gpus)
}

fn query_audio_snapshot() -> Result<AudioSnapshot, String> {
    let volume = default_endpoint_volume()?;
    let default_device = default_audio_endpoint_for_role(eConsole)?;
    let default_device_id = audio_device_id(&default_device)?;
    let mut devices = enumerate_output_devices()?;

    for device in &mut devices {
        device.is_default = device.id == default_device_id;
    }

    Ok(AudioSnapshot {
        device_name: devices
            .iter()
            .find(|device| device.is_default)
            .map(|device| device.name.clone())
            .unwrap_or_else(|| query_audio_device_name(&default_device).unwrap_or_default()),
        volume: unsafe { volume.GetMasterVolumeLevelScalar() }
            .map_err(|error| format!("Default audio volume query failed: {error}"))?,
        muted: unsafe { volume.GetMute() }
            .map_err(|error| format!("Default audio mute query failed: {error}"))?
            .as_bool(),
        devices,
    })
}

fn query_audio_device_name(device: &IMMDevice) -> Result<String, String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }
        .map_err(|error| format!("Default audio property store open failed: {error}"))?;
    let value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }
        .map_err(|error| format!("Default audio friendly-name query failed: {error}"))?;
    let name = value.to_string();

    if name.trim().is_empty() {
        Err(String::from("Default audio friendly name was empty."))
    } else {
        Ok(name)
    }
}

fn enumerate_output_devices() -> Result<Vec<AudioDeviceSnapshot>, String> {
    let enumerator = audio_enumerator()?;
    let collection: IMMDeviceCollection = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }
        .map_err(|error| format!("Audio output enumeration failed: {error}"))?;
    let count = unsafe { collection.GetCount() }
        .map_err(|error| format!("Audio output count query failed: {error}"))?;
    let mut devices = Vec::with_capacity(count as usize);

    for index in 0..count {
        let device = unsafe { collection.Item(index) }
            .map_err(|error| format!("Audio output item query failed: {error}"))?;
        let id = audio_device_id(&device)?;
        let name = query_audio_device_name(&device).unwrap_or_else(|_| id.clone());

        devices.push(AudioDeviceSnapshot {
            id,
            name,
            is_default: false,
        });
    }

    Ok(devices)
}

fn query_media_snapshot() -> Result<Option<MediaSnapshot>, String> {
    initialize_runtime()?;

    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .map_err(|error| format!("Media session manager request failed: {error}"))?
        .get()
        .map_err(|error| format!("Media session manager wait failed: {error}"))?;

    let session = match manager.GetCurrentSession() {
        Ok(session) => session,
        Err(_) => return Ok(None),
    };

    Ok(Some(snapshot_from_session(&session)?))
}

fn apply_audio_command(command: AudioCommand) -> Result<(), String> {
    match command {
        AudioCommand::SetVolume(level) => {
            let volume = default_endpoint_volume()?;
            unsafe { volume.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null()) }
                .map_err(|error| format!("Setting output volume failed: {error}"))
        }
        AudioCommand::SetMuted(muted) => {
            let volume = default_endpoint_volume()?;
            unsafe { volume.SetMute(BOOL::from(muted), std::ptr::null()) }
                .map_err(|error| format!("Setting output mute failed: {error}"))
        }
        AudioCommand::SetDefaultOutputDevice(device_id) => set_default_output_device(&device_id),
    }
}

fn apply_media_command(command: MediaCommand) -> Result<(), String> {
    let session = current_media_session()?.ok_or_else(|| String::from("No active media session."))?;

    match command {
        MediaCommand::PlayPause => {
            let ok = session
                .TryTogglePlayPauseAsync()
                .map_err(|error| format!("Starting play/pause request failed: {error}"))?
                .get()
                .map_err(|error| format!("Play/pause request failed: {error}"))?;

            if ok {
                Ok(())
            } else {
                Err(String::from("The active media session rejected play/pause."))
            }
        }
        MediaCommand::Next => {
            let ok = session
                .TrySkipNextAsync()
                .map_err(|error| format!("Starting next-track request failed: {error}"))?
                .get()
                .map_err(|error| format!("Next-track request failed: {error}"))?;

            if ok {
                Ok(())
            } else {
                Err(String::from("The active media session rejected next track."))
            }
        }
        MediaCommand::Previous => {
            let ok = session
                .TrySkipPreviousAsync()
                .map_err(|error| format!("Starting previous-track request failed: {error}"))?
                .get()
                .map_err(|error| format!("Previous-track request failed: {error}"))?;

            if ok {
                Ok(())
            } else {
                Err(String::from("The active media session rejected previous track."))
            }
        }
    }
}

fn apply_device_command(command: DeviceCommand) -> Result<(), String> {
    match command {
        DeviceCommand::OpenSettings => open_settings(),
        DeviceCommand::LogOut => unsafe {
            ExitWindowsEx(
                EWX_LOGOFF,
                SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_FLAG_PLANNED,
            )
            .map_err(|error| format!("Log out request failed: {error}"))
        },
        DeviceCommand::Lock => unsafe {
            LockWorkStation().map_err(|error| format!("Lock workstation failed: {error}"))
        },
        DeviceCommand::Shutdown => {
            enable_shutdown_privilege()?;
            unsafe {
                ExitWindowsEx(
                    EWX_SHUTDOWN | EWX_POWEROFF,
                    SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_FLAG_PLANNED,
                )
                .map_err(|error| format!("Shutdown request failed: {error}"))
            }
        }
        DeviceCommand::Sleep => {
            let result = unsafe { SetSuspendState(false, false, false) };
            if result.as_bool() {
                Ok(())
            } else {
                Err(String::from("Sleep request was rejected by the system."))
            }
        }
    }
}

fn current_media_session() -> Result<Option<GlobalSystemMediaTransportControlsSession>, String> {
    initialize_runtime()?;

    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .map_err(|error| format!("Media session manager request failed: {error}"))?
        .get()
        .map_err(|error| format!("Media session manager wait failed: {error}"))?;

    match manager.GetCurrentSession() {
        Ok(session) => Ok(Some(session)),
        Err(_) => Ok(None),
    }
}

fn query_active_window_title() -> Result<String, String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return Ok(String::new());
    }

    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return Ok(String::new());
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if copied <= 0 {
        return Ok(String::new());
    }

    Ok(String::from_utf16_lossy(&buffer[..copied as usize]).trim().to_string())
}

fn snapshot_from_session(
    session: &GlobalSystemMediaTransportControlsSession,
) -> Result<MediaSnapshot, String> {
    let source_app_id = session
        .SourceAppUserModelId()
        .map(|value| value.to_string())
        .map_err(|error| format!("Media source app id query failed: {error}"))?;
    let media = session
        .TryGetMediaPropertiesAsync()
        .map_err(|error| format!("Media properties request failed: {error}"))?
        .get()
        .map_err(|error| format!("Media properties wait failed: {error}"))?;
    let playback = session
        .GetPlaybackInfo()
        .map_err(|error| format!("Media playback info query failed: {error}"))?;
    let controls = playback
        .Controls()
        .map_err(|error| format!("Media playback controls query failed: {error}"))?;
    let timeline = session
        .GetTimelineProperties()
        .map_err(|error| format!("Media timeline query failed: {error}"))?;

    let end = timeline.EndTime().ok().map(timespan_to_seconds);
    let position = timeline.Position().ok().map(timespan_to_seconds);

    Ok(MediaSnapshot {
        title: media.Title().map(|value| value.to_string()).unwrap_or_default(),
        artist: media.Artist().map(|value| value.to_string()).unwrap_or_default(),
        album: media.AlbumTitle().map(|value| value.to_string()).unwrap_or_default(),
        source_app_id,
        playback: map_playback_state(
            playback
                .PlaybackStatus()
                .map_err(|error| format!("Media playback status query failed: {error}"))?,
        ),
        position_seconds: position,
        duration_seconds: end,
        can_toggle_play_pause: controls.IsPlayPauseToggleEnabled().unwrap_or(false),
        can_go_next: controls.IsNextEnabled().unwrap_or(false),
        can_go_previous: controls.IsPreviousEnabled().unwrap_or(false),
    })
}

fn audio_enumerator() -> Result<IMMDeviceEnumerator, String> {
    initialize_runtime()?;

    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
        .map_err(|error| format!("Default audio enumerator creation failed: {error}"))
}

fn open_settings() -> Result<(), String> {
    let operation = wide_null("open");
    let target = wide_null("ms-settings:");
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    if result.0 as isize <= 32 {
        Err(format!("Opening Settings failed with shell code {}.", result.0 as isize))
    } else {
        Ok(())
    }
}

fn enable_shutdown_privilege() -> Result<(), String> {
    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ACCESS_MASK(TOKEN_ADJUST_PRIVILEGES.0 | TOKEN_QUERY.0),
            &mut token,
        )
    }
    .map_err(|error| format!("Opening process token failed: {error}"))?;

    let mut luid = LUID::default();
    unsafe { LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid) }
        .map_err(|error| format!("LookupPrivilegeValueW failed: {error}"))?;

    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };

    unsafe {
        AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None)
            .map_err(|error| format!("AdjustTokenPrivileges failed: {error}"))
    }
}

fn default_audio_endpoint_for_role(role: ERole) -> Result<IMMDevice, String> {
    let enumerator = audio_enumerator()?;

    unsafe { enumerator.GetDefaultAudioEndpoint(eRender, role) }
        .map_err(|error| format!("Default audio endpoint query failed: {error}"))
}

fn default_endpoint_volume() -> Result<IAudioEndpointVolume, String> {
    let device = default_audio_endpoint_for_role(eConsole)?;

    unsafe { device.Activate(CLSCTX_ALL, None) }
        .map_err(|error| format!("Audio endpoint volume activation failed: {error}"))
}

fn audio_device_id(device: &IMMDevice) -> Result<String, String> {
    let id = unsafe { device.GetId() }
        .map_err(|error| format!("Default audio endpoint id query failed: {error}"))?;
    Ok(take_pwstr(id))
}

fn set_default_output_device(device_id: &str) -> Result<(), String> {
    initialize_runtime()?;

    let policy: IPolicyConfig = unsafe { CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL) }
        .map_err(|error| format!("PolicyConfig creation failed: {error}"))?;
    let wide = wide_null(device_id);

    for role in [eConsole, eMultimedia, eCommunications] {
        let hr = unsafe {
            (policy.vtable().SetDefaultEndpoint)(policy.as_raw(), PCWSTR(wide.as_ptr()), role)
        };

        hr.ok()
            .map_err(|error| format!("Default output device switch failed: {error}"))?;
    }

    Ok(())
}

fn apply_wifi_metadata(network: &mut NetworkSnapshot) -> Result<(), String> {
    let wifi_interfaces = enumerate_wifi_interfaces()?;

    for metadata in wifi_interfaces {
        let normalized_wifi_name = normalize_name(&metadata.interface_name);

        let matching_index = network.interfaces.iter().position(|interface| {
            let normalized_interface_name = normalize_name(&interface.name);
            let normalized_description = normalize_name(&interface.description);

            normalized_interface_name == normalized_wifi_name
                || normalized_description == normalized_wifi_name
                || normalized_interface_name.contains(&normalized_wifi_name)
                || normalized_wifi_name.contains(&normalized_interface_name)
        });

        if let Some(index) = matching_index {
            let interface = &mut network.interfaces[index];
            interface.kind = NetworkKind::Wifi;
            interface.connected = metadata.connected || interface.connected;
            interface.detail_name = metadata.current_network.clone();
            interface.signal_percent = metadata.signal_percent;
            interface.available_networks = metadata.available_networks.clone();
            interface.description = metadata.interface_name.clone();
        } else {
            network.interfaces.push(NetworkInterfaceSnapshot {
                name: metadata.interface_name.clone(),
                description: metadata.interface_name.clone(),
                kind: NetworkKind::Wifi,
                connected: metadata.connected,
                is_primary: false,
                detail_name: metadata.current_network.clone(),
                signal_percent: metadata.signal_percent,
                received_bps: 0,
                transmitted_bps: 0,
                available_networks: metadata.available_networks.clone(),
            });
        }
    }

    let primary_index = network
        .interfaces
        .iter()
        .position(|interface| interface.connected && interface.kind == NetworkKind::Wifi)
        .or_else(|| {
            network
                .interfaces
                .iter()
                .position(|interface| interface.connected || interface.received_bps > 0 || interface.transmitted_bps > 0)
        });

    if let Some(index) = primary_index {
        network.interfaces[index].is_primary = true;
    }

    network.interfaces.sort_by(|left, right| {
        right
            .is_primary
            .cmp(&left.is_primary)
            .then((right.received_bps + right.transmitted_bps).cmp(&(left.received_bps + left.transmitted_bps)))
            .then(left.name.cmp(&right.name))
    });

    Ok(())
}

fn enumerate_wifi_interfaces() -> Result<Vec<WifiInterfaceMetadata>, String> {
    initialize_runtime()?;

    let mut negotiated = 0;
    let mut handle = HANDLE::default();
    check_wlan(
        unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut handle) },
        "WLAN client open",
    )?;

    let mut interface_list = std::ptr::null_mut::<WLAN_INTERFACE_INFO_LIST>();
    let enum_result = unsafe { WlanEnumInterfaces(handle, None, &mut interface_list) };
    let mut interfaces = Vec::new();

    if enum_result == 0 && !interface_list.is_null() {
        let items = unsafe {
            std::slice::from_raw_parts(
                (*interface_list).InterfaceInfo.as_ptr(),
                (*interface_list).dwNumberOfItems as usize,
            )
        };

        for info in items {
            interfaces.push(snapshot_wifi_interface(handle, info)?);
        }
    }

    unsafe {
        if !interface_list.is_null() {
            WlanFreeMemory(interface_list.cast());
        }
        WlanCloseHandle(handle, None);
    }

    check_wlan(enum_result, "WLAN interface enumeration")?;
    Ok(interfaces)
}

fn snapshot_wifi_interface(handle: HANDLE, info: &WLAN_INTERFACE_INFO) -> Result<WifiInterfaceMetadata, String> {
    let mut network_list = std::ptr::null_mut::<WLAN_AVAILABLE_NETWORK_LIST>();
    check_wlan(
        unsafe { WlanGetAvailableNetworkList(handle, &info.InterfaceGuid, 0, None, &mut network_list) },
        "WLAN network list query",
    )?;

    let mut available_networks = Vec::new();
    let mut current_network = None;
    let mut signal_percent = None;

    if !network_list.is_null() {
        let items = unsafe {
            std::slice::from_raw_parts(
                (*network_list).Network.as_ptr(),
                (*network_list).dwNumberOfItems as usize,
            )
        };

        for network in items {
            let ssid = dot11_ssid_to_string(&network.dot11Ssid);
            if ssid.is_empty() {
                continue;
            }

            let connected = network.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED != 0;
            if connected {
                current_network = Some(ssid.clone());
                signal_percent = Some(network.wlanSignalQuality);
            }

            available_networks.push(AvailableNetworkSnapshot {
                ssid,
                connected,
                secure: network.bSecurityEnabled.as_bool(),
                saved: network.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE != 0,
                signal_percent: network.wlanSignalQuality,
            });
        }
    }

    unsafe {
        if !network_list.is_null() {
            WlanFreeMemory(network_list.cast());
        }
    }

    available_networks.sort_by(|left, right| {
        right
            .connected
            .cmp(&left.connected)
            .then(right.signal_percent.cmp(&left.signal_percent))
            .then(left.ssid.cmp(&right.ssid))
    });

    Ok(WifiInterfaceMetadata {
        interface_name: utf16_to_string(&info.strInterfaceDescription),
        connected: current_network.is_some(),
        current_network,
        signal_percent,
        available_networks,
    })
}

fn initialize_runtime() -> Result<(), String> {
    match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        Ok(()) => Ok(()),
        Err(error) if is_changed_mode(&error) => Ok(()),
        Err(error) => Err(format!("Windows runtime initialization failed: {error}")),
    }
}

fn is_changed_mode(error: &WindowsError) -> bool {
    error.code().0 == 0x8001_0106_u32 as i32
}

fn map_playback_state(
    state: GlobalSystemMediaTransportControlsSessionPlaybackStatus,
) -> MediaPlaybackState {
    match state {
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed => MediaPlaybackState::Closed,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Opened => MediaPlaybackState::Opened,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing => MediaPlaybackState::Changing,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => MediaPlaybackState::Stopped,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => MediaPlaybackState::Playing,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => MediaPlaybackState::Paused,
        _ => MediaPlaybackState::Unknown,
    }
}

fn dxgi_description_to_string(desc: &DXGI_ADAPTER_DESC1) -> String {
    utf16_to_string(&desc.Description)
}

fn utf16_to_string(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&value| value == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end]).trim().to_string()
}

fn take_pwstr(value: PWSTR) -> String {
    if value.is_null() {
        return String::new();
    }

    let text = unsafe { value.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    text
}

fn timespan_to_seconds(span: windows::Foundation::TimeSpan) -> f64 {
    span.Duration as f64 / 10_000_000.0
}

fn infer_network_kind(name: &str) -> NetworkKind {
    let normalized = name.to_ascii_lowercase();

    if normalized.contains("wi-fi") || normalized.contains("wifi") || normalized.contains("wireless") {
        NetworkKind::Wifi
    } else if normalized.contains("ethernet") || normalized.contains("lan") {
        NetworkKind::Ethernet
    } else {
        NetworkKind::Other
    }
}

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect()
}

fn check_wlan(result: u32, context: &str) -> Result<(), String> {
    if result == 0 {
        Ok(())
    } else {
        Err(format!("{context} failed with code {result}."))
    }
}

fn dot11_ssid_to_string(ssid: &windows::Win32::NetworkManagement::WiFi::DOT11_SSID) -> String {
    let length = ssid.uSSIDLength.min(ssid.ucSSID.len() as u32) as usize;
    String::from_utf8_lossy(&ssid.ucSSID[..length]).trim().to_string()
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
