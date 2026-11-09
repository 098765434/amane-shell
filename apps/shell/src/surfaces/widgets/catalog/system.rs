//! The system widgets: the processor, memory, temperature, network and
//! disks, each as a figure over a graph of the last readings, and the
//! System card with all of them at once. They read [`SystemStats`].

use amane::{Service, Widget};

use super::units::{self, degrees, graph, interface_kind, or_automatic, sensor_name};
use super::{
    Face, SPARK_POINTS, accent, bar, caption_style, empty, figure_style, header, header_height,
    label_style, line, percent, place, primary, ring_with, secondary, sparkline, sparkline_between,
};
use crate::services::desktop::system_stats::{Series, SystemStats};
use crate::surfaces::widgets::kind::{WidgetKind, WidgetSize};
use crate::tokens::sys;
use emaki_kit::layout::{icon, text_height, text_width};

/// Hotter than this reads in the critical colour, for a sensor that
/// doesn't say where it runs hot.
const HOT: f32 = 85.0;
/// A processor this busy, or memory this full, reads in the critical colour.
const BUSY: f32 = 0.9;
/// The least a network graph's top stands for, in bytes a second, so a
/// quiet link doesn't fill it with noise.
const NETWORK_FLOOR: f32 = 64_000.0;

fn recent(series: &Series) -> Vec<f32> {
    series.recent(SPARK_POINTS).collect()
}

/// What a reading shows: a figure over a caption, and a graph of `series`
/// (up to `max`), in the warning colour when `warn`, ending `below` above
/// the bottom so a Large face has room for more under it.
struct Reading<'a> {
    kind: WidgetKind,
    title: &'a str,
    figure: String,
    caption: String,
    series: &'a [f32],
    min: f32,
    max: f32,
    warn: bool,
    below: f32,
}

/// A figure over a caption at the top, and a graph of the series under
/// them filling the rest: the shape every Small and Medium system widget
/// takes.
fn reading(face: &Face, reading: Reading) -> Vec<Box<dyn Widget>> {
    let Reading {
        kind,
        title,
        figure,
        caption,
        series,
        min,
        max,
        warn,
        below,
    } = reading;
    let mut out = header(kind, title, face.width);
    let large = face.size != WidgetSize::Small;
    let figure_style = figure_style(large);
    let mut y = header_height() + sys::space::gap::xs();
    out.push(line(
        figure,
        figure_style,
        primary(),
        (0.0, y),
        face.width,
        false,
    ));
    y += text_height(figure_style);
    out.push(line(
        caption,
        caption_style(),
        secondary(),
        (0.0, y),
        face.width,
        false,
    ));
    y += text_height(caption_style()) + sys::space::gap::s();
    let graph_h = face.height - y - below;
    if graph_h > 8.0 {
        out.push(place(
            sparkline_between(
                series,
                (min, max),
                (face.width, graph_h),
                accent(warn),
                true,
            ),
            0.0,
            y,
            face.width,
            graph_h,
        ));
    }
    out
}

pub(super) fn cpu(face: &Face) -> Vec<Box<dyn Widget>> {
    let stats = SystemStats::read();
    let now = stats.cpu.last().unwrap_or(0.0);
    let cores = stats.cores.len();
    let caption = if cores > 0 {
        format!("{cores} cores")
    } else {
        String::new()
    };
    // a Large one has each core as a thin bar along the bottom
    let per_core = face.size == WidgetSize::Large && cores > 0;
    let bars_h = face.height * 0.28;
    let mut out = reading(
        face,
        Reading {
            kind: WidgetKind::Cpu,
            title: "CPU",
            figure: percent(now),
            caption,
            series: &graph(&stats.cpu, face.settings),
            min: 0.0,
            max: 1.0,
            warn: now >= BUSY,
            below: if per_core {
                bars_h + sys::space::gap::m()
            } else {
                0.0
            },
        },
    );
    if per_core {
        let gap = 2.0;
        let width = ((face.width + gap) / cores as f32 - gap).max(1.0);
        for (i, load) in stats.cores.iter().enumerate() {
            let height = (bars_h * load).max(2.0);
            out.push(place(
                amane::Rectangle::new()
                    .width(width)
                    .height(height)
                    .radius(width.min(3.0) / 2.0)
                    .fill(accent(*load >= BUSY)),
                i as f32 * (width + gap),
                face.height - height,
                width,
                height,
            ));
        }
    }
    out
}

