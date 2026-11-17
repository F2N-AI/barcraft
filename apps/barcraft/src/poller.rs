//! A background thread that reads the system (windows, load, battery, volume, weather, icons)
//! so the dock's drawing never waits on Windows or the network.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use barcraft_core::cpu::{CpuTimes, History, busy_fraction};
use barcraft_core::settings::WeatherSettings;
use barcraft_core::weather::{self, Current, Place};
use barcraft_core::{TaskbarMode, TempUnit};
use barcraft_win32::{
    IconRgba, MemoryStatus, PowerStatus, PxRect, VolumeStatus, WindowId, WindowInfo,
};

const TICK: Duration = Duration::from_millis(250);
const WINDOWS_EVERY: Duration = Duration::from_millis(500);
const STATS_EVERY: Duration = Duration::from_secs(1);
const WEATHER_EVERY: Duration = Duration::from_secs(15 * 60);
const WEATHER_RETRY: Duration = Duration::from_secs(60);
const ICON_SIDE: u32 = 96;

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
    pub weather: WeatherState,
    /// A place the weather lookup resolved, for the app to store in settings.
    pub resolved_place: Option<Place>,
    /// Icons loaded since the dock last took them.
    pub icons: VecDeque<(String, Option<IconRgba>)>,
    pub hidden_for_fullscreen: bool,
}

/// What the poller needs to know about the dock window.
#[derive(Clone, Copy, Debug)]
pub struct DockWatch {
    pub window: Option<WindowId>,
    pub monitor: Option<PxRect>,
    pub hide_for_fullscreen: bool,
    pub taskbar_mode: TaskbarMode,
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
    Watch(DockWatch),
}

pub struct Poller {
    shared: Arc<Mutex<Snapshot>>,
    tx: Sender<Command>,
}

impl Poller {
    pub fn start(repaint: egui::Context) -> Poller {
        let shared = Arc::new(Mutex::new(Snapshot {
            windows: Vec::new(),
            foreground: None,
            cpu: History::new(60),
            memory: None,
            power: None,
            volume: None,
            weather: WeatherState::Off,
            resolved_place: None,
            icons: VecDeque::new(),
            hidden_for_fullscreen: false,
        }));
        let (tx, rx) = channel();
        let thread_shared = Arc::clone(&shared);
        let spawned = std::thread::Builder::new()
            .name("barcraft-poller".into())
            .spawn(move || run(thread_shared, rx, repaint));
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

fn run(shared: Arc<Mutex<Snapshot>>, rx: Receiver<Command>, repaint: egui::Context) {
    let own_pid = std::process::id();
    let start = Instant::now();
    let mut last_windows = start.checked_sub(WINDOWS_EVERY).unwrap_or(start);
    let mut last_stats = start.checked_sub(STATS_EVERY).unwrap_or(start);
    let mut cpu_before: Option<CpuTimes> = None;
    let mut watch = DockWatch {
        window: None,
        monitor: None,
        hide_for_fullscreen: true,
        taskbar_mode: TaskbarMode::Untouched,
    };
    let mut weather: Option<WeatherJob> = None;
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
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match rx.try_recv() {
                    Ok(c) => c,
                    Err(_) => break,
                }
            };
            match cmd {
                Command::LoadIcon { key, file, window } => {
                    let icon = file
                        .as_deref()
                        .and_then(|f| barcraft_win32::icon_for_file(f, ICON_SIDE))
                        .or_else(|| window.and_then(barcraft_win32::icon_for_window));
                    lock(&shared).icons.push_back((key, icon));
                    repaint.request_repaint();
                }
                Command::Weather(settings) => {
                    let off = settings.city.trim().is_empty() && settings.latitude.is_none();
                    lock(&shared).weather = if off {
                        WeatherState::Off
                    } else {
                        WeatherState::Loading
                    };
                    weather = (!off).then(|| WeatherJob {
                        settings,
                        next: Instant::now(),
                    });
                    repaint.request_repaint();
                }
                Command::SetVolume(level) => {
                    barcraft_win32::set_volume(level);
                    lock(&shared).volume = barcraft_win32::volume();
                    repaint.request_repaint();
                }
                Command::SetMuted(muted) => {
                    barcraft_win32::set_muted(muted);
                    lock(&shared).volume = barcraft_win32::volume();
                    repaint.request_repaint();
                }
                Command::Watch(w) => watch = w,
            }
        }

        let now = Instant::now();
        let mut changed = false;

        if now.duration_since(last_windows) >= WINDOWS_EVERY {
            last_windows = now;
            let windows = barcraft_win32::list_app_windows(own_pid);
            let foreground = barcraft_win32::foreground_window();
            let hide = match (watch.window, watch.monitor) {
                (Some(own), Some(monitor)) if watch.hide_for_fullscreen => {
                    barcraft_win32::foreground_is_fullscreen(own, monitor)
                }
                _ => false,
            };
            let mut snap = lock(&shared);
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
                changed = true;
            }
        }

        if now.duration_since(last_stats) >= STATS_EVERY {
            last_stats = now;
            let times = barcraft_win32::cpu_times().map(|(idle, kernel, user)| CpuTimes {
                idle,
                kernel,
                user,
            });
            let load = match (cpu_before, times) {
                (Some(a), Some(b)) => busy_fraction(a, b),
                _ => None,
            };
            cpu_before = times;
            let memory = barcraft_win32::memory_status();
            let power = barcraft_win32::power_status();
            let volume = barcraft_win32::volume();
            crate::taskbar::keep_applied(watch.taskbar_mode);
            let mut snap = lock(&shared);
            if let Some(load) = load {
                snap.cpu.push(load);
            }
            snap.memory = memory;
            snap.power = power;
            snap.volume = volume;
            changed = true;
        }

        if let Some(job) = weather.as_mut()
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

/// Looks the place up when needed, then reads the current weather.
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
