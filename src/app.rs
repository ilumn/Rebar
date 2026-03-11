use crate::{
    config::{AppConfig, PaletteMode, StartupMode},
    native::{FlyoutAnchor, ReservedArea},
    palette::{PaletteVariants, WallpaperPalette, WallpaperSignature},
    system::{SystemCommand, SystemSnapshot},
    ui, widgets,
};
use iced::{Animation, Element, Font, Point, Size, Subscription, Task, time, window};
use iced_plot::{PlotUiMessage, PlotWidget};
use lucide_icons::LUCIDE_FONT_BYTES;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use widgets::{WidgetHistory, WidgetKind};

pub(crate) const BAR_RADIUS: i32 = 8;
pub(crate) const FLYOUT_GAP: i32 = 4;
const FLYOUT_MIN_WIDTH: f32 = 260.0;
const FLYOUT_MIN_HEIGHT: f32 = 180.0;
pub(crate) const FLYOUT_RADIUS: i32 = 8;
const FLYOUT_SHELL_HORIZONTAL_PADDING: f32 = 36.0;
const FLYOUT_SHELL_VERTICAL_PADDING: f32 = 36.0;
const JETBRAINS_MONO: Font = Font::with_name("JetBrains Mono");
const JETBRAINS_MONO_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono[wght].ttf");

pub(crate) struct Rebar {
    pub(crate) bar_id: window::Id,
    pub(crate) flyout_id: Option<window::Id>,
    pub(crate) status: String,
    pub(crate) flyout_size: Size,
    pub(crate) flyout_window_size: Size,
    pub(crate) flyout_reveal_height: f32,
    pub(crate) flyout_anchor: Option<FlyoutAnchor>,
    pub(crate) flyout_target_open: bool,
    pub(crate) flyout_visible: bool,
    flyout_generation: u64,
    pub(crate) flyout_animation: Animation<bool>,
    flyout_animation_duration: Duration,
    bar_position: Option<Point>,
    bar_size: Size,
    monitor_height: f32,
    pub(crate) bar_height: f32,
    pub(crate) palette_mode: PaletteMode,
    hide_windows_taskbar: bool,
    auto_hide_panels_on_focus_loss: bool,
    launch_mode: StartupMode,
    pub(crate) palette: WallpaperPalette,
    pub(crate) palette_variants: PaletteVariants,
    wallpaper_signature: Option<WallpaperSignature>,
    wallpaper_reload_pending: bool,
    pub(crate) system: SystemSnapshot,
    pub(crate) active_widget: Option<WidgetKind>,
    pub(crate) widget_history: WidgetHistory,
    pub(crate) system_plot: PlotWidget,
    pub(crate) network_plot: PlotWidget,
    media_progress: Option<MediaProgressState>,
}

#[derive(Debug, Clone)]
struct MediaProgressState {
    session_key: String,
    position_seconds: f64,
    duration_seconds: Option<f64>,
    playback: crate::system::MediaPlaybackState,
    anchor_instant: Instant,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    BarOpened(window::Id),
    BarConfigured(Result<ReservedArea, String>),
    WallpaperPaletteLoaded(Result<(WallpaperSignature, PaletteVariants), String>),
    WallpaperCheck,
    WallpaperSignatureLoaded(Result<WallpaperSignature, String>),
    SystemPoll,
    SystemRefreshed(Result<SystemSnapshot, String>),
    SystemCommand(SystemCommand),
    SystemCommandFinished(Result<(), String>),
    MediaPulse,
    WidgetSelected(WidgetKind),
    PlotMessage(WidgetKind, PlotUiMessage),
    FlyoutOpened(window::Id),
    FlyoutConfigured(Result<(), String>),
    Tick(Instant),
    WindowEvent(window::Id, window::Event),
}

