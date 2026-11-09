//! What the system widgets watch: how busy the processor is, memory and
//! swap in use, what each network interface moves, the temperature
//! sensors and how full each disk is, with the last quarter hour of each
//! for the graphs.
//!
//! One service on one tick (every 2 s), so a desktop of widgets redraws
//! once a tick rather than once per reading. It starts the first time a
//! widget reads it, and reads nothing while no system widget shows
//! ([`SystemStats::watch`]); the temperature sensors, some of which sit on
//! slow buses, only while a temperature widget shows. Everything comes from the kernel
//! (`/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `/proc/net/route`,
//! `/sys/class/hwmon`, `/proc/mounts` and `statvfs`); the parsers are plain
//! functions over the files' text. The files are read with no lock held, so
//! a slow sensor or a stalled network mount never holds up drawing.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use amane::Service;

use crate::services::Wake;

/// How often it reads, and how many readings each graph keeps (15 min).
const TICK: Duration = Duration::from_secs(2);
pub const HISTORY: usize = 450;
/// Disks fill slowly: they're read every this many ticks.
const DISK_EVERY: u32 = 15;
/// Sensors come and go rarely (a drive plugged in): the hwmon directories
/// are looked through again every this many ticks, or when a read fails.
const SCAN_EVERY: u32 = 30;
/// A mount whose `statvfs` took this long (a network share gone away) is
/// left out of this many disk readings after.
const STALLED: Duration = Duration::from_secs(1);
const STALLED_SKIP: u32 = 10;

/// Whether a system widget shows anywhere; while none does, nothing is read.
static WATCHED: AtomicBool = AtomicBool::new(false);
/// Whether a temperature shows: some sensors sit on slow buses (a drive's
/// is a command to it), so they're read only then.
static TEMPERATURES: AtomicBool = AtomicBool::new(false);

// a widget showing again wakes the paused listener
static WAKE: Wake = Wake::new();

/// The readings of one thing over time, oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Series(VecDeque<f32>);

impl Series {
    fn push(&mut self, value: f32) {
        if self.0.len() == HISTORY {
            self.0.pop_front();
        }
        self.0.push_back(value);
    }

    pub fn last(&self) -> Option<f32> {
        self.0.back().copied()
    }

    /// The last `n` readings, oldest first.
    pub fn recent(&self, n: usize) -> impl Iterator<Item = f32> + '_ {
        self.0.iter().skip(self.0.len().saturating_sub(n)).copied()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Ticks {
    busy: u64,
    total: u64,
}

impl Ticks {
    /// How busy, 0 … 1, between `before` and this reading.
    fn since(self, before: Ticks) -> Option<f32> {
        let total = self.total.checked_sub(before.total)?;
        let busy = self.busy.checked_sub(before.busy)?;
        (total > 0).then(|| (busy as f32 / total as f32).clamp(0.0, 1.0))
    }
}

/// Memory in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryUse {
    pub used: u64,
    pub total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
}

impl MemoryUse {
    pub fn fraction(&self) -> f32 {
        fraction(self.used, self.total)
    }

    pub fn swap_fraction(&self) -> f32 {
        fraction(self.swap_used, self.swap_total)
    }
}

/// One interface's traffic: bytes a second in and out, and over time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Traffic {
    pub down: Series,
    pub up: Series,
    counters: (u64, u64),
}

/// A temperature sensor: its name and what it reads, in °C.
#[derive(Clone, Debug, PartialEq)]
pub struct Sensor {
    pub label: String,
    pub celsius: f32,
    /// Whether it's the processor's.
    pub cpu: bool,
    /// Where it runs hot, by its own say (hwmon's `max`, else 10 under its
    /// `crit`), when it says.
    pub high: Option<f32>,
}

/// A mounted filesystem's size and what's used, in bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskUse {
    pub mount: String,
    pub used: u64,
    pub total: u64,
}

impl DiskUse {
    pub fn fraction(&self) -> f32 {
        fraction(self.used, self.total)
    }
}

fn fraction(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f32 / whole as f32).clamp(0.0, 1.0)
    }
}

pub struct SystemStats {
    ticks: Option<(Ticks, Vec<Ticks>)>,
    /// The whole processor over time, 0 … 1.
    pub cpu: Series,
    /// Each core now, 0 … 1.
    pub cores: Vec<f32>,
    pub memory: MemoryUse,
    pub memory_history: Series,
    traffic: HashMap<String, Traffic>,
    read_at: Option<Instant>,
    default_route: Option<String>,
    pub sensors: Vec<Sensor>,
    temperature: HashMap<String, Series>,
    pub disks: Vec<DiskUse>,
}

