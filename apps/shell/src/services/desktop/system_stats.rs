//! What the system widgets watch: how busy the processor is, memory and
//! swap in use, what each network interface moves, the temperature
//! sensors and how full each disk is, with the last quarter hour of each
//! for the graphs.
//!
//! One service on one tick (every 2 s), so a desktop of widgets redraws
//! once a tick rather than once per reading. It starts the first time a
//! widget reads it, and reads nothing while no system widget shows
//! ([`SystemStats::watch`]). Everything comes from the kernel
//! (`/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `/proc/net/route`,
//! `/sys/class/hwmon`, `/proc/mounts` and `statvfs`); the parsers are plain
//! functions over the files' text. The files are read with no lock held, so
//! a slow sensor or a stalled network mount never holds up drawing.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use amane::Service;

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
    /// Says whether a system widget shows, so the readings go on or pause.
    pub fn watch(watched: bool) {
        WATCHED.store(watched, Ordering::Relaxed);
    }

    fn apply(&mut self, reading: Reading) {
        if let Some((total, cores)) = reading.stat {
            if let Some((before, before_cores)) = &self.ticks {
                self.cpu.push(total.since(*before).unwrap_or(0.0));
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
            let traffic = self.traffic.entry(name).or_default();
            if let Some(seconds) = seconds {
                let rate = |now: u64, then: u64| now.saturating_sub(then) as f32 / seconds;
                traffic.down.push(rate(counters.0, traffic.counters.0));
                traffic.up.push(rate(counters.1, traffic.counters.1));
            }
            traffic.counters = counters;
        }
        self.read_at = Some(reading.at);
        self.default_route = reading.default_route;

        self.sensors = reading.sensors;
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

        if let Some(disks) = reading.disks {
            self.disks = disks;
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
    sensors: Vec<Sensor>,
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
        let default_route = parse_route(&self.text);

        if !self.scanned || self.tick.is_multiple_of(SCAN_EVERY) {
            self.inputs = scan(Path::new("/sys/class/hwmon"));
            self.scanned = true;
        }
        let sensors = match read_sensors(&self.inputs, &mut self.text) {
            Some(sensors) => sensors,
            None => {
                // one went away: look again
                self.inputs = scan(Path::new("/sys/class/hwmon"));
                read_sensors(&self.inputs, &mut self.text).unwrap_or_default()
            }
        };

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

/// The interface the default route goes out of, from `/proc/net/route`.
fn parse_route(text: &str) -> Option<String> {
    text.lines().skip(1).find_map(|line| {
        let mut fields = line.split_whitespace();
        let name = fields.next()?;
        (fields.next()? == "00000000").then(|| name.to_string())
    })
}

/// Drivers whose first sensor is the processor's.
const CPU_SENSORS: [&str; 4] = ["k10temp", "coretemp", "zenpower", "cpu_thermal"];

/// A temperature sensor's file, and what to call it.
struct Input {
    path: PathBuf,
    label: String,
    cpu: bool,
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
            out.push(Input {
                path,
                label: format!("{driver} {label}"),
                cpu: cpu && first,
            });
            first = false;
        }
    }
    out
}

/// What each sensor reads now; none if one couldn't be read, as when a
/// drive was taken out.
fn read_sensors(inputs: &[Input], text: &mut String) -> Option<Vec<Sensor>> {
    inputs
        .iter()
        .map(|input| {
            if !read_into(&input.path, text) {
                return None;
            }
            let millis = text.trim().parse::<i64>().ok()?;
            Some(Sensor {
                label: input.label.clone(),
                celsius: millis as f32 / 1000.0,
                cpu: input.cpu,
            })
        })
        .collect()
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
        if !real || mount.starts_with("/nix/store") || mount.starts_with("/boot") {
            continue;
        }
        if seen.contains(&device) {
            continue;
        }
        seen.push(device);
        // octal escapes for spaces and the like
        out.push(mount.replace("\\040", " "));
    }
    out
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
            let path = CString::new(mount.as_bytes()).ok()?;
            let started = Instant::now();
            // SAFETY: a zeroed statvfs is a valid one for the call to fill,
            // and `path` is a NUL-terminated string that outlives it.
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            let failed = unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0;
            if started.elapsed() >= STALLED {
                stalled.insert(mount.clone(), STALLED_SKIP);
            }
            if failed {
                return None;
            }
            let block = stat.f_frsize as u64;
            let total = stat.f_blocks as u64 * block;
            let free = stat.f_bavail as u64 * block;
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
        assert_eq!(parse_route("Iface\tDestination\n"), None);
    }

    #[test]
    fn only_real_filesystems_count_once() {
        let text = "/dev/nvme0n1p2 / ext4 rw 0 0\nproc /proc proc rw 0 0\n\
                    /dev/nvme0n1p2 /nix/store ext4 ro 0 0\n/dev/nvme0n1p1 /boot vfat rw 0 0\n\
                    /dev/sda1 /mnt/My\\040Disk ext4 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n\
                    /dev/sda1 /mnt/again ext4 rw 0 0\n";
        assert_eq!(mounts(text), vec!["/", "/mnt/My Disk"]);
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

    #[test]
    fn the_sensors_on_this_machine_read() {
        // whatever the machine has, each reads as a plausible temperature
        let inputs = scan(Path::new("/sys/class/hwmon"));
        let mut text = String::new();
        for sensor in read_sensors(&inputs, &mut text).unwrap_or_default() {
            assert!(
                sensor.celsius > -40.0 && sensor.celsius < 150.0,
                "{sensor:?}"
            );
        }
    }
}