impl Rebar {
    fn boot(config: AppConfig, launch_mode: StartupMode) -> (Self, Task<Message>) {
        let system = SystemSnapshot::default();
        let widget_history = WidgetHistory::default();
        let initial_bar_height = 40.0;
        let flyout_size = Size::new(640.0, 420.0);
        let (bar_id, open_task) = window::open(window::Settings {
            size: Size::new(900.0, initial_bar_height),
            visible: false,
            decorations: false,
            transparent: true,
            resizable: false,
            minimizable: false,
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: true,
            ..window::Settings::default()
        });

        let mut state = Self {
            bar_id,
            flyout_id: None,
            status: String::from("Reserving desktop space..."),
            flyout_size,
            flyout_window_size: flyout_size,
            flyout_reveal_height: 1.0,
            flyout_anchor: None,
            flyout_target_open: false,
            flyout_visible: false,
            flyout_generation: 0,
            flyout_animation: flyout_animation(config.flyout_animation_ms, false),
            flyout_animation_duration: Duration::from_millis(config.flyout_animation_ms.max(1)),
            bar_position: None,
            bar_size: Size::new(900.0, initial_bar_height),
            monitor_height: 1080.0,
            bar_height: initial_bar_height,
            palette_mode: config.palette_mode,
            hide_windows_taskbar: config.hide_windows_taskbar,
            auto_hide_panels_on_focus_loss: config.auto_hide_panels_on_focus_loss,
            launch_mode,
            palette: WallpaperPalette::default(),
            palette_variants: PaletteVariants::default(),
            wallpaper_signature: None,
            wallpaper_reload_pending: true,
            system,
            active_widget: None,
            widget_history,
            system_plot: PlotWidget::default(),
            network_plot: PlotWidget::default(),
            media_progress: None,
        };
        state.refresh_widget_models();
        state.flyout_size = state.desired_flyout_size();
        state.flyout_window_size = state.flyout_size;

        (
            state,
            Task::batch([
                open_task.map(Message::BarOpened),
                load_wallpaper_palette(),
                refresh_system_snapshot(),
            ]),
        )
    }

    fn desired_flyout_size(&self) -> Size {
        let widget = self.active_widget.unwrap_or(WidgetKind::System);
        let spec = widgets::panel_spec(self, widget);
        let max_height = self.max_flyout_height();

        Size::new(
            (spec.preferred_size.width + FLYOUT_SHELL_HORIZONTAL_PADDING)
                .max(spec.min_size.width + FLYOUT_SHELL_HORIZONTAL_PADDING)
                .max(FLYOUT_MIN_WIDTH),
            (spec.preferred_size.height + FLYOUT_SHELL_VERTICAL_PADDING)
                .max(spec.min_size.height + FLYOUT_SHELL_VERTICAL_PADDING)
                .max(FLYOUT_MIN_HEIGHT)
                .min(max_height),
        )
    }

    pub(crate) fn max_flyout_height(&self) -> f32 {
        (self.monitor_height * 0.8).max(FLYOUT_MIN_HEIGHT)
    }

    fn desired_bar_height(&self) -> f32 {
        let _ = self;
        40.0
    }

    fn refresh_widget_models(&mut self) {
        self.system_plot = widgets::system::build_plot(self);
        self.network_plot = widgets::network::build_plot(self);
    }

    pub(crate) fn current_media_timing(&self) -> Option<(f64, f64)> {
        let progress = self.media_progress.as_ref()?;
        let duration = progress.duration_seconds?;
        let elapsed = if progress.playback == crate::system::MediaPlaybackState::Playing {
            progress.anchor_instant.elapsed().as_secs_f64()
        } else {
            0.0
        };

        Some(((progress.position_seconds + elapsed).min(duration), duration))
    }

    fn select_widget(&mut self, widget: WidgetKind) -> Task<Message> {
        let is_same_open =
            self.active_widget == Some(widget) && (self.flyout_target_open || self.flyout_visible);

        self.active_widget = Some(widget);
        self.flyout_size = self.desired_flyout_size();
        self.flyout_window_size = self.flyout_size;
        self.update_flyout_anchor();

        if is_same_open {
            return self.close_flyout();
        }

        self.open_flyout()
    }