impl Service for SystemStats {
    fn new() -> Self {
        let mut stats = Self {
            ticks: None,
            cpu: Series::default(),
            cores: Vec::new(),
            memory: MemoryUse::default(),
            memory_history: Series::default(),
            traffic: HashMap::new(),
            read_at: None,
            default_route: None,
            sensors: Vec::new(),
            temperature: HashMap::new(),
            disks: Vec::new(),
        };
        // something to show on the first frame, before the first tick
        stats.apply(Probe::default().read());
        stats
    }

    fn interval() -> Duration {
        TICK
    }

    fn listen() {
        let mut probe = Probe::default();
        loop {
            thread::sleep(TICK);
            if !WATCHED.load(Ordering::Relaxed) {
                // a reading after the pause would average over all of it
                probe.paused = true;
                // no wake-ups until a widget shows again
                WAKE.wait(None);
                continue;
            }
            let reading = probe.read();
            let paused = std::mem::take(&mut probe.paused);
            let mut stats = Self::write();
            if paused {
                stats.ticks = None;
                stats.read_at = None;
            }
            stats.apply(reading);
        }
    }
}

impl SystemStats {
    /// Says whether a system widget shows, so the readings go on or pause,
    /// and whether one shows a temperature, so the sensors are read too.
    pub fn watch(watched: bool, temperatures: bool) {
        TEMPERATURES.store(temperatures, Ordering::Relaxed);
        if !WATCHED.swap(watched, Ordering::Relaxed) && watched {
            WAKE.nudge();
        }
    }

    fn apply(&mut self, reading: Reading) {
        if let Some((total, cores)) = reading.stat {
            if let Some((before, before_cores)) = &self.ticks {
                // counters that went backwards (a resume) say nothing, rather
                // than a dip to idle in the graph
                if let Some(busy) = total.since(*before) {
                    self.cpu.push(busy);
                }
                self.cores.clear();
                self.cores.extend(
                    cores
                        .iter()
                        .zip(before_cores)
                        .map(|(now, then)| now.since(*then).unwrap_or(0.0)),
                );
            }
            self.ticks = Some((total, cores));
        }

        if let Some(memory) = reading.memory {
            self.memory = memory;
            self.memory_history.push(memory.fraction());
        }

        let seconds = self
            .read_at
            .map(|then| reading.at.duration_since(then).as_secs_f32())
            .filter(|s| *s > 0.0);
        // an interface that went away (a VPN's tun) takes its history along
        self.traffic
            .retain(|name, _| reading.traffic.iter().any(|(n, _)| n == name));
        for (name, counters) in reading.traffic {
            // one that just came up (a VPN, a tether) has nothing to measure
            // its counters against yet: they're everything it ever carried
            let known = self.traffic.contains_key(&name);
            let traffic = self.traffic.entry(name).or_default();
            if let Some(seconds) = seconds.filter(|_| known) {
                let rate = |now: u64, then: u64| now.saturating_sub(then) as f32 / seconds;
                traffic.down.push(rate(counters.0, traffic.counters.0));
                traffic.up.push(rate(counters.1, traffic.counters.1));
            }
            traffic.counters = counters;
        }
        self.read_at = Some(reading.at);
        self.default_route = reading.default_route;

        if let Some(sensors) = reading.sensors {
            self.apply_sensors(sensors);
        }
        if let Some(disks) = reading.disks {
            self.disks = disks;
        }
    }

    fn apply_sensors(&mut self, sensors: Vec<Sensor>) {
        self.sensors = sensors;
        let sensors = &self.sensors;
        self.temperature
            .retain(|label, _| sensors.iter().any(|s| &s.label == label));
        for sensor in &self.sensors {
            match self.temperature.get_mut(&sensor.label) {
                Some(series) => series.push(sensor.celsius),
                None => {
                    let mut series = Series::default();
                    series.push(sensor.celsius);
                    self.temperature.insert(sensor.label.clone(), series);
                }
            }
        }
    }
}

