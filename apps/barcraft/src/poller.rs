//! A background thread that reads the system (windows, load, battery, volume, network, media,
//! weather, icons) so the dock's drawing never waits on Windows or the network.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use barcraft_core::apps::display_name;
use barcraft_core::cpu::{CpuTimes, History, busy_fraction};
use barcraft_core::settings::WeatherSettings;
use barcraft_core::usage::DayUsage;
use barcraft_core::weather::{self, Current, Place};
use barcraft_core::{TaskbarMode, TempUnit, WidgetKind};
use barcraft_win32::{
    DiskSpace, IconRgba, MediaAction, MediaStatus, MemoryStatus, NetCounters, PowerStatus, PxRect,
    RecycleBinStatus, VolumeStatus, WindowId, WindowInfo,
};

const TICK: Duration = Duration::from_millis(250);
const WINDOWS_EVERY: Duration = Duration::from_millis(500);
const STATS_EVERY: Duration = Duration::from_secs(1);
const MEDIA_EVERY: Duration = Duration::from_secs(2);
const BIN_EVERY: Duration = Duration::from_secs(5);
const DISK_EVERY: Duration = Duration::from_secs(30);
const USAGE_SAVE_EVERY: Duration = Duration::from_secs(60);
const WEATHER_EVERY: Duration = Duration::from_secs(15 * 60);
const WEATHER_RETRY: Duration = Duration::from_secs(60);
/// Time in front only counts while someone is at the PC.
const IDLE_LIMIT_SECS: u32 = 120;
/// How long the taskbar stays up after the tray tile is clicked, before the pointer reaches it.
const TRAY_GRACE: Duration = Duration::from_secs(4);
const TRAY_LINGER: Duration = Duration::from_millis(1500);
const ICON_SIDE: u32 = 96;
const NET_HISTORY: usize = 30;

#[derive(Clone, Debug, PartialEq)]
pub enum WeatherState {
    Off,
    Loading,
    Ready { current: Current, at: Instant },
    Failed(String),
}

/// What the dock reads each frame.
pub struct Snapshot {
    pub windows: Vec<WindowInfo>,
    pub foreground: Option<WindowId>,
    pub cpu: History,
    pub memory: Option<MemoryStatus>,
    pub power: Option<PowerStatus>,
    pub volume: Option<VolumeStatus>,
    pub microphone: Option<VolumeStatus>,
    pub media: Option<MediaStatus>,
    /// Bytes per second, newest last.
    pub net_down: VecDeque<f64>,
    pub net_up: f64,
    pub disk: Option<DiskSpace>,
    pub recycle_bin: Option<RecycleBinStatus>,
    pub usage: DayUsage,
    pub weather: WeatherState,
    /// A place the weather lookup resolved, for the app to store in settings.
    pub resolved_place: Option<Place>,
    /// Icons loaded since the dock last took them.
    pub icons: VecDeque<(String, Option<IconRgba>)>,
    pub hidden_for_fullscreen: bool,
    /// Whether Windows is in light mode, for the "Follow Windows" theme.
    pub windows_light: bool,
}

/// What the poller needs to know about the dock.
#[derive(Clone, Debug)]
pub struct DockWatch {
    pub window: Option<WindowId>,
    /// The blurred backdrop under the bar, hidden and shown with the dock.
    pub backdrop: Option<WindowId>,
    pub monitor: Option<PxRect>,
    pub hide_for_fullscreen: bool,
    pub taskbar_mode: TaskbarMode,
    pub widgets: Vec<WidgetKind>,
}

impl DockWatch {
    fn wants(&self, kind: WidgetKind) -> bool {
        self.widgets.contains(&kind)
    }
}

pub enum Command {
    LoadIcon {
        key: String,
        file: Option<PathBuf>,
        window: Option<WindowId>,
    },
    Weather(WeatherSettings),
    SetVolume(f32),
    SetMuted(bool),
    SetMicrophoneMuted(bool),
    Media(MediaAction),
    ShowTray,
    EmptyRecycleBin,
    ClearUsage,
    Watch(DockWatch),
}

pub struct Poller {
    shared: Arc<Mutex<Snapshot>>,
    tx: Sender<Command>,
}