pub(super) fn memory(face: &Face) -> Vec<Box<dyn Widget>> {
    let stats = SystemStats::read();
    let memory = stats.memory;
    let caption = format!(
        "{} of {}",
        emaki_kit::format::bytes(memory.used),
        emaki_kit::format::bytes(memory.total)
    );
    // a Large one has the swap under the graph: a line over a bar
    let swap = face.size == WidgetSize::Large && memory.swap_total > 0;
    let swap_h = text_height(caption_style()) * 2.0 + sys::space::gap::xs();
    let mut out = reading(
        face,
        Reading {
            kind: WidgetKind::Memory,
            title: "Memory",
            figure: percent(memory.fraction()),
            caption,
            series: &graph(&stats.memory_history, face.settings),
            min: 0.0,
            max: 1.0,
            warn: memory.fraction() >= BUSY,
            below: if swap {
                swap_h + sys::space::gap::s()
            } else {
                0.0
            },
        },
    );
    if swap {
        let caption = caption_style();
        let y = face.height - swap_h;
        out.push(line(
            format!(
                "Swap {} of {}",
                emaki_kit::format::bytes(memory.swap_used),
                emaki_kit::format::bytes(memory.swap_total)
            ),
            caption,
            secondary(),
            (0.0, y),
            face.width,
            false,
        ));
        out.push(place(
            bar(
                memory.swap_fraction(),
                face.width,
                4.0,
                accent(memory.swap_fraction() >= BUSY),
            ),
            0.0,
            face.height - 4.0,
            face.width,
            4.0,
        ));
    }
    out
}

pub(super) fn temperature(face: &Face) -> Vec<Box<dyn Widget>> {
    let stats = SystemStats::read();
    let wanted = face.settings.get("sensor");
    let (sensor, gone) = or_automatic(wanted, |label| stats.sensor(label));
    let Some(sensor) = sensor else {
        return reading(
            face,
            Reading {
                kind: WidgetKind::Temperature,
                title: "Temperature",
                figure: String::from("—"),
                caption: String::from("No sensor"),
                series: &[],
                min: 0.0,
                max: 1.0,
                warn: false,
                below: 0.0,
            },
        );
    };
    let history = stats
        .temperature_history(&sensor.label)
        .map(|series| graph(series, face.settings))
        .unwrap_or_default();
    // the graph spans what it read, so a change of a few degrees shows
    let low = history.iter().copied().fold(sensor.celsius, f32::min);
    let high = history.iter().copied().fold(sensor.celsius, f32::max);
    let min = (low - 5.0).floor().max(0.0);
    let max = (high + 5.0).max(min + 20.0);
    let hot =
        |s: &crate::services::desktop::system_stats::Sensor| s.celsius >= s.high.unwrap_or(HOT);
    // a Large one has the other sensors under the graph, hottest first
    let mut others: Vec<_> = stats
        .sensors
        .iter()
        .filter(|s| s.label != sensor.label)
        .collect();
    others.sort_by(|a, b| b.celsius.total_cmp(&a.celsius));
    let caption = caption_style();
    let row = text_height(caption) + sys::space::gap::xs();
    let rows = if face.size == WidgetSize::Large {
        4.min(others.len())
    } else {
        0
    };
    let top = face.height - row * rows as f32;
    let name = sensor_name(&sensor.label);
    let mut out = reading(
        face,
        Reading {
            kind: WidgetKind::Temperature,
            title: "Temperature",
            figure: degrees(sensor.celsius, face.settings),
            // the one it was set to went away: says so, over the automatic one
            caption: match wanted.filter(|_| gone) {
                Some(wanted) => format!("{name} · {} not found", sensor_name(wanted)),
                None => name,
            },
            series: &history,
            min,
            max,
            warn: hot(sensor),
            below: if rows > 0 {
                face.height - top + sys::space::gap::s()
            } else {
                0.0
            },
        },
    );
    for (i, other) in others.iter().take(rows).enumerate() {
        let y = top + row * i as f32;
        let value = degrees(other.celsius, face.settings);
        let value_w = text_width(&value, caption) + 1.0;
        out.push(line(
            sensor_name(&other.label),
            caption,
            secondary(),
            (0.0, y),
            face.width - value_w - sys::space::gap::s(),
            false,
        ));
        out.push(line(
            value,
            caption,
            accent_if(hot(other)),
            (0.0, y),
            face.width,
            true,
        ));
    }
    out
}

/// The primary label colour, or the critical colour when `warn`.
fn accent_if(warn: bool) -> amane::Color {
    if warn { accent(true) } else { primary() }
}