/// One tick's readings, taken with no lock held.
struct Reading {
    at: Instant,
    stat: Option<(Ticks, Vec<Ticks>)>,
    memory: Option<MemoryUse>,
    traffic: Vec<(String, (u64, u64))>,
    default_route: Option<String>,
    /// Only while a temperature shows.
    sensors: Option<Vec<Sensor>>,
    /// Only on the ticks the disks are read.
    disks: Option<Vec<DiskUse>>,
}

/// What the reading thread keeps between ticks: where the sensors are, a
/// buffer to read files into, and the mounts that stalled.
#[derive(Default)]
struct Probe {
    tick: u32,
    inputs: Vec<Input>,
    scanned: bool,
    text: String,
    stalled: HashMap<String, u32>,
    /// Set while nothing watched, so the next reading starts afresh.
    paused: bool,
}

impl Probe {
    fn read(&mut self) -> Reading {
        let at = Instant::now();

        read_into("/proc/stat", &mut self.text);
        let stat = parse_stat(&self.text);
        read_into("/proc/meminfo", &mut self.text);
        let memory = parse_meminfo(&self.text);
        read_into("/proc/net/dev", &mut self.text);
        let traffic = parse_net_dev(&self.text)
            .into_iter()
            .filter(|(name, _)| name != "lo")
            .collect();
        read_into("/proc/net/route", &mut self.text);
        // an IPv6-only network has no IPv4 default route
        let default_route = parse_route(&self.text).or_else(|| {
            read_into("/proc/net/ipv6_route", &mut self.text);
            parse_route6(&self.text)
        });

        let sensors = TEMPERATURES.load(Ordering::Relaxed).then(|| self.sensors());

        let disks = self.tick.is_multiple_of(DISK_EVERY).then(|| {
            read_into("/proc/mounts", &mut self.text);
            let mounts = mounts(&self.text);
            disks(mounts, &mut self.stalled)
        });

        self.tick = self.tick.wrapping_add(1);
        Reading {
            at,
            stat,
            memory,
            traffic,
            default_route,
            sensors,
            disks,
        }
    }
}

impl Probe {
    fn sensors(&mut self) -> Vec<Sensor> {
        if !self.scanned || self.tick.is_multiple_of(SCAN_EVERY) {
            self.inputs = scan(Path::new("/sys/class/hwmon"));
            self.scanned = true;
        }
        let (sensors, whole) = read_sensors(&self.inputs, &mut self.text);
        if whole {
            return sensors;
        }
        // one went away, or a drive asleep didn't answer: look again, and
        // show what still reads either way
        self.inputs = scan(Path::new("/sys/class/hwmon"));
        read_sensors(&self.inputs, &mut self.text).0
    }
}

/// `path`'s text into `text`, which is kept from one read to the next;
/// empty if it can't be read.
fn read_into(path: impl AsRef<Path>, text: &mut String) -> bool {
    text.clear();
    File::open(path)
        .and_then(|mut file| file.read_to_string(text))
        .is_ok()
}

impl SystemStats {
    /// The interface `name`, or the one the default route goes out of.
    pub fn interface(&self, name: Option<&str>) -> Option<(&str, &Traffic)> {
        let name = name.or(self.default_route.as_deref())?;
        self.traffic
            .get_key_value(name)
            .map(|(name, traffic)| (name.as_str(), traffic))
    }

    /// Every interface but loopback, by name.
    pub fn interfaces(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.traffic.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// The sensor labelled `label`, or the processor's, or the hottest.
    pub fn sensor(&self, label: Option<&str>) -> Option<&Sensor> {
        if let Some(label) = label {
            return self.sensors.iter().find(|s| s.label == label);
        }
        self.sensors.iter().find(|s| s.cpu).or_else(|| {
            self.sensors
                .iter()
                .max_by(|a, b| a.celsius.total_cmp(&b.celsius))
        })
    }

    pub fn temperature_history(&self, label: &str) -> Option<&Series> {
        self.temperature.get(label)
    }

    /// The disk mounted at `mount`; or the root, or, where the root isn't
    /// a disk (a tmpfs root), the first that is.
    pub fn disk(&self, mount: Option<&str>) -> Option<&DiskUse> {
        match mount {
            Some(mount) => self.disks.iter().find(|d| d.mount == mount),
            None => self
                .disks
                .iter()
                .find(|d| d.mount == "/")
                .or_else(|| self.disks.first()),
        }
    }
}

/// The whole processor's ticks and each core's, from `/proc/stat`.
fn parse_stat(text: &str) -> Option<(Ticks, Vec<Ticks>)> {
    let ticks = |line: &str| {
        let fields: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|f| f.parse().ok())
            .collect();
        // user nice system idle iowait irq softirq steal (guests are in user)
        let total: u64 = fields.iter().take(8).sum();
        let idle = fields.get(3).copied().unwrap_or(0) + fields.get(4).copied().unwrap_or(0);
        Ticks {
            busy: total.saturating_sub(idle),
            total,
        }
    };
    let mut lines = text.lines();
    let first = lines.next().filter(|line| line.starts_with("cpu "))?;
    let cores = lines
        .take_while(|line| line.starts_with("cpu"))
        .map(ticks)
        .collect();
    Some((ticks(first), cores))
}

