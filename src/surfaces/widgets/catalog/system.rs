//! The system widgets: the processor, memory, temperature, network and
//! disks, each as a figure over a graph of the last readings, and the
//! System card with all of them at once. They read [`SystemStats`].

use amane::{Service, Widget};

use super::{
    Face, SPARK_POINTS, accent, bar, caption_style, figure_style, header, header_height,
    label_style, line, percent, place, primary, ring_with, secondary, sparkline,
};
use crate::components::layout::{icon, text_height};
use crate::surfaces::widgets::kind::{WidgetKind, WidgetSize};
use crate::services::desktop::system_stats::{Series, SystemStats};
use crate::tokens::sys;

/// Hotter than this reads in the critical colour.
const HOT: f32 = 85.0;
/// A processor this busy, or memory this full, reads in the critical colour.
const BUSY: f32 = 0.9;

/// "9.8 GB", in powers of 1000 as the file managers count.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// "1.2 MB/s".
pub fn rate(per_second: f32) -> String {
    format!("{}/s", bytes(per_second.max(0.0) as u64))
}

fn recent(series: &Series) -> Vec<f32> {
    series.recent(SPARK_POINTS).collect()
}

/// A figure over a caption at the top, and a graph of `series` under them
/// filling the rest: the shape every Small and Medium system widget takes.
#[allow(clippy::too_many_arguments)]
fn reading(
    face: &Face,
    kind: WidgetKind,
    title: &str,
    figure: String,
    caption: String,
    series: &[f32],
    max: f32,
    warn: bool,
) -> Vec<Box<dyn Widget>> {
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
    let graph_h = face.height - y;
    if graph_h > 8.0 {
        out.push(place(
            sparkline(series, max, face.width, graph_h, accent(warn)),
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
    let mut out = reading(
        face,
        WidgetKind::Cpu,
        "CPU",
        percent(now),
        caption,
        &recent(&stats.cpu),
        1.0,
        now >= BUSY,
    );
    if face.size == WidgetSize::Large && cores > 0 {
        // each core as a thin bar along the bottom
        out.truncate(4);
        let bars_h = face.height * 0.28;
        let graph_y = header_height()
            + text_height(figure_style(true))
            + text_height(caption_style())
            + sys::space::gap::s() * 2.0;
        let graph_h = face.height - graph_y - bars_h - sys::space::gap::m();
        out.push(place(
            sparkline(
                &recent(&stats.cpu),
                1.0,
                face.width,
                graph_h,
                accent(now >= BUSY),
            ),
            0.0,
            graph_y,
            face.width,
            graph_h,
        ));
        let gap = 2.0;
        let width = ((face.width + gap) / cores as f32 - gap).max(1.0);
        for (i, load) in stats.cores.iter().enumerate() {
            let height = (bars_h * load).max(2.0);
            out.push(place(
                amane::Rectangle::new()
                    .width(amane::Size::Fixed(width))
                    .height(amane::Size::Fixed(height))
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
    let caption = format!("{} of {}", bytes(memory.used), bytes(memory.total));
    let mut out = reading(
        face,
        WidgetKind::Memory,
        "Memory",
        percent(memory.fraction()),
        caption,
        &recent(&stats.memory_history),
        1.0,
        memory.fraction() >= BUSY,
    );
    if face.size == WidgetSize::Large && memory.swap_total > 0 {
        let caption = caption_style();
        let y = face.height - text_height(caption) * 2.0 - sys::space::gap::xs();
        out.push(line(
            format!(
                "Swap {} of {}",
                bytes(memory.swap_used),
                bytes(memory.swap_total)
            ),
            caption,
            secondary(),
            (0.0, y),
            face.width,
            false,
        ));
        out.push(place(
            bar(memory.swap_fraction(), face.width, 4.0, accent(false)),
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
    let Some(sensor) = stats.sensor(face.settings.sensor.as_deref()) else {
        return reading(
            face,
            WidgetKind::Temperature,
            "Temperature",
            String::from("—"),
            String::from("No sensor"),
            &[],
            1.0,
            false,
        );
    };
    let history = stats
        .temperature_history(&sensor.label)
        .map(recent)
        .unwrap_or_default();
    let max = history.iter().copied().fold(HOT, f32::max) + 5.0;
    let mut out = reading(
        face,
        WidgetKind::Temperature,
        "Temperature",
        format!("{:.0}°", sensor.celsius),
        sensor.label.clone(),
        &history,
        max,
        sensor.celsius >= HOT,
    );
    if face.size == WidgetSize::Large {
        // the other sensors under the graph, hottest first
        let mut others: Vec<_> = stats
            .sensors
            .iter()
            .filter(|s| s.label != sensor.label)
            .collect();
        others.sort_by(|a, b| b.celsius.total_cmp(&a.celsius));
        let caption = caption_style();
        let row = text_height(caption) + sys::space::gap::xs();
        let rows = 4.min(others.len());
        let top = face.height - row * rows as f32;
        out.truncate(4);
        let graph_y = header_height()
            + text_height(figure_style(true))
            + text_height(caption)
            + sys::space::gap::s() * 2.0;
        out.push(place(
            sparkline(
                &history,
                max,
                face.width,
                top - graph_y - sys::space::gap::s(),
                accent(sensor.celsius >= HOT),
            ),
            0.0,
            graph_y,
            face.width,
            top - graph_y - sys::space::gap::s(),
        ));
        for (i, other) in others.iter().take(rows).enumerate() {
            let y = top + row * i as f32;
            out.push(line(
                other.label.clone(),
                caption,
                secondary(),
                (0.0, y),
                face.width * 0.75,
                false,
            ));
            out.push(line(
                format!("{:.0}°", other.celsius),
                caption,
                primary(),
                (0.0, y),
                face.width,
                true,
            ));
        }
    }
    out
}

pub(super) fn network(face: &Face) -> Vec<Box<dyn Widget>> {
    let stats = SystemStats::read();
    let Some((name, traffic)) = stats.interface(face.settings.interface.as_deref()) else {
        return reading(
            face,
            WidgetKind::Network,
            "Network",
            String::from("—"),
            String::from("Not connected"),
            &[],
            1.0,
            false,
        );
    };
    let down: Vec<f32> = recent(&traffic.down);
    let up: Vec<f32> = recent(&traffic.up);
    let max = down.iter().chain(&up).copied().fold(64_000.0, f32::max);
    let now_down = traffic.down.last().unwrap_or(0.0);
    let now_up = traffic.up.last().unwrap_or(0.0);
    let mut out = header(WidgetKind::Network, name, face.width);
    let figure = figure_style(face.size != WidgetSize::Small);
    let figure = crate::tokens::TextStyle {
        size: figure.size * 0.75,
        ..figure
    };
    let line_h = text_height(figure);
    let mut y = header_height() + sys::space::gap::xs();
    for (arrow, value) in [('↓', now_down), ('↑', now_up)] {
        out.push(line(
            format!("{arrow} {}", rate(value)),
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
        out.push(place(
            sparkline(&down, max, face.width, graph_h, accent(false)),
            0.0,
            y,
            face.width,
            graph_h,
        ));
        out.push(place(
            sparkline(&up, max, face.width, graph_h, secondary()),
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
    let mut out = header(WidgetKind::Disk, "Disk", face.width);
    if face.size == WidgetSize::Medium {
        // every disk as a bar, the root first
        let label = label_style();
        let caption = caption_style();
        let row = text_height(label) + 4.0 + sys::space::gap::m();
        let mut y = header_height() + sys::space::gap::m();
        for disk in &stats.disks {
            if y + row > face.height + sys::space::gap::m() {
                break;
            }
            out.push(line(
                disk.mount.clone(),
                label,
                primary(),
                (0.0, y),
                face.width * 0.5,
                false,
            ));
            out.push(line(
                format!("{} free", bytes(disk.total - disk.used)),
                caption,
                secondary(),
                (0.0, y),
                face.width,
                true,
            ));
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
        return out;
    }
    let Some(disk) = stats.disk(face.settings.mount.as_deref()) else {
        out.push(line(
            "No disk",
            label_style(),
            secondary(),
            (0.0, face.height / 2.0),
            face.width,
            false,
        ));
        return out;
    };
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
    out.push(line(
        format!("{} · {} free", disk.mount, bytes(disk.total - disk.used)),
        caption,
        secondary(),
        (0.0, face.height - text_height(caption)),
        face.width,
        false,
    ));
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
    let cpu = stats.cpu.last().unwrap_or(0.0);
    let memory = stats.memory.fraction();
    let disk = stats.disk(None).map_or(0.0, |d| d.fraction());
    let traffic = stats.interface(None);
    // what, its name, how full (none for the network), what it reads, and
    // its graph
    type Row<'a> = (WidgetKind, &'a str, f32, String, Option<Vec<f32>>);
    let rows: [Row; 4] = [
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
                bytes(stats.memory.used),
                bytes(stats.memory.total)
            ),
            Some(recent(&stats.memory_history)),
        ),
        (
            WidgetKind::Disk,
            "Disk",
            disk,
            stats
                .disk(None)
                .map_or_else(String::new, |d| format!("{} free", bytes(d.total - d.used))),
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
                        rate(t.down.last().unwrap_or(0.0)),
                        rate(t.up.last().unwrap_or(0.0))
                    )
                },
            ),
            traffic.map(|(_, t)| recent(&t.down)),
        ),
    ];
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
            amane::Stack::new(vec![crate::components::layout::boxed(icon(
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
                series.iter().copied().fold(64_000.0, f32::max)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_as_the_file_managers_count() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1_500), "1.5 KB");
        assert_eq!(bytes(9_800_000_000), "9.8 GB");
        assert_eq!(bytes(512_000_000_000), "512 GB");
        assert_eq!(rate(1_234_567.0), "1.2 MB/s");
        assert_eq!(rate(-3.0), "0 B/s");
    }
}