pub(super) fn network(face: &Face) -> Vec<Box<dyn Widget>> {
    let stats = SystemStats::read();
    let wanted = face.settings.get("interface");
    let (found, gone) = or_automatic(wanted, |name| stats.interface(name));
    let Some((name, traffic)) = found else {
        return reading(
            face,
            Reading {
                kind: WidgetKind::Network,
                title: "Network",
                figure: String::from("—"),
                caption: match wanted {
                    Some(wanted) => format!("{wanted} not connected"),
                    None => String::from("Not connected"),
                },
                series: &[],
                min: 0.0,
                max: 1.0,
                warn: false,
                below: 0.0,
            },
        );
    };
    let bits = units::in_bits(face.settings);
    let down = graph(&traffic.down, face.settings);
    let up = graph(&traffic.up, face.settings);
    let max = down
        .iter()
        .chain(&up)
        .copied()
        .fold(NETWORK_FLOOR, f32::max);
    let now_down = traffic.down.last().unwrap_or(0.0);
    let now_up = traffic.up.last().unwrap_or(0.0);
    // what kind of link, and the one it was set to when that's gone
    let title = match wanted.filter(|_| gone) {
        Some(wanted) => format!("{} · {wanted} not connected", interface_kind(name)),
        None => format!("{} · {name}", interface_kind(name)),
    };
    let mut out = header(WidgetKind::Network, &title, face.width);
    let figure = figure_style(face.size != WidgetSize::Small);
    let figure = crate::tokens::TextStyle {
        size: figure.size * 0.75,
        ..figure
    };
    let line_h = text_height(figure);
    let mut y = header_height() + sys::space::gap::xs();
    for (arrow, value) in [('↓', now_down), ('↑', now_up)] {
        out.push(line(
            format!("{arrow} {}", units::rate(value, bits)),
            figure,
            primary(),
            (0.0, y),
            face.width,
            false,
        ));
        y += line_h;
    }
    y += sys::space::gap::s();
    let graph_h = face.height - y;
    if graph_h > 8.0 {
        // what comes in, filled; what goes out, a line over it
        let size = (face.width, graph_h);
        out.push(place(
            sparkline_between(&down, (0.0, max), size, accent(false), true),
            0.0,
            y,
            face.width,
            graph_h,
        ));
        out.push(place(
            sparkline_between(&up, (0.0, max), size, secondary(), false),
            0.0,
            y,
            face.width,
            graph_h,
        ));
    }
    out
}

pub(super) fn disk(face: &Face) -> Vec<Box<dyn Widget>> {
    let stats = SystemStats::read();
    if stats.disks.is_empty() {
        return empty(face, WidgetKind::Disk, "Disk", "No disk");
    }
    if face.size == WidgetSize::Medium {
        return disks(face, &stats);
    }
    let wanted = face.settings.get("mount");
    let (disk, gone) = or_automatic(wanted, |mount| stats.disk(mount));
    let Some(disk) = disk else {
        return empty(face, WidgetKind::Disk, "Disk", "No disk");
    };
    let mut out = header(WidgetKind::Disk, "Disk", face.width);
    let caption = caption_style();
    let figure = figure_style(false);
    let below = text_height(figure) + text_height(caption);
    let diameter =
        (face.height - header_height() - below - sys::space::gap::s()).min(face.width * 0.5);
    out.push(place(
        ring_with(
            disk.fraction(),
            diameter,
            diameter * 0.1,
            accent(disk.fraction() >= BUSY),
            crate::surfaces::widgets::kind::ICON_DISK,
        ),
        0.0,
        header_height() + sys::space::gap::s(),
        diameter,
        diameter,
    ));
    out.push(line(
        percent(disk.fraction()),
        figure,
        primary(),
        (0.0, face.height - below),
        face.width,
        false,
    ));
    let free = emaki_kit::format::bytes(disk.total - disk.used);
    out.push(line(
        // the one it was set to went away: says so, over the main disk
        match wanted.filter(|_| gone) {
            Some(wanted) => format!("{wanted} not mounted"),
            None => format!("{} · {free} free", disk.mount),
        },
        caption,
        secondary(),
        (0.0, face.height - text_height(caption)),
        face.width,
        false,
    ));
    out
}

/// Every disk as a bar, the root first, its mount at the start and
/// what's free at the end.
fn disks(face: &Face, stats: &SystemStats) -> Vec<Box<dyn Widget>> {
    let mut out = header(WidgetKind::Disk, "Disk", face.width);
    let label = label_style();
    let caption = caption_style();
    let row = text_height(label) + 4.0 + sys::space::gap::m();
    let mut y = header_height() + sys::space::gap::m();
    for disk in &stats.disks {
        if y + row > face.height + sys::space::gap::m() {
            break;
        }
        let free = format!("{} free", emaki_kit::format::bytes(disk.total - disk.used));
        let free_w = text_width(&free, caption) + 1.0;
        out.push(line(
            disk.mount.clone(),
            label,
            primary(),
            (0.0, y),
            face.width - free_w - sys::space::gap::s(),
            false,
        ));
        out.push(line(free, caption, secondary(), (0.0, y), face.width, true));
        let bar_y = y + text_height(label) + 2.0;
        out.push(place(
            bar(
                disk.fraction(),
                face.width,
                4.0,
                accent(disk.fraction() >= BUSY),
            ),
            0.0,
            bar_y,
            face.width,
            4.0,
        ));
        y += row;
    }
    out
}