/// Memory and swap from `/proc/meminfo` (in kB there): what's used is
/// what isn't available, as `free` counts it.
fn parse_meminfo(text: &str) -> Option<MemoryUse> {
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
            .and_then(|rest| rest.split_whitespace().next()?.parse::<u64>().ok())
            .map(|kib| kib * 1024)
    };
    let total = field("MemTotal")?;
    let available = field("MemAvailable").or_else(|| field("MemFree"))?;
    let swap_total = field("SwapTotal").unwrap_or(0);
    let swap_free = field("SwapFree").unwrap_or(0);
    Some(MemoryUse {
        used: total.saturating_sub(available),
        total,
        swap_used: swap_total.saturating_sub(swap_free),
        swap_total,
    })
}

/// Each interface's bytes received and sent, from `/proc/net/dev`.
fn parse_net_dev(text: &str) -> Vec<(String, (u64, u64))> {
    text.lines()
        .skip(2)
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            let fields: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|f| f.parse().ok())
                .collect();
            Some((name.trim().to_string(), (*fields.first()?, *fields.get(8)?)))
        })
        .collect()
}

/// The interface the default route goes out of, from `/proc/net/route`:
/// of several (wired and wireless both up), the one with the lowest metric,
/// as the kernel picks.
fn parse_route(text: &str) -> Option<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let metric = fields
                .get(6)
                .and_then(|m| m.parse::<u32>().ok())
                .unwrap_or(0);
            (*fields.get(1)? == "00000000").then(|| (metric, fields[0]))
        })
        .min_by_key(|(metric, _)| *metric)
        .map(|(_, name)| name.to_string())
}

/// The interface the IPv6 default route (`::/0`) goes out of, from
/// `/proc/net/ipv6_route`, the one with the lowest metric; loopback's
/// unreachable default doesn't count.
fn parse_route6(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let [dest, len, .., metric, _, _, _, iface] = fields.as_slice() else {
                return None;
            };
            let default = dest.bytes().all(|b| b == b'0') && *len == "00" && *iface != "lo";
            let metric = u32::from_str_radix(metric, 16).ok()?;
            default.then(|| (metric, (*iface).to_string()))
        })
        .min_by_key(|(metric, _)| *metric)
        .map(|(_, name)| name)
}

/// Drivers whose first sensor is the processor's.
const CPU_SENSORS: [&str; 4] = ["k10temp", "coretemp", "zenpower", "cpu_thermal"];

/// A temperature sensor's file, what to call it, and where it runs hot.
struct Input {
    path: PathBuf,
    label: String,
    cpu: bool,
    high: Option<f32>,
}

/// Every temperature sensor under `root` (`/sys/class/hwmon`), named by
/// its driver and label ("k10temp Tctl").
fn scan(root: &Path) -> Vec<Input> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
        let Some(driver) = read("name").map(|s| s.trim().to_string()) else {
            continue;
        };
        let cpu = CPU_SENSORS.contains(&driver.as_str());
        let mut first = true;
        for n in 1..=16 {
            let path = dir.join(format!("temp{n}_input"));
            if !path.exists() {
                continue;
            }
            let label = read(&format!("temp{n}_label"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| format!("temp{n}"));
            let degrees = |name: &str| {
                read(&format!("temp{n}_{name}"))
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .map(|millis| millis as f32 / 1000.0)
                    // a sensor that reports nothing useful says 0, or far too much
                    .filter(|c| *c > 20.0 && *c < 150.0)
            };
            let high = degrees("max").or_else(|| degrees("crit").map(|c| c - 10.0));
            out.push(Input {
                path,
                label: unique(&out, format!("{driver} {label}")),
                cpu: cpu && first,
                high,
            });
            first = false;
        }
    }
    out
}