    fn open_flyout(&mut self) -> Task<Message> {
        let Some(_anchor) = self.flyout_anchor else {
            self.status = String::from("The bar anchor is not ready yet.");
            return Task::none();
        };

        let now = Instant::now();
        self.flyout_target_open = true;
        self.flyout_generation = self.next_flyout_generation();
        self.flyout_animation.go_mut(true, now);
        self.flyout_reveal_height = self.animated_flyout_reveal_height(now);

        if let Some(id) = self.flyout_id {
            self.flyout_visible = true;
            return window::resize(id, self.flyout_size).chain(self.sync_flyout_native(
                id,
                true,
                self.flyout_generation,
                self.flyout_reveal_height,
            ));
        }

        let (id, open_task) = window::open(window::Settings {
            size: self.flyout_size,
            min_size: Some(Size::new(FLYOUT_MIN_WIDTH, FLYOUT_MIN_HEIGHT)),
            visible: false,
            decorations: false,
            transparent: true,
            resizable: true,
            minimizable: false,
            level: window::Level::AlwaysOnTop,
            exit_on_close_request: false,
            ..window::Settings::default()
        });

        self.flyout_id = Some(id);
        open_task.map(Message::FlyoutOpened)
    }

    fn close_flyout(&mut self) -> Task<Message> {
        let Some(id) = self.flyout_id else {
            return Task::none();
        };

        let now = Instant::now();
        self.flyout_target_open = false;
        self.flyout_generation = self.next_flyout_generation();
        self.flyout_animation.go_mut(false, now);
        self.flyout_reveal_height = self.animated_flyout_reveal_height(now);

        self.sync_flyout_native(id, true, self.flyout_generation, self.flyout_reveal_height)
    }

    fn sync_flyout_size(&mut self) -> Task<Message> {
        self.flyout_size = self.desired_flyout_size();
        self.flyout_window_size = self.flyout_size;
        self.update_flyout_anchor();

        let Some(id) = self.flyout_id else {
            return Task::none();
        };

        let now = Instant::now();
        self.flyout_reveal_height = self.animated_flyout_reveal_height(now);

        if self.flyout_visible {
            window::resize(id, self.flyout_size).chain(self.sync_flyout_native(
                id,
                true,
                self.flyout_generation,
                self.flyout_reveal_height,
            ))
        } else {
            window::resize(id, self.flyout_size)
        }
    }