/// The System card: a ring each for the processor, memory and the root
/// disk, and the network's rates (Large); with each reading's graph across
/// the extra width (Extra Large).
pub(super) fn overview(face: &Face) -> Vec<Box<dyn Widget>> {
    let stats = SystemStats::read();
    let mut out = header(WidgetKind::System, "System", face.width);
    let top = header_height() + sys::space::gap::m();
    let caption = caption_style();
    let label = label_style();
    let rows = overview_rows(&stats);
    let row_h = (face.height - top) / 4.0;
    let diameter = (row_h - sys::space::gap::s()).min(48.0);
    let wide = face.size == WidgetSize::ExtraLarge;
    let text_w = if wide { face.width * 0.4 } else { face.width } - diameter - sys::space::gap::m();
    for (i, (kind, title, fraction, detail, series)) in rows.into_iter().enumerate() {
        let y = top + row_h * i as f32 + (row_h - diameter) / 2.0;
        let warn = fraction >= BUSY;
        let ring = if fraction >= 0.0 {
            ring_with(
                fraction,
                diameter,
                diameter * 0.1,
                accent(warn),
                kind.glyph(),
            )
        } else {
            amane::Stack::new(vec![emaki_kit::layout::boxed(icon(
                kind.glyph(),
                diameter * 0.5,
                primary(),
            ))])
        };
        let inset = if fraction >= 0.0 {
            0.0
        } else {
            diameter * 0.25
        };
        out.push(place(ring, inset, y + inset, diameter, diameter));
        let x = diameter + sys::space::gap::m();
        let text_y = y + (diameter - text_height(label) - text_height(caption)) / 2.0;
        out.push(line(title, label, primary(), (x, text_y), text_w, false));
        out.push(line(
            detail,
            caption,
            secondary(),
            (x, text_y + text_height(label)),
            text_w,
            false,
        ));
        if wide && let Some(series) = series {
            let graph_x = face.width * 0.4 + sys::space::gap::m();
            let max = if kind == WidgetKind::Network {
                series.iter().copied().fold(NETWORK_FLOOR, f32::max)
            } else {
                1.0
            };
            out.push(place(
                sparkline(&series, max, face.width - graph_x, diameter, accent(warn)),
                graph_x,
                y,
                face.width - graph_x,
                diameter,
            ));
        }
    }
    out
}

/// One of the System card's rows: what, its name, how full (none for the
/// network, as -1), what it reads, and its graph.
type Row = (WidgetKind, &'static str, f32, String, Option<Vec<f32>>);

/// The System card's rows: the processor, memory, the root disk and the
/// network.
fn overview_rows(stats: &SystemStats) -> [Row; 4] {
    let cpu = stats.cpu.last().unwrap_or(0.0);
    let memory = stats.memory.fraction();
    let disk = stats.disk(None).map_or(0.0, |d| d.fraction());
    let traffic = stats.interface(None);
    [
        (
            WidgetKind::Cpu,
            "CPU",
            cpu,
            percent(cpu),
            Some(recent(&stats.cpu)),
        ),
        (
            WidgetKind::Memory,
            "Memory",
            memory,
            format!(
                "{} of {}",
                emaki_kit::format::bytes(stats.memory.used),
                emaki_kit::format::bytes(stats.memory.total)
            ),
            Some(recent(&stats.memory_history)),
        ),
        (
            WidgetKind::Disk,
            "Disk",
            disk,
            stats.disk(None).map_or_else(
                || String::from("No disk"),
                |d| format!("{} free", emaki_kit::format::bytes(d.total - d.used)),
            ),
            None,
        ),
        (
            WidgetKind::Network,
            "Network",
            -1.0,
            traffic.map_or_else(
                || String::from("Not connected"),
                |(_, t)| {
                    format!(
                        "↓ {}  ↑ {}",
                        units::rate(t.down.last().unwrap_or(0.0), false),
                        units::rate(t.up.last().unwrap_or(0.0), false)
                    )
                },
            ),
            traffic.map(|(_, t)| recent(&t.down)),
        ),
    ]
}