/// `label`, or with a number after it when one of `taken` has it already
/// (two drives of a kind are both "nvme Composite"), so each sensor keeps a
/// history and a setting of its own.
fn unique(taken: &[Input], label: String) -> String {
    let has = |name: &str| taken.iter().any(|input| input.label == name);
    if !has(&label) {
        return label;
    }
    (2..)
        .map(|n| format!("{label} {n}"))
        .find(|name| !has(name))
        .expect("a free number")
}

/// What each sensor that reads reads now, and whether they all did (one
/// that didn't went away, as a drive taken out, or is asleep).
fn read_sensors(inputs: &[Input], text: &mut String) -> (Vec<Sensor>, bool) {
    let mut whole = true;
    let sensors = inputs
        .iter()
        .filter_map(|input| {
            let millis = read_into(&input.path, text)
                .then(|| text.trim().parse::<i64>().ok())
                .flatten();
            whole &= millis.is_some();
            Some(Sensor {
                label: input.label.clone(),
                celsius: millis? as f32 / 1000.0,
                cpu: input.cpu,
                high: input.high,
            })
        })
        .collect();
    (sensors, whole)
}

/// The filesystems a person keeps files on, from `/proc/mounts`: those on
/// a block device (or a few network and pooled kinds), each once.
fn mounts(text: &str) -> Vec<String> {
    const KINDS: [&str; 4] = ["zfs", "nfs", "nfs4", "fuse.sshfs"];
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let (Some(device), Some(mount), Some(kind)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let real = device.starts_with("/dev/") || KINDS.contains(&kind);
        // a store mounted read-only again (NixOS's /nix/store) is the same disk
        let boot = mount == "/boot" || mount.starts_with("/boot/");
        if !real || mount.starts_with("/nix/store") || boot {
            continue;
        }
        if seen.contains(&device) {
            continue;
        }
        seen.push(device);
        out.push(unescape(mount));
    }
    out
}