    fn sync_bar_height(&mut self) -> Task<Message> {
        let desired = self.desired_bar_height();

        if (self.bar_height - desired).abs() < 0.5 {
            return Task::none();
        }

        self.bar_height = desired;
        self.bar_size.height = desired;

        let resize = window::resize(self.bar_id, Size::new(self.bar_size.width.max(900.0), desired));

        #[cfg(target_os = "windows")]
        {
            let hide_windows_taskbar = self.hide_windows_taskbar;
            let launch_in_background = self.launch_mode == StartupMode::Background;
            return resize.chain(
                window::run(self.bar_id, move |native_window| unsafe {
                    crate::native::install_appbar(
                        native_window,
                        desired.round() as i32,
                        hide_windows_taskbar,
                        launch_in_background,
                    )
                })
                .map(Message::BarConfigured),
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            resize
        }
    }

    fn sync_media_progress(&mut self) {
        let now = Instant::now();
        let Some(media) = &self.system.media else {
            self.media_progress = None;
            return;
        };

        let session_key = format!("{}|{}|{}", media.source_app_id, media.title, media.artist);
        let reported_position = media.position_seconds.unwrap_or(0.0);

        match &mut self.media_progress {
            Some(progress) if progress.session_key == session_key => {
                let predicted = if progress.playback == crate::system::MediaPlaybackState::Playing {
                    progress.position_seconds + progress.anchor_instant.elapsed().as_secs_f64()
                } else {
                    progress.position_seconds
                };
                let backward_delta = predicted - reported_position;
                let looks_like_seek_back = backward_delta > 5.0;

                progress.position_seconds = if progress.playback
                    == crate::system::MediaPlaybackState::Playing
                    && media.playback == crate::system::MediaPlaybackState::Playing
                    && !looks_like_seek_back
                {
                    predicted.max(reported_position)
                } else {
                    reported_position
                };
                progress.duration_seconds = media.duration_seconds;
                progress.playback = media.playback;
                progress.anchor_instant = now;
            }
            _ => {
                self.media_progress = Some(MediaProgressState {
                    session_key,
                    position_seconds: reported_position,
                    duration_seconds: media.duration_seconds,
                    playback: media.playback,
                    anchor_instant: now,
                });
            }
        }
    }

    fn sync_flyout_native(
        &self,
        id: window::Id,
        visible: bool,
        generation: u64,
        reveal_height: f32,
    ) -> Task<Message> {
        let Some(anchor) = self.flyout_anchor else {
            return Task::none();
        };

        let size = self.flyout_size;

        #[cfg(target_os = "windows")]
        {
            return window::run(id, move |native_window| unsafe {
                crate::native::sync_flyout(
                    native_window,
                    anchor,
                    size,
                    reveal_height,
                    visible,
                    generation,
                )
            })
            .map(Message::FlyoutConfigured);
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = id;
            Task::none()
        }
    }

    fn next_flyout_generation(&mut self) -> u64 {
        self.flyout_generation = self.flyout_generation.wrapping_add(1);

        if self.flyout_generation == 0 {
            self.flyout_generation = 1;
        }

        self.flyout_generation
    }

    fn update_bar_bounds(&mut self, position: Option<Point>, size: Option<Size>) {
        if let Some(position) = position {
            self.bar_position = Some(position);
        }

        if let Some(size) = size {
            self.bar_size = size;
        }

        self.update_flyout_anchor();
    }

    fn update_flyout_anchor(&mut self) {
        let Some(position) = self.bar_position else {
            return;
        };

        let right = if self.active_widget == Some(WidgetKind::Device) {
            position.x.round() as i32 + self.flyout_size.width.round() as i32
        } else {
            (position.x + self.bar_size.width).round() as i32
        };

        self.flyout_anchor = Some(FlyoutAnchor {
            right,
            top: (position.y + self.bar_size.height).round() as i32 + FLYOUT_GAP,
        });
    }

    fn animated_flyout_reveal_height(&self, now: Instant) -> f32 {
        let progress = self
            .flyout_animation
            .interpolate(0.0_f32, 1.0_f32, now)
            .clamp(0.0, 1.0);

        (self.flyout_size.height * progress).max(1.0)
    }

    fn refresh_visible_flyout(&mut self, now: Instant) -> Task<Message> {
        let Some(id) = self.flyout_id else {
            return Task::none();
        };

        if !self.flyout_visible {
            return Task::none();
        }

        self.flyout_reveal_height = self.animated_flyout_reveal_height(now);

        self.sync_flyout_native(id, true, self.flyout_generation, self.flyout_reveal_height)
    }
}

fn refresh_system_snapshot() -> Task<Message> {
    Task::perform(async { crate::system::refresh_snapshot() }, Message::SystemRefreshed)
}

fn run_system_command(command: SystemCommand) -> Task<Message> {
    Task::perform(
        async move { crate::system::apply_command(command) },
        Message::SystemCommandFinished,
    )
}

fn flyout_animation(duration_ms: u64, is_open: bool) -> Animation<bool> {
    Animation::new(is_open).duration(Duration::from_millis(duration_ms.max(1)))
}

pub(crate) fn run() -> iced::Result {
    let config_path = current_config_path();
    if let Err(error) = AppConfig::ensure_default_at_path(&config_path) {
        eprintln!("Failed to create default config: {error}");
    }
    let config = AppConfig::load_from_path(&config_path).unwrap_or_default();
    let (launch_mode, detached_child) = current_launch_mode(config.startup_mode);

    #[cfg(target_os = "windows")]
    if let Err(error) = crate::native::sync_startup_registration(
        config.launch_on_startup,
        config.startup_mode,
        &config_path,
    ) {
        eprintln!("Failed to sync startup registration: {error}");
    }

    #[cfg(target_os = "windows")]
    if launch_mode == StartupMode::Background && !detached_child {
        if let Err(error) = spawn_detached_background_process(&config_path) {
            eprintln!("Failed to launch detached background process: {error}");
        } else {
            return Ok(());
        }
    }

    iced::daemon(move || Rebar::boot(config, launch_mode), update, view)
        .font(JETBRAINS_MONO_BYTES)
        .font(LUCIDE_FONT_BYTES)
        .default_font(JETBRAINS_MONO)
        .subscription(subscription)
        .run()
}

fn update(state: &mut Rebar, message: Message) -> Task<Message> {
    match message {
        Message::BarOpened(id) => {
            if id != state.bar_id {
                return Task::none();
            }

            #[cfg(target_os = "windows")]
            {
                let bar_height = state.bar_height.round() as i32;
                let hide_windows_taskbar = state.hide_windows_taskbar;
                let launch_in_background = state.launch_mode == StartupMode::Background;
                return window::run(id, move |native_window| unsafe {
                    crate::native::install_appbar(
                        native_window,
                        bar_height,
                        hide_windows_taskbar,
                        launch_in_background,
                    )
                })
                .map(Message::BarConfigured);
            }

            #[cfg(not(target_os = "windows"))]
            {
                Task::none()
            }
        }
        Message::BarConfigured(Ok(area)) => {
            state.bar_position = Some(Point::new(area.left as f32, area.top as f32));
            state.bar_size = Size::new(area.width as f32, area.height as f32);
            if area.monitor_height > 0 {
                state.monitor_height = area.monitor_height as f32;
            }
            state.update_flyout_anchor();
            state.status = format!(
                "Reserved {}x{} at ({}, {}).",
                area.width, area.height, area.left, area.top
            );

            state.refresh_visible_flyout(Instant::now())
        }
        Message::BarConfigured(Err(error)) => {
            state.status = error;
            Task::none()
        }
        Message::WallpaperPaletteLoaded(Ok(variants)) => {
            let (signature, variants) = variants;
            state.palette = variants.select(state.palette_mode);
            state.palette_variants = variants;
            state.wallpaper_signature = Some(signature);
            state.wallpaper_reload_pending = false;
            state.refresh_widget_models();
            Task::batch([state.sync_bar_height(), state.sync_flyout_size()])
        }
        Message::WallpaperPaletteLoaded(Err(error)) => {
            state.wallpaper_reload_pending = false;
            state.status = format!("{error} Using fallback palette.");
            Task::none()
        }
        Message::WallpaperCheck => {
            if state.wallpaper_reload_pending {
                Task::none()
            } else {
                load_wallpaper_signature()
            }
        }
        Message::WallpaperSignatureLoaded(Ok(signature)) => {
            let changed = state
                .wallpaper_signature
                .as_ref()
                .map(|current| current != &signature)
                .unwrap_or(true);

            if changed {
                state.wallpaper_reload_pending = true;
                load_wallpaper_palette()
            } else {
                Task::none()
            }
        }
        Message::WallpaperSignatureLoaded(Err(_error)) => Task::none(),
        Message::SystemPoll => refresh_system_snapshot(),
        Message::SystemRefreshed(Ok(snapshot)) => {
            state.system = snapshot;
            state.sync_media_progress();
            state.widget_history.observe(&state.system);
            state.refresh_widget_models();
            Task::batch([state.sync_bar_height(), state.sync_flyout_size()])
        }
        Message::SystemRefreshed(Err(error)) => {
            state.status = format!("System info refresh failed: {error}");
            Task::none()
        }
        Message::SystemCommand(command) => run_system_command(command),
        Message::SystemCommandFinished(Ok(())) => refresh_system_snapshot(),
        Message::SystemCommandFinished(Err(error)) => {
            state.status = format!("System command failed: {error}");
            refresh_system_snapshot()
        }
        Message::MediaPulse => Task::none(),
        Message::WidgetSelected(widget) => state.select_widget(widget),
        Message::PlotMessage(widget, plot_message) => {
            match widget {
                WidgetKind::System => state.system_plot.update(plot_message),
                WidgetKind::Network => state.network_plot.update(plot_message),
                WidgetKind::Audio | WidgetKind::Media | WidgetKind::Device => {}
            }
            Task::none()
        }
        Message::FlyoutOpened(id) => {
            state.flyout_id = Some(id);

            if state.flyout_target_open {
                state.flyout_visible = true;
                state.flyout_reveal_height = state.animated_flyout_reveal_height(Instant::now());
                return state.sync_flyout_native(
                    id,
                    true,
                    state.flyout_generation,
                    state.flyout_reveal_height,
                );
            }

            Task::none()
        }
        Message::FlyoutConfigured(Ok(())) => Task::none(),
        Message::FlyoutConfigured(Err(error)) => {
            state.status = error;
            Task::none()
        }
        Message::Tick(now) => {
            let Some(id) = state.flyout_id else {
                return Task::none();
            };

            state.flyout_reveal_height = state.animated_flyout_reveal_height(now);

            if state.flyout_animation.is_animating(now) {
                return state.sync_flyout_native(
                    id,
                    true,
                    state.flyout_generation,
                    state.flyout_reveal_height,
                );
            }

            if state.flyout_target_open && state.flyout_visible {
                return state.sync_flyout_native(
                    id,
                    true,
                    state.flyout_generation,
                    state.flyout_reveal_height,
                );
            }

            if !state.flyout_target_open && state.flyout_visible {
                state.flyout_visible = false;
                return state.sync_flyout_native(
                    id,
                    false,
                    state.flyout_generation,
                    state.flyout_reveal_height,
                );
            }

            Task::none()
        }
        Message::WindowEvent(id, event) => {
            if id == state.bar_id {
                match event {
                    window::Event::Opened { position, size } => {
                        state.update_bar_bounds(position, Some(size));
                        return state.refresh_visible_flyout(Instant::now());
                    }
                    window::Event::Moved(position) => {
                        state.update_bar_bounds(Some(position), None);
                        return state.refresh_visible_flyout(Instant::now());
                    }
                    window::Event::Resized(size) => {
                        state.update_bar_bounds(None, Some(size));
                        return state.refresh_visible_flyout(Instant::now());
                    }
                    _ => {}
                }
            }

            if Some(id) == state.flyout_id {
                match event {
                    window::Event::Opened { size, .. } | window::Event::Resized(size) => {
                        state.flyout_window_size = size;
                        let resized_size = Size::new(
                            size.width.max(FLYOUT_MIN_WIDTH),
                            size.height.max(FLYOUT_MIN_HEIGHT),
                        );

                        if !state.flyout_animation.is_animating(Instant::now()) {
                            state.flyout_size = resized_size;

                            if state.flyout_target_open || state.flyout_visible {
                                state.flyout_reveal_height = state.flyout_size.height;
                            } else {
                                state.flyout_reveal_height = 1.0;
                            }
                        }
                    }
                    window::Event::Closed => {
                        state.flyout_id = None;
                        state.flyout_target_open = false;
                        state.flyout_visible = false;
                        state.flyout_animation = Animation::new(false)
                            .duration(state.flyout_animation_duration);
                        state.flyout_window_size = state.flyout_size;
                        state.flyout_reveal_height = 1.0;
                    }
                    window::Event::CloseRequested => {
                        if state.flyout_target_open {
                            return state.close_flyout();
                        }
                    }
                    window::Event::Unfocused => {
                        if state.auto_hide_panels_on_focus_loss && state.flyout_target_open {
                            return state.close_flyout();
                        }
                    }
                    _ => {}
                }
            }

            Task::none()
        }
    }
}

fn view(state: &Rebar, id: window::Id) -> Element<'_, Message> {
    if id == state.bar_id {
        ui::view_bar(state)
    } else {
        ui::view_flyout(state)
    }
}