impl Poller {
    pub fn start(repaint: egui::Context, usage_path: PathBuf) -> Poller {
        let shared = Arc::new(Mutex::new(Snapshot {
            windows: Vec::new(),
            foreground: None,
            cpu: History::new(60),
            memory: None,
            power: None,
            volume: None,
            microphone: None,
            media: None,
            net_down: VecDeque::with_capacity(NET_HISTORY),
            net_up: 0.0,
            disk: None,
            recycle_bin: None,
            usage: DayUsage::load(&usage_path),
            weather: WeatherState::Off,
            resolved_place: None,
            icons: VecDeque::new(),
            hidden_for_fullscreen: false,
            windows_light: barcraft_win32::system_uses_light_theme().unwrap_or(false),
        }));
        let (tx, rx) = channel();
        let thread_shared = Arc::clone(&shared);
        let spawned = std::thread::Builder::new()
            .name("barcraft-poller".into())
            .spawn(move || run(thread_shared, rx, repaint, usage_path));
        if let Err(e) = spawned {
            log::error!("could not start the poller thread: {e}");
        }
        Poller { shared, tx }
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    /// The latest readings. A poisoned lock still holds usable data, so it is recovered.
    pub fn snapshot(&self) -> MutexGuard<'_, Snapshot> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn lock(shared: &Mutex<Snapshot>) -> MutexGuard<'_, Snapshot> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct WeatherJob {
    settings: WeatherSettings,
    next: Instant,
}

/// The poller's private state between ticks.
struct State {
    last_windows: Instant,
    last_stats: Instant,
    last_media: Instant,
    last_bin: Instant,
    last_disk: Instant,
    last_usage_save: Instant,
    cpu_before: Option<CpuTimes>,
    net_before: Option<(NetCounters, Instant)>,
    usage_carry: f64,
    usage_last: Instant,
    usage_dirty: bool,
    tray_until: Option<Instant>,
    weather: Option<WeatherJob>,
}

fn run(
    shared: Arc<Mutex<Snapshot>>,
    rx: Receiver<Command>,
    repaint: egui::Context,
    usage_path: PathBuf,
) {
    let own_pid = std::process::id();
    let start = Instant::now();
    let long_ago = |d: Duration| start.checked_sub(d).unwrap_or(start);
    let mut st = State {
        last_windows: long_ago(WINDOWS_EVERY),
        last_stats: long_ago(STATS_EVERY),
        last_media: long_ago(MEDIA_EVERY),
        last_bin: long_ago(BIN_EVERY),
        last_disk: long_ago(DISK_EVERY),
        last_usage_save: start,
        cpu_before: None,
        net_before: None,
        usage_carry: 0.0,
        usage_last: start,
        usage_dirty: false,
        tray_until: None,
        weather: None,
    };
    let mut watch = DockWatch {
        window: None,
        backdrop: None,
        monitor: None,
        hide_for_fullscreen: true,
        taskbar_mode: TaskbarMode::Untouched,
        widgets: Vec::new(),
    };
    let agent = http_agent();

    loop {
        // Handle every queued command, waiting up to one tick for the first.
        let mut first = true;
        loop {
            let cmd = if first {
                first = false;
                match rx.recv_timeout(TICK) {
                    Ok(c) => c,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => {
                        save_usage(&shared, &usage_path, &mut st);
                        return;
                    }
                }
            } else {
                match rx.try_recv() {
                    Ok(c) => c,
                    Err(_) => break,
                }
            };
            handle(cmd, &shared, &repaint, &mut st, &mut watch);
        }

        let now = Instant::now();
        let mut changed = false;

        if now.duration_since(st.last_windows) >= WINDOWS_EVERY {
            st.last_windows = now;
            changed |= poll_windows(&shared, &watch, own_pid);
            track_usage(&shared, &watch, &mut st, now);
            update_tray_peek(&mut st, now);
        }

        if now.duration_since(st.last_stats) >= STATS_EVERY {
            st.last_stats = now;
            poll_stats(&shared, &watch, &mut st, now);
            if st.tray_until.is_none() {
                crate::taskbar::keep_applied(watch.taskbar_mode);
            }
            changed = true;
        }

        if watch.wants(WidgetKind::Media) && now.duration_since(st.last_media) >= MEDIA_EVERY {
            st.last_media = now;
            let media = barcraft_win32::media_status();
            let mut snap = lock(&shared);
            if snap.media != media {
                snap.media = media;
                changed = true;
            }
        }

        if watch.wants(WidgetKind::RecycleBin) && now.duration_since(st.last_bin) >= BIN_EVERY {
            st.last_bin = now;
            let bin = barcraft_win32::recycle_bin();
            let mut snap = lock(&shared);
            if snap.recycle_bin != bin {
                snap.recycle_bin = bin;
                changed = true;
            }
        }

        if watch.wants(WidgetKind::Storage) && now.duration_since(st.last_disk) >= DISK_EVERY {
            st.last_disk = now;
            lock(&shared).disk = barcraft_win32::system_drive_space();
            changed = true;
        }

        if st.usage_dirty && now.duration_since(st.last_usage_save) >= USAGE_SAVE_EVERY {
            save_usage(&shared, &usage_path, &mut st);
        }

        if let Some(job) = st.weather.as_mut()
            && now >= job.next
        {
            let (state, place) = fetch_weather(&agent, &job.settings);
            if let Some(p) = &place {
                job.settings.latitude = Some(p.latitude);
                job.settings.longitude = Some(p.longitude);
                job.settings.resolved_name = p.name.clone();
            }
            job.next = now
                + if matches!(state, WeatherState::Ready { .. }) {
                    WEATHER_EVERY
                } else {
                    WEATHER_RETRY
                };
            let mut snap = lock(&shared);
            snap.weather = state;
            if place.is_some() {
                snap.resolved_place = place;
            }
            changed = true;
        }

        if changed {
            repaint.request_repaint();
        }
    }
}

fn handle(
    cmd: Command,
    shared: &Mutex<Snapshot>,
    repaint: &egui::Context,
    st: &mut State,
    watch: &mut DockWatch,
) {
    match cmd {
        Command::LoadIcon { key, file, window } => {
            let icon = file
                .as_deref()
                .and_then(|f| barcraft_win32::icon_for_file(f, ICON_SIDE))
                .or_else(|| window.and_then(barcraft_win32::icon_for_window));
            lock(shared).icons.push_back((key, icon));
        }
        Command::Weather(settings) => {
            let off = settings.city.trim().is_empty() && settings.latitude.is_none();
            lock(shared).weather = if off {
                WeatherState::Off
            } else {
                WeatherState::Loading
            };
            st.weather = (!off).then(|| WeatherJob {
                settings,
                next: Instant::now(),
            });
        }
        Command::SetVolume(level) => {
            barcraft_win32::set_volume(level);
            lock(shared).volume = barcraft_win32::volume();
        }
        Command::SetMuted(muted) => {
            barcraft_win32::set_muted(muted);
            lock(shared).volume = barcraft_win32::volume();
        }
        Command::SetMicrophoneMuted(muted) => {
            barcraft_win32::set_microphone_muted(muted);
            lock(shared).microphone = barcraft_win32::microphone();
        }
        Command::Media(action) => {
            barcraft_win32::media_action(action);
            lock(shared).media = barcraft_win32::media_status();
            st.last_media = Instant::now();
        }
        Command::ShowTray => {
            if watch.taskbar_mode == TaskbarMode::Hidden {
                barcraft_win32::set_taskbar_shown(true);
            }
            barcraft_win32::focus_system_tray();
            st.tray_until = Some(Instant::now() + TRAY_GRACE);
        }
        Command::EmptyRecycleBin => {
            // Windows shows its own confirmation, which waits for the user: keep it off this thread.
            let spawned = std::thread::Builder::new()
                .name("barcraft-empty-bin".into())
                .spawn(barcraft_win32::empty_recycle_bin);
            if let Err(e) = spawned {
                log::warn!("could not empty the Recycle Bin: {e}");
            }
        }
        Command::ClearUsage => {
            lock(shared).usage = DayUsage::default();
            st.usage_carry = 0.0;
            st.usage_dirty = true;
            st.last_usage_save = Instant::now()
                .checked_sub(USAGE_SAVE_EVERY)
                .unwrap_or_else(Instant::now);
        }
        Command::Watch(w) => *watch = w,
    }
    repaint.request_repaint();
}

fn poll_windows(shared: &Mutex<Snapshot>, watch: &DockWatch, own_pid: u32) -> bool {
    let windows = barcraft_win32::list_app_windows(own_pid);
    let foreground = barcraft_win32::foreground_window();
    let hide = match (watch.window, watch.monitor) {
        (Some(own), Some(monitor)) if watch.hide_for_fullscreen => {
            barcraft_win32::foreground_is_fullscreen(own, monitor)
        }
        _ => false,
    };
    let mut changed = false;
    let mut snap = lock(shared);
    if snap.windows != windows || snap.foreground != foreground {
        snap.windows = windows;
        snap.foreground = foreground;
        changed = true;
    }
    if snap.hidden_for_fullscreen != hide {
        snap.hidden_for_fullscreen = hide;
        if let Some(own) = watch.window {
            // Done from here because a hidden window gets no frames to undo it.
            barcraft_win32::set_window_shown(own, !hide);
        }
        if let Some(backdrop) = watch.backdrop {
            barcraft_win32::set_window_shown(backdrop, !hide);
        }
        changed = true;
    }
    changed
}

fn poll_stats(shared: &Mutex<Snapshot>, watch: &DockWatch, st: &mut State, now: Instant) {
    let times =
        barcraft_win32::cpu_times().map(|(idle, kernel, user)| CpuTimes { idle, kernel, user });
    let load = match (st.cpu_before, times) {
        (Some(a), Some(b)) => busy_fraction(a, b),
        _ => None,
    };
    st.cpu_before = times;
    let memory = barcraft_win32::memory_status();
    let power = barcraft_win32::power_status();
    let volume = barcraft_win32::volume();
    let windows_light = barcraft_win32::system_uses_light_theme();
    let microphone = watch
        .wants(WidgetKind::Microphone)
        .then(barcraft_win32::microphone)
        .flatten();
    let net = if watch.wants(WidgetKind::Network) {
        barcraft_win32::net_counters().map(|c| (c, now))
    } else {
        None
    };
    let rates = match (st.net_before, net) {
        (Some((a, ta)), Some((b, tb))) => {
            let secs = tb.duration_since(ta).as_secs_f64();
            (secs > 0.05).then(|| {
                (
                    b.received.saturating_sub(a.received) as f64 / secs,
                    b.sent.saturating_sub(a.sent) as f64 / secs,
                )
            })
        }
        _ => None,
    };
    st.net_before = net;

    let mut snap = lock(shared);
    if let Some(load) = load {
        snap.cpu.push(load);
    }
    snap.memory = memory;
    snap.power = power;
    snap.volume = volume;
    if let Some(light) = windows_light {
        snap.windows_light = light;
    }
    snap.microphone = microphone;
    if let Some((down, up)) = rates {
        if snap.net_down.len() == NET_HISTORY {
            snap.net_down.pop_front();
        }
        snap.net_down.push_back(down);
        snap.net_up = up;
    }
}

/// Adds the time since the last tick to the app in front, while someone is at the PC.
fn track_usage(shared: &Mutex<Snapshot>, watch: &DockWatch, st: &mut State, now: Instant) {
    let elapsed = now.duration_since(st.usage_last).as_secs_f64().min(5.0);
    st.usage_last = now;
    if !watch.wants(WidgetKind::TimeOnPc) {
        return;
    }
    if barcraft_win32::idle_seconds().is_none_or(|s| s >= IDLE_LIMIT_SECS) {
        return;
    }
    let mut snap = lock(shared);
    let Some(fg) = snap.foreground else {
        return;
    };
    let Some(app) = snap
        .windows
        .iter()
        .find(|w| w.id == fg)
        .and_then(|w| w.exe.as_deref())
        .map(display_name)
    else {
        return;
    };
    st.usage_carry += elapsed;
    let whole = st.usage_carry.floor();
    if whole >= 1.0 {
        st.usage_carry -= whole;
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        snap.usage.add(&date, &app, whole as u64);
        st.usage_dirty = true;
    }
}

fn save_usage(shared: &Mutex<Snapshot>, path: &std::path::Path, st: &mut State) {
    if !st.usage_dirty {
        return;
    }
    st.usage_dirty = false;
    st.last_usage_save = Instant::now();
    let usage = lock(shared).usage.clone();
    if let Err(e) = usage.save(path) {
        log::warn!("could not save Time on PC: {e}");
    }
}

/// Keeps the taskbar up while the user is in it after clicking the tray tile.
fn update_tray_peek(st: &mut State, now: Instant) {
    let Some(until) = st.tray_until else {
        return;
    };
    if barcraft_win32::foreground_is_shell_ui() {
        st.tray_until = Some(until.max(now + TRAY_LINGER));
    } else if now >= until {
        st.tray_until = None;
    }
}

fn http_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent(concat!("BarCraft/", env!("CARGO_PKG_VERSION")))
        .build();
    ureq::Agent::new_with_config(config)
}

fn get_text(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    let mut resp = agent.get(url).call().map_err(|e| e.to_string())?;
    resp.body_mut()
        .with_config()
        .limit(256 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Looks the place up when needed, then reads the current weather and the next days.
fn fetch_weather(agent: &ureq::Agent, s: &WeatherSettings) -> (WeatherState, Option<Place>) {
    let mut place = None;
    let (lat, lon) = match (s.latitude, s.longitude) {
        (Some(lat), Some(lon)) => (lat, lon),
        _ => {
            let found = get_text(agent, &weather::geocoding_url(&s.city))
                .ok()
                .and_then(|json| weather::parse_geocoding(&json));
            match found {
                Some(p) => {
                    let coords = (p.latitude, p.longitude);
                    place = Some(p);
                    coords
                }
                None => {
                    return (
                        WeatherState::Failed(format!(
                            "No place called \u{201c}{}\u{201d} was found",
                            s.city.trim()
                        )),
                        None,
                    );
                }
            }
        }
    };
    let unit: TempUnit = s.unit;
    match get_text(agent, &weather::forecast_url(lat, lon, unit)) {
        Ok(json) => match weather::parse_forecast(&json) {
            Some(current) => (
                WeatherState::Ready {
                    current,
                    at: Instant::now(),
                },
                place,
            ),
            None => (
                WeatherState::Failed("The weather service sent an answer we couldn't read".into()),
                place,
            ),
        },
        Err(e) => {
            log::info!("weather request failed: {e}");
            (
                WeatherState::Failed("The weather service can't be reached".into()),
                place,
            )
        }
    }
}