/// A mount point as `/proc/mounts` writes it, its space, tab, newline and
/// backslash each an octal escape (`\040`), back as it is.
fn unescape(mount: &str) -> String {
    let bytes = mount.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|digits| bytes[i] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d)))
            .and_then(|digits| u8::from_str_radix(std::str::from_utf8(digits).ok()?, 8).ok());
        match octal {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// How full each of `mounts` is. A mount that stalled is left out for a
/// while (`stalled` counts down its skips), so a share gone away holds up
/// one reading, not every one.
fn disks(mounts: Vec<String>, stalled: &mut HashMap<String, u32>) -> Vec<DiskUse> {
    let present: HashSet<&str> = mounts.iter().map(String::as_str).collect();
    stalled.retain(|mount, _| present.contains(mount.as_str()));
    mounts
        .into_iter()
        .filter_map(|mount| {
            if let Some(skips) = stalled.get_mut(&mount) {
                *skips -= 1;
                if *skips == 0 {
                    stalled.remove(&mount);
                }
                return None;
            }
            let started = Instant::now();
            let space = emaki_kit::fs::space(Path::new(&mount));
            if started.elapsed() >= STALLED {
                stalled.insert(mount.clone(), STALLED_SKIP);
            }
            let space = space?;
            let (total, free) = (space.total, space.available);
            (total > 0).then(|| DiskUse {
                mount,
                used: total.saturating_sub(free),
                total,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processor_ticks_make_a_busy_fraction() {
        let before = "cpu  100 0 100 700 100 0 0 0 0 0\ncpu0 50 0 50 350 50 0 0 0 0 0\n\
                      cpu1 50 0 50 350 50 0 0 0 0 0\nintr 1 2 3\n";
        let after = "cpu  200 0 200 800 100 0 0 0 0 0\ncpu0 150 0 50 350 50 0 0 0 0 0\n\
                     cpu1 50 0 150 450 50 0 0 0 0 0\nintr 1 2 3\n";
        let (b, b_cores) = parse_stat(before).unwrap();
        let (a, a_cores) = parse_stat(after).unwrap();
        assert_eq!(a.since(b), Some(2.0 / 3.0));
        assert_eq!(a_cores.len(), 2);
        assert_eq!(a_cores[0].since(b_cores[0]), Some(1.0));
        assert_eq!(a_cores[1].since(b_cores[1]), Some(0.5));
        // counters that went backwards (a resume) give nothing
        assert_eq!(b.since(a), None);
        assert_eq!(parse_stat("intr 1"), None);
    }

    #[test]
    fn memory_in_use_is_what_isnt_available() {
        let text = "MemTotal:       16000000 kB\nMemFree:         1000000 kB\n\
                    MemAvailable:    6000000 kB\nSwapTotal:       4000000 kB\n\
                    SwapFree:        3000000 kB\n";
        let memory = parse_meminfo(text).unwrap();
        assert_eq!(memory.total, 16_000_000 * 1024);
        assert_eq!(memory.used, 10_000_000 * 1024);
        assert_eq!(memory.swap_used, 1_000_000 * 1024);
        assert!((memory.fraction() - 0.625).abs() < 1e-6);
        assert_eq!(parse_meminfo(""), None);
    }

    #[test]
    fn interfaces_and_the_default_route() {
        let dev = "Inter-|   Receive |  Transmit\n face |bytes packets|bytes\n\
                   \x20   lo: 5975727 21975 0 0 0 0 0 0 5975727 21975 0 0 0 0 0 0\n\
                   wlp194s0: 50706225 114232 0 0 0 0 0 0 44324094 106685 0 0 0 0 0 0\n";
        assert_eq!(
            parse_net_dev(dev),
            vec![
                (String::from("lo"), (5_975_727, 5_975_727)),
                (String::from("wlp194s0"), (50_706_225, 44_324_094)),
            ]
        );
        let route = "Iface\tDestination\tGateway\tFlags\n\
                     wlp194s0\t0001A8C0\t00000000\t0001\n\
                     wlp194s0\t00000000\t0101A8C0\t0003\n";
        assert_eq!(parse_route(route).as_deref(), Some("wlp194s0"));
        // wired and wireless both up: the lower metric is the default
        let both = "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\n\
                    wlp194s0\t00000000\t0101A8C0\t0003\t0\t0\t600\n\
                    enp5s0\t00000000\t0101A8C0\t0003\t0\t0\t100\n";
        assert_eq!(parse_route(both).as_deref(), Some("enp5s0"));
        // IPv6 only: the default (all zeros, length 0) that isn't loopback's
        let six = "00000000000000000000000000000000 00 00000000000000000000000000000000 00 \
                   fe800000000000000000000000000001 00000400 00000001 00000000 00000003 wlan0\n\
                   00000000000000000000000000000000 00 00000000000000000000000000000000 00 \
                   00000000000000000000000000000000 ffffffff 00000001 00000000 00200200 lo\n\
                   fd000000000000000000000000000000 40 00000000000000000000000000000000 00 \
                   00000000000000000000000000000000 00000100 00000001 00000000 00000001 wlan0\n";
        assert_eq!(parse_route6(six).as_deref(), Some("wlan0"));
        assert_eq!(parse_route("Iface\tDestination\n"), None);
    }

    #[test]
    fn only_real_filesystems_count_once() {
        let text = "/dev/nvme0n1p2 / ext4 rw 0 0\nproc /proc proc rw 0 0\n\
                    /dev/nvme0n1p2 /nix/store ext4 ro 0 0\n/dev/nvme0n1p1 /boot vfat rw 0 0\n\
                    /dev/sda1 /mnt/My\\040Disk ext4 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n\
                    /dev/sda1 /mnt/again ext4 rw 0 0\n";
        assert_eq!(mounts(text), vec!["/", "/mnt/My Disk"]);
        let text = "/dev/sdb1 /bootleg ext4 rw 0 0\n/dev/sdc1 /mnt/a\\011b\\134c ext4 rw 0 0\n";
        assert_eq!(mounts(text), vec!["/bootleg", "/mnt/a\tb\\c"]);
    }

    #[test]
    fn a_series_keeps_the_last_quarter_hour() {
        let mut series = Series::default();
        for n in 0..(HISTORY + 10) {
            series.push(n as f32);
        }
        assert_eq!(series.0.len(), HISTORY);
        assert_eq!(series.last(), Some((HISTORY + 9) as f32));
        assert_eq!(
            series.recent(2).collect::<Vec<_>>(),
            [(HISTORY + 8) as f32, (HISTORY + 9) as f32]
        );
    }

    /// A hwmon tree of its own under the test's folder: (driver, inputs as
    /// (label, millidegrees, or none for one that can't be read)).
    /// A chip's inputs: (label, millidegrees, or none for one that can't be read).
    type Inputs<'a> = &'a [(&'a str, Option<&'a str>)];

    fn hwmon(test: &str, chips: &[(&str, Inputs)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("emaki-hwmon-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (i, (driver, inputs)) in chips.iter().enumerate() {
            let dir = root.join(format!("hwmon{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("name"), driver).unwrap();
            for (n, (label, millis)) in inputs.iter().enumerate() {
                let n = n + 1;
                std::fs::write(dir.join(format!("temp{n}_label")), label).unwrap();
                match millis {
                    Some(millis) => std::fs::write(dir.join(format!("temp{n}_input")), millis),
                    // a directory where the file is: there, but it can't be read
                    None => std::fs::create_dir(dir.join(format!("temp{n}_input"))),
                }
                .unwrap();
            }
        }
        root
    }

    #[test]
    fn sensors_read_each_on_its_own_and_keep_names_of_their_own() {
        let root = hwmon(
            "sensors",
            &[
                ("k10temp", &[("Tctl", Some("52000"))]),
                ("nvme", &[("Composite", Some("40500"))]),
                ("nvme", &[("Composite", None)]),
                ("nvme", &[("Composite", Some("38000"))]),
            ],
        );
        // where each runs hot: its max, else 10 under its crit, else nothing
        std::fs::write(root.join("hwmon0/temp1_max"), "95000").unwrap();
        std::fs::write(root.join("hwmon1/temp1_crit"), "84850").unwrap();
        std::fs::write(root.join("hwmon3/temp1_max"), "0").unwrap();
        let inputs = scan(&root);
        let highs: Vec<Option<f32>> = inputs.iter().map(|i| i.high).collect();
        assert_eq!(highs, [Some(95.0), Some(74.85), None, None]);
        let labels: Vec<&str> = inputs.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "k10temp Tctl",
                "nvme Composite",
                "nvme Composite 2",
                "nvme Composite 3"
            ]
        );
        assert!(inputs[0].cpu && !inputs[1].cpu);
        // the one that can't be read is left out, not every one
        let (sensors, whole) = read_sensors(&inputs, &mut String::new());
        assert!(!whole);
        let read: Vec<(&str, f32)> = sensors
            .iter()
            .map(|s| (s.label.as_str(), s.celsius))
            .collect();
        assert_eq!(
            read,
            [
                ("k10temp Tctl", 52.0),
                ("nvme Composite", 40.5),
                ("nvme Composite 3", 38.0)
            ]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn reading(at: Instant, traffic: &[(&str, (u64, u64))], busy: u64) -> Reading {
        let ticks = Ticks {
            total: busy * 2,
            busy,
        };
        Reading {
            at,
            stat: Some((ticks, Vec::new())),
            memory: None,
            traffic: traffic
                .iter()
                .map(|(n, c)| ((*n).to_string(), *c))
                .collect(),
            default_route: None,
            sensors: None,
            disks: None,
        }
    }

    #[test]
    fn an_interface_that_comes_up_starts_without_a_spike() {
        // nothing read from this machine, so its interfaces don't count
        let mut stats = SystemStats {
            ticks: None,
            cpu: Series::default(),
            cores: Vec::new(),
            memory: MemoryUse::default(),
            memory_history: Series::default(),
            traffic: HashMap::new(),
            read_at: None,
            default_route: None,
            sensors: Vec::new(),
            temperature: HashMap::new(),
            disks: Vec::new(),
        };
        let start = Instant::now();
        let later = |s: u64| start + std::time::Duration::from_secs(s);
        stats.apply(reading(start, &[("wlan0", (1_000, 1_000))], 100));
        stats.apply(reading(
            later(2),
            &[("wlan0", (3_000, 1_000)), ("tun0", (9_000_000, 9_000_000))],
            200,
        ));
        // the tether's lifetime counters aren't a rate
        assert_eq!(stats.traffic["tun0"].down.last(), None);
        assert_eq!(stats.traffic["wlan0"].down.last(), Some(1_000.0));
        stats.apply(reading(
            later(4),
            &[("wlan0", (3_000, 1_000)), ("tun0", (9_002_000, 9_000_000))],
            300,
        ));
        assert_eq!(stats.traffic["tun0"].down.last(), Some(1_000.0));
        // processor counters that went backwards add nothing to the graph
        let before = stats.cpu.0.len();
        stats.apply(reading(later(6), &[], 10));
        assert_eq!(stats.cpu.0.len(), before);
    }
}