fn subscription(state: &Rebar) -> Subscription<Message> {
    let window_events = window::events().map(|(id, event)| Message::WindowEvent(id, event));
    let system_polls = time::every(Duration::from_secs(1)).map(|_| Message::SystemPoll);
    let wallpaper_checks = time::every(Duration::from_secs(5)).map(|_| Message::WallpaperCheck);
    let media_pulse = if matches!(
        state.system.media.as_ref().map(|media| media.playback),
        Some(crate::system::MediaPlaybackState::Playing)
    ) {
        Some(time::every(Duration::from_millis(250)).map(|_| Message::MediaPulse))
    } else {
        None
    };

    if state.flyout_animation.is_animating(Instant::now())
        || (!state.flyout_target_open && state.flyout_visible)
    {
        let mut subscriptions =
            vec![
                window_events,
                system_polls,
                wallpaper_checks,
                window::frames().map(Message::Tick),
            ];
        if let Some(media_pulse) = media_pulse {
            subscriptions.push(media_pulse);
        }
        Subscription::batch(subscriptions)
    } else {
        let mut subscriptions = vec![window_events, system_polls, wallpaper_checks];
        if let Some(media_pulse) = media_pulse {
            subscriptions.push(media_pulse);
        }
        Subscription::batch(subscriptions)
    }
}

