#[derive(Debug, Clone, Copy)]
pub(crate) struct ReservedArea {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) monitor_height: i32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlyoutAnchor {
    pub(crate) right: i32,
    pub(crate) top: i32,
}

#[cfg(target_os = "windows")]
pub(crate) use imp::{install_appbar, sync_flyout, sync_startup_registration};

#[cfg(target_os = "windows")]
mod imp {
    use super::{FlyoutAnchor, ReservedArea};
    use crate::app::{BAR_RADIUS, FLYOUT_RADIUS};
    use iced::{window, Size};
    use raw_window_handle::RawWindowHandle;
    use std::{
        collections::HashMap,
        mem, ptr,
        sync::{Mutex, OnceLock},
    };
    use windows::{
        core::{PCWSTR, w},
        Win32::{
            Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HWND, LPARAM, LRESULT, RECT, WPARAM},
            Graphics::{
                Dwm::{DWM_BB_ENABLE, DWM_BLURBEHIND, DwmEnableBlurBehindWindow},
                Gdi::{
                    CreateRoundRectRgn, GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
                    MonitorFromWindow, SetWindowRgn,
                },
            },
            System::Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPEN_CREATE_OPTIONS,
                REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
                RegSetValueExW,
            },
            UI::{
                Shell::{
                    ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, ABN_POSCHANGED,
                    APPBARDATA, SHAppBarMessage,
                },
                WindowsAndMessaging::{
                    CallWindowProcW, FindWindowExW, FindWindowW, GWL_EXSTYLE, GWL_STYLE,
                    GWLP_WNDPROC, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST, MoveWindow,
                    SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_SHOWWINDOW,
                    SW_HIDE, SW_SHOW, SW_SHOWNA, SW_SHOWNOACTIVATE, SetWindowLongPtrW,
                    SetWindowPos, ShowWindow, WINDOWPOS, WM_APP, WM_DISPLAYCHANGE, WM_NCDESTROY,
                    WM_WINDOWPOSCHANGED, WM_WINDOWPOSCHANGING, WS_EX_APPWINDOW,
                    WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW,
                    WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
                },
            },
        },
        core::Error,
    };

    const APPBAR_CALLBACK_MESSAGE: u32 = WM_APP + 1;
    const RUN_KEY_PATH: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const RUN_VALUE_NAME: PCWSTR = w!("Rebar");

    #[derive(Debug, Clone, Copy)]
    struct AppBarWindow {
        original_wndproc: isize,
        height: i32,
        registered: bool,
        reserving: bool,
        hide_system_taskbar: bool,
        launch_in_background: bool,
    }

    #[derive(Debug, Clone, Copy)]
    struct FlyoutWindow {
        original_wndproc: isize,
        anchor: FlyoutAnchor,
        size: Size,
        reveal_height: f32,
        visible: bool,
        generation: u64,
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct SystemTaskbarState {
        hidden_by_app: bool,
    }

    fn appbar_windows() -> &'static Mutex<HashMap<isize, AppBarWindow>> {
        static WINDOWS: OnceLock<Mutex<HashMap<isize, AppBarWindow>>> = OnceLock::new();
        WINDOWS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn flyout_windows() -> &'static Mutex<HashMap<isize, FlyoutWindow>> {
        static WINDOWS: OnceLock<Mutex<HashMap<isize, FlyoutWindow>>> = OnceLock::new();
        WINDOWS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn system_taskbar_state() -> &'static Mutex<SystemTaskbarState> {
        static STATE: OnceLock<Mutex<SystemTaskbarState>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(SystemTaskbarState::default()))
    }

    fn hwnd_key(hwnd: HWND) -> isize {
        hwnd.0 as isize
    }

    pub(crate) unsafe fn install_appbar(
        native_window: &dyn window::Window,
        height: i32,
        hide_system_taskbar: bool,
        launch_in_background: bool,
    ) -> Result<ReservedArea, String> {
        let hwnd = unsafe { hwnd_from_handle(native_window)? };

        if let Some(window) = appbar_windows()
            .lock()
            .map_err(|_| String::from("Appbar state lock was poisoned."))?
            .get_mut(&hwnd_key(hwnd))
        {
            window.height = height;
            window.hide_system_taskbar = hide_system_taskbar;
            window.launch_in_background = launch_in_background;

            let area = unsafe { reserve_top_edge(hwnd, height) }?;
            let _ = unsafe {
                ShowWindow(
                    hwnd,
                    if launch_in_background {
                        SW_SHOWNOACTIVATE
                    } else {
                        SW_SHOW
                    },
                )
            };
            unsafe { set_system_taskbar_hidden(hide_system_taskbar)? };

            return Ok(area);
        }

        unsafe { make_tool_window(hwnd)? };

        let original_wndproc =
            unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, appbar_wndproc as *const () as _) };

        appbar_windows()
            .lock()
            .map_err(|_| String::from("Appbar state lock was poisoned."))?
            .insert(
                hwnd_key(hwnd),
                AppBarWindow {
                    original_wndproc,
                    height,
                    registered: false,
                    reserving: false,
                    hide_system_taskbar,
                    launch_in_background,
                },
            );

        let mut appbar_data = new_appbar_data(hwnd, height);
        let registered = unsafe { SHAppBarMessage(ABM_NEW, &mut appbar_data) } != 0;

        if !registered {
            unsafe { cleanup_appbar(hwnd) };
            return Err(last_error("ABM_NEW failed"));
        }

        if let Some(state) = appbar_windows()
            .lock()
            .map_err(|_| String::from("Appbar state lock was poisoned."))?
            .get_mut(&hwnd_key(hwnd))
        {
            state.registered = true;
        }

        let area = unsafe { reserve_top_edge(hwnd, height) }?;
        let _ = unsafe {
            ShowWindow(
                hwnd,
                if launch_in_background {
                    SW_SHOWNOACTIVATE
                } else {
                    SW_SHOW
                },
            )
        };
        unsafe { set_system_taskbar_hidden(hide_system_taskbar)? };

        Ok(area)
    }

    pub(crate) fn sync_startup_registration(
        enabled: bool,
        startup_mode: crate::config::StartupMode,
        config_path: &std::path::Path,
    ) -> Result<(), String> {
        let mut key = HKEY::default();

        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                RUN_KEY_PATH,
                0,
                PCWSTR::null(),
                REG_OPEN_CREATE_OPTIONS(REG_OPTION_NON_VOLATILE.0),
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
        };

        if status != ERROR_SUCCESS {
            return Err(format!(
                "RegCreateKeyExW failed while opening the Run key: {}",
                Error::from_win32()
            ));
        }

        let result = if enabled {
            let command = startup_command(startup_mode, config_path)?;
            let bytes = utf16_bytes(&command);
            let status = unsafe { RegSetValueExW(key, RUN_VALUE_NAME, 0, REG_SZ, Some(&bytes)) };

            if status == ERROR_SUCCESS {
                Ok(())
            } else {
                Err(format!(
                    "RegSetValueExW failed while registering startup: {}",
                    Error::from_win32()
                ))
            }
        } else {
            let status = unsafe { RegDeleteValueW(key, RUN_VALUE_NAME) };

            if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!(
                    "RegDeleteValueW failed while removing startup registration: {}",
                    Error::from_win32()
                ))
            }
        };

        unsafe {
            let _ = RegCloseKey(key);
        }

        result
    }

    pub(crate) unsafe fn sync_flyout(
        native_window: &dyn window::Window,
        anchor: FlyoutAnchor,
        size: Size,
        reveal_height: f32,
        visible: bool,
        generation: u64,
    ) -> Result<(), String> {
        let hwnd = unsafe { hwnd_from_handle(native_window)? };

        if let Some(window) = flyout_windows()
            .lock()
            .map_err(|_| String::from("Flyout state lock was poisoned."))?
            .get_mut(&hwnd_key(hwnd))
        {
            if generation < window.generation {
                return Ok(());
            }

            window.anchor = anchor;
            window.size = size;
            window.reveal_height = reveal_height;
            window.visible = visible;
            window.generation = generation;
        } else {
            unsafe { make_flyout_window(hwnd)? };

            let original_wndproc =
                unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, flyout_wndproc as *const () as _) };

            flyout_windows()
                .lock()
                .map_err(|_| String::from("Flyout state lock was poisoned."))?
                .insert(
                    hwnd_key(hwnd),
                    FlyoutWindow {
                        original_wndproc,
                        anchor,
                        size,
                        reveal_height,
                        visible,
                        generation,
                    },
                );
        }

        if visible {
            unsafe { apply_flyout_state(hwnd, true) }?;
            let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        } else {
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        }

        Ok(())
    }

    unsafe fn hwnd_from_handle(native_window: &dyn window::Window) -> Result<HWND, String> {
        let handle = native_window
            .window_handle()
            .map_err(|error| format!("Could not get the native window handle: {error}"))?;

        match handle.as_raw() {
            RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as _)),
            _ => Err(String::from("iced did not provide a Win32 window handle.")),
        }
    }

    unsafe fn enable_blur(hwnd: HWND) {
        let blur = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE,
            fEnable: true.into(),
            hRgnBlur: Default::default(),
            fTransitionOnMaximized: false.into(),
        };

        let _ = unsafe { DwmEnableBlurBehindWindow(hwnd, &blur) };
    }

    unsafe fn apply_rounded_region(hwnd: HWND, radius: i32) -> Result<(), String> {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut rect) }
            .map_err(|error| format!("GetWindowRect failed while rounding window: {error}"))?;

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        if width <= 0 || height <= 0 {
            return Ok(());
        }

        let region =
            unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2) };

        if region.0 == ptr::null_mut() {
            return Err(last_error("CreateRoundRectRgn failed"));
        }

        let result = unsafe { SetWindowRgn(hwnd, region, true) };

        if result == 0 {
            Err(last_error("SetWindowRgn failed while rounding window"))
        } else {
            Ok(())
        }
    }

    unsafe fn apply_partial_rounded_region(
        hwnd: HWND,
        width: i32,
        height: i32,
        radius: i32,
    ) -> Result<(), String> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }

        let diameter = (radius * 2).min(height.max(1));
        let region =
            unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter) };

        if region.0 == ptr::null_mut() {
            return Err(last_error("CreateRoundRectRgn failed"));
        }

        let result = unsafe { SetWindowRgn(hwnd, region, true) };

        if result == 0 {
            Err(last_error("SetWindowRgn failed while updating flyout region"))
        } else {
            Ok(())
        }
    }

    unsafe fn make_tool_window(hwnd: HWND) -> Result<(), String> {
        let mut style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 };
        style &= !(WS_OVERLAPPEDWINDOW.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0);
        style |= WS_POPUP.0;
        unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize) };

        let mut ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 };
        ex_style &= !WS_EX_APPWINDOW.0;
        ex_style |= WS_EX_TOOLWINDOW.0;
        unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style as isize) };

        let result = unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER,
            )
        }
        .map_err(|error| format!("SetWindowPos failed while applying tool-window styles: {error}"));

        unsafe { enable_blur(hwnd) };
        let _ = unsafe { apply_rounded_region(hwnd, BAR_RADIUS) };
        result
    }

    unsafe fn make_flyout_window(hwnd: HWND) -> Result<(), String> {
        let mut style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 };
        style &= !(WS_OVERLAPPEDWINDOW.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0);
        style |= WS_POPUP.0 | WS_THICKFRAME.0;
        unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize) };

        let mut ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 };
        ex_style &= !WS_EX_APPWINDOW.0;
        ex_style |= WS_EX_TOOLWINDOW.0;
        unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style as isize) };

        let result = unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER,
            )
        }
        .map_err(|error| format!("SetWindowPos failed while applying flyout styles: {error}"));

        unsafe { enable_blur(hwnd) };
        let _ = unsafe { apply_rounded_region(hwnd, FLYOUT_RADIUS) };
        result
    }

    unsafe fn apply_flyout_region(hwnd: HWND) -> Result<(), String> {
        let Some(window) = flyout_windows()
            .lock()
            .ok()
            .and_then(|state| state.get(&hwnd_key(hwnd)).copied())
        else {
            return Ok(());
        };

        let mut rect = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut rect) }
            .map_err(|error| format!("GetWindowRect failed while updating flyout region: {error}"))?;

        let width = (rect.right - rect.left).max(1);
        let height = (rect.bottom - rect.top).max(1);
        let reveal_height = window.reveal_height.round().clamp(1.0, height as f32) as i32;

        unsafe { apply_partial_rounded_region(hwnd, width, reveal_height, FLYOUT_RADIUS) }
    }

    unsafe fn apply_flyout_state(hwnd: HWND, show: bool) -> Result<(), String> {
        let Some(window) = flyout_windows()
            .lock()
            .ok()
            .and_then(|state| state.get(&hwnd_key(hwnd)).copied())
        else {
            return Ok(());
        };

        let width = window.size.width.round().max(1.0) as i32;
        let height = window.size.height.round().max(1.0) as i32;
        let mut flags = SWP_FRAMECHANGED | SWP_NOOWNERZORDER;

        if show {
            flags |= SWP_SHOWWINDOW;
        }

        unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                window.anchor.right - width,
                window.anchor.top,
                width,
                height,
                flags,
            )
        }
        .map_err(|error| format!("SetWindowPos failed while anchoring flyout: {error}"))?;

        unsafe { apply_flyout_region(hwnd) }
    }

    unsafe fn reserve_top_edge(hwnd: HWND, height: i32) -> Result<ReservedArea, String> {
        {
            let mut windows = appbar_windows()
                .lock()
                .map_err(|_| String::from("Appbar state lock was poisoned."))?;

            let Some(window) = windows.get_mut(&hwnd_key(hwnd)) else {
                return Err(String::from("Appbar window state was missing during reservation."));
            };

            if window.reserving {
                let mut rect = RECT::default();
                unsafe { GetWindowRect(hwnd, &mut rect) }.map_err(|error| {
                    format!("GetWindowRect failed while reading the current appbar bounds: {error}")
                })?;

                return Ok(ReservedArea {
                    left: rect.left,
                    top: rect.top,
                    width: rect.right - rect.left,
                    height: rect.bottom - rect.top,
                    monitor_height: 0,
                });
            }

            window.reserving = true;
        }

        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) };

        let mut monitor_info = MONITORINFO {
            cbSize: mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        let got_monitor =
            unsafe { GetMonitorInfoW(monitor, &mut monitor_info as *mut _ as *mut _) }.as_bool();
        if !got_monitor {
            clear_appbar_reserving(hwnd);
            return Err(last_error("GetMonitorInfoW failed"));
        }

        let mut appbar_data = new_appbar_data(hwnd, height);
        appbar_data.rc = RECT {
            left: monitor_info.rcMonitor.left,
            top: monitor_info.rcMonitor.top,
            right: monitor_info.rcMonitor.right,
            bottom: monitor_info.rcMonitor.top + height,
        };

        unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut appbar_data) };
        appbar_data.rc.bottom = appbar_data.rc.top + height;
        unsafe { SHAppBarMessage(ABM_SETPOS, &mut appbar_data) };

        let width = appbar_data.rc.right - appbar_data.rc.left;
        unsafe {
            MoveWindow(
                hwnd,
                appbar_data.rc.left,
                appbar_data.rc.top,
                width,
                height,
                true,
            )
        }
        .map_err(|error| {
            clear_appbar_reserving(hwnd);
            format!("MoveWindow failed while sizing the appbar: {error}")
        })?;

        unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER,
            )
        }
        .map_err(|error| {
            clear_appbar_reserving(hwnd);
            format!("SetWindowPos failed while keeping the appbar topmost: {error}")
        })?;

        if let Err(error) = unsafe { apply_rounded_region(hwnd, BAR_RADIUS) } {
            clear_appbar_reserving(hwnd);
            return Err(error);
        }
        clear_appbar_reserving(hwnd);

        Ok(ReservedArea {
            left: appbar_data.rc.left,
            top: appbar_data.rc.top,
            width,
            height,
            monitor_height: monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top,
        })
    }

    unsafe extern "system" fn appbar_wndproc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == APPBAR_CALLBACK_MESSAGE {
            if wparam.0 as u32 == ABN_POSCHANGED {
                if let Some(window) = get_appbar(hwnd) {
                    if !window.reserving {
                        let _ = unsafe { reserve_top_edge(hwnd, window.height) };
                    }
                }
            }

            return LRESULT(0);
        }

        if message == WM_DISPLAYCHANGE {
            if let Some(window) = get_appbar(hwnd) {
                if !window.reserving {
                    let _ = unsafe { reserve_top_edge(hwnd, window.height) };
                }
            }
        }

        if message == WM_NCDESTROY {
            let result = unsafe {
                call_window_proc(
                    get_appbar(hwnd).map(|window| window.original_wndproc),
                    hwnd,
                    message,
                    wparam,
                    lparam,
                )
            };
            unsafe { cleanup_appbar(hwnd) };
            return result;
        }

        unsafe {
            call_window_proc(
                get_appbar(hwnd).map(|window| window.original_wndproc),
                hwnd,
                message,
                wparam,
                lparam,
            )
        }
    }

    unsafe extern "system" fn flyout_wndproc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_WINDOWPOSCHANGING {
            if let Some(window) = get_flyout(hwnd) {
                let position = lparam.0 as *mut WINDOWPOS;

                if !position.is_null() {
                    unsafe {
                        (*position).x = window.anchor.right - (*position).cx;
                        (*position).y = window.anchor.top;
                    }
                }
            }
        }

        if message == WM_WINDOWPOSCHANGED {
            let _ = unsafe { apply_flyout_region(hwnd) };
        }

        if message == WM_NCDESTROY {
            let result = unsafe {
                call_window_proc(
                    get_flyout(hwnd).map(|window| window.original_wndproc),
                    hwnd,
                    message,
                    wparam,
                    lparam,
                )
            };
            unsafe { cleanup_flyout(hwnd) };
            return result;
        }

        unsafe {
            call_window_proc(
                get_flyout(hwnd).map(|window| window.original_wndproc),
                hwnd,
                message,
                wparam,
                lparam,
            )
        }
    }

    unsafe fn call_window_proc(
        proc: Option<isize>,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if let Some(proc) = proc {
            let proc = unsafe { mem::transmute(proc) };
            unsafe { CallWindowProcW(proc, hwnd, message, wparam, lparam) }
        } else {
            LRESULT(0)
        }
    }

    fn get_appbar(hwnd: HWND) -> Option<AppBarWindow> {
        appbar_windows()
            .lock()
            .ok()
            .and_then(|state| state.get(&hwnd_key(hwnd)).copied())
    }

    fn clear_appbar_reserving(hwnd: HWND) {
        if let Ok(mut state) = appbar_windows().lock() {
            if let Some(window) = state.get_mut(&hwnd_key(hwnd)) {
                window.reserving = false;
            }
        }
    }

    fn get_flyout(hwnd: HWND) -> Option<FlyoutWindow> {
        flyout_windows()
            .lock()
            .ok()
            .and_then(|state| state.get(&hwnd_key(hwnd)).copied())
    }

    unsafe fn set_system_taskbar_hidden(hidden: bool) -> Result<(), String> {
        let mut state = system_taskbar_state()
            .lock()
            .map_err(|_| String::from("System taskbar state lock was poisoned."))?;

        if state.hidden_by_app == hidden {
            return Ok(());
        }

        for hwnd in unsafe { enumerate_system_taskbars() } {
            let _ = unsafe { ShowWindow(hwnd, if hidden { SW_HIDE } else { SW_SHOWNA }) };
        }

        state.hidden_by_app = hidden;
        Ok(())
    }

    unsafe fn enumerate_system_taskbars() -> Vec<HWND> {
        let mut windows = Vec::new();

        if let Ok(hwnd) = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) } {
            if !hwnd.0.is_null() {
                windows.push(hwnd);
            }
        }

        let mut current = HWND(ptr::null_mut());
        while let Ok(next) = unsafe {
            FindWindowExW(
                HWND(ptr::null_mut()),
                current,
                w!("Shell_SecondaryTrayWnd"),
                PCWSTR::null(),
            )
        } {
            if next.0.is_null() {
                break;
            }

            windows.push(next);
            current = next;
        }

        windows
    }

    fn startup_command(
        startup_mode: crate::config::StartupMode,
        config_path: &std::path::Path,
    ) -> Result<String, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("Failed to resolve the current executable path: {error}"))?;
        let path = executable
            .to_str()
            .ok_or_else(|| String::from("The current executable path is not valid UTF-8."))?;
        let config_path = config_path
            .to_str()
            .ok_or_else(|| String::from("The config path is not valid UTF-8."))?;

        let mut command = format!("\"{path}\"");
        command.push_str(&format!(" --config \"{config_path}\""));

        if let Some(argument) = startup_mode.as_startup_arg() {
            command.push(' ');
            command.push_str(argument);
        }

        Ok(command)
    }

    fn utf16_bytes(value: &str) -> Vec<u8> {
        value.encode_utf16()
            .chain(Some(0))
            .flat_map(|unit| unit.to_le_bytes())
            .collect()
    }

    unsafe fn cleanup_appbar(hwnd: HWND) {
        let Some(window) = appbar_windows()
            .lock()
            .ok()
            .and_then(|mut state| state.remove(&hwnd_key(hwnd)))
        else {
            return;
        };

        if window.registered {
            let mut appbar_data = new_appbar_data(hwnd, window.height);
            unsafe { SHAppBarMessage(ABM_REMOVE, &mut appbar_data) };
        }

        let _ = unsafe { set_system_taskbar_hidden(false) };
        let _ = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, window.original_wndproc) };
    }

    unsafe fn cleanup_flyout(hwnd: HWND) {
        let Some(window) = flyout_windows()
            .lock()
            .ok()
            .and_then(|mut state| state.remove(&hwnd_key(hwnd)))
        else {
            return;
        };

        let _ = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, window.original_wndproc) };
    }

    fn new_appbar_data(hwnd: HWND, height: i32) -> APPBARDATA {
        APPBARDATA {
            cbSize: mem::size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            uCallbackMessage: APPBAR_CALLBACK_MESSAGE,
            uEdge: ABE_TOP,
            rc: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: height,
            },
            ..Default::default()
        }
    }

    fn last_error(context: &str) -> String {
        format!("{context}: {}", Error::from_win32())
    }
}