#[cfg(target_os = "windows")]
fn load_wallpaper_palette() -> Task<Message> {
    Task::perform(
        async { crate::palette::sample_palettes() },
        Message::WallpaperPaletteLoaded,
    )
}

#[cfg(target_os = "windows")]
fn load_wallpaper_signature() -> Task<Message> {
    Task::perform(
        async { crate::palette::current_wallpaper_signature() },
        Message::WallpaperSignatureLoaded,
    )
}

#[cfg(not(target_os = "windows"))]
fn load_wallpaper_palette() -> Task<Message> {
    Task::none()
}

#[cfg(not(target_os = "windows"))]
fn load_wallpaper_signature() -> Task<Message> {
    Task::none()
}

fn current_launch_mode(default_mode: StartupMode) -> (StartupMode, bool) {
    let mut launch_mode = default_mode;
    let mut detached_child = false;

    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--background" => launch_mode = StartupMode::Background,
            "--foreground" => launch_mode = StartupMode::Foreground,
            "--detached-child" => detached_child = true,
            "--config" => {
                let _ = args.next();
            }
            _ => {}
        }
    }

    (launch_mode, detached_child)
}

#[cfg(target_os = "windows")]
fn spawn_detached_background_process(config_path: &Path) -> Result<(), String> {
    use std::{env, os::windows::process::CommandExt, process::Command};

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    let current_exe = env::current_exe()
        .map_err(|error| format!("Failed to resolve the current executable path: {error}"))?;

    let mut passthrough_args = Vec::new();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        if arg == "--background" || arg == "--foreground" || arg == "--detached-child" {
            continue;
        }

        if arg == "--config" {
            let _ = args.next();
            continue;
        }

        passthrough_args.push(arg);
    }

    let mut command = Command::new(current_exe);
    command
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .current_dir(
            config_path
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )
        .args(passthrough_args)
        .arg("--config")
        .arg(config_path)
        .arg("--background")
        .arg("--detached-child");

    command
        .spawn()
        .map_err(|error| format!("Failed to spawn detached background process: {error}"))?;

    Ok(())
}

fn current_config_path() -> PathBuf {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if arg == "--config" {
            if let Some(path) = args.next() {
                return PathBuf::from(path);
            }
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("rebar.toml")
}
