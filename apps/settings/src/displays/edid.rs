//! What a monitor says it can do, read from its EDID: the block it hands
//! the kernel (`/sys/class/drm/card*-<connector>/edid`).
//!
//! Only what the Displays page shows is read: the name, the bit depth and
//! interface, the colour primaries, the refresh range for variable refresh
//! (the range limits descriptor, or AMD's FreeSync block), and from the
//! CTA-861 extension the colorimetry and the HDR static metadata. Anything
//! else, or a block that's cut short, is left out rather than guessed.

/// A display's EDID, as far as the page needs it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edid {
    /// The three-letter PNP id of the maker (`LEN`).
    pub vendor: String,
    /// The name descriptor (`ATNA40HQ10-0`).
    pub name: Option<String>,
    /// Bits per colour the panel takes, on a digital input.
    pub bits: Option<u8>,
    /// The digital interface: `DisplayPort`, `HDMI`…
    pub interface: Option<&'static str>,
    /// The red, green, blue and white points, CIE 1931 xy.
    pub primaries: Option<[(f32, f32); 4]>,
    /// The lowest and highest refresh rate it runs at, in Hz.
    pub refresh: Option<(u16, u16)>,
    /// Whether AMD's FreeSync block is there.
    pub freesync: bool,
    /// The wide colour encodings it accepts, by CTA-861 name.
    pub colorimetry: Vec<&'static str>,
    pub hdr: Option<Hdr>,
}

/// The HDR static metadata block: which transfer functions it decodes, and
/// how bright it gets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hdr {
    /// SMPTE ST 2084, HDR10's curve.
    pub pq: bool,
    /// Hybrid log-gamma, broadcast HDR's curve.
    pub hlg: bool,
    /// Peak, in nits.
    pub max: Option<f32>,
    /// The most a whole frame holds on average, in nits.
    pub average: Option<f32>,
    /// The darkest, in nits.
    pub min: Option<f32>,
}

/// How wide a gamut the primaries span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gamut {
    Srgb,
    /// Wider than sRGB, short of P3.
    Wide,
    P3,
    Bt2020,
}

impl Gamut {
    pub fn label(self) -> &'static str {
        match self {
            Gamut::Srgb => "sRGB",
            Gamut::Wide => "Wide color",
            Gamut::P3 => "P3",
            Gamut::Bt2020 => "BT.2020",
        }
    }
}

const SRGB: [(f32, f32); 3] = [(0.640, 0.330), (0.300, 0.600), (0.150, 0.060)];
const P3: [(f32, f32); 3] = [(0.680, 0.320), (0.265, 0.690), (0.150, 0.060)];
const BT2020: [(f32, f32); 3] = [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)];

impl Edid {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
        if bytes.len() < 128 || bytes[..8] != HEADER {
            return None;
        }

        let mut edid = Edid {
            vendor: vendor(bytes[8], bytes[9]),
            primaries: Some(primaries(&bytes[25..35])),
            ..Edid::default()
        };

        let input = bytes[20];
        if input & 0x80 != 0 {
            edid.bits = match (input >> 4) & 0x07 {
                1 => Some(6),
                2 => Some(8),
                3 => Some(10),
                4 => Some(12),
                5 => Some(14),
                6 => Some(16),
                _ => None,
            };
            edid.interface = match input & 0x0f {
                1 => Some("DVI"),
                2 | 3 => Some("HDMI"),
                4 => Some("MDDI"),
                5 => Some("DisplayPort"),
                _ => None,
            };
        }

        for descriptor in bytes[54..126].as_chunks::<18>().0 {
            // a detailed timing has a pixel clock; a descriptor starts with zeros
            if descriptor[0] != 0 || descriptor[1] != 0 {
                continue;
            }
            match descriptor[3] {
                0xfc => edid.name = text(&descriptor[5..18]),
                0xfd => {
                    // the offset flags add 255 to a rate past 255 Hz
                    let low =
                        u16::from(descriptor[5]) + if descriptor[4] & 0x01 != 0 { 255 } else { 0 };
                    let high =
                        u16::from(descriptor[6]) + if descriptor[4] & 0x02 != 0 { 255 } else { 0 };
                    if low > 0 && high > low {
                        edid.refresh = Some((low, high));
                    }
                }
                _ => {}
            }
        }

        let extensions = usize::from(bytes[126]);
        for block in bytes[128..].as_chunks::<128>().0.iter().take(extensions) {
            if block[0] == 0x02 {
                edid.read_cta(block);
            }
        }

        Some(edid)
    }

    /// The CTA-861 extension's data blocks, between its header and the
    /// detailed timings at the offset its third byte gives.
    fn read_cta(&mut self, block: &[u8]) {
        let end = usize::from(block[2]).clamp(4, 127);
        let mut at = 4;
        while at < end {
            let tag = block[at] >> 5;
            let len = usize::from(block[at] & 0x1f);
            let Some(data) = block.get(at + 1..at + 1 + len) else {
                break;
            };
            match tag {
                // vendor-specific: AMD's FreeSync block, with its range
                3 if data.len() >= 7 && data[..3] == [0x1a, 0x00, 0x00] => {
                    self.freesync = true;
                    let (low, high) = (u16::from(data[5]), u16::from(data[6]));
                    if self.refresh.is_none() && low > 0 && high > low {
                        self.refresh = Some((low, high));
                    }
                }
                7 if !data.is_empty() => self.read_extended(data[0], &data[1..]),
                _ => {}
            }
            at += 1 + len;
        }
    }

    fn read_extended(&mut self, tag: u8, data: &[u8]) {
        match tag {
            // colorimetry
            0x05 if !data.is_empty() => {
                const NAMES: [&str; 8] = [
                    "xvYCC601",
                    "xvYCC709",
                    "sYCC601",
                    "opYCC601",
                    "opRGB",
                    "BT2020cYCC",
                    "BT2020YCC",
                    "BT2020RGB",
                ];
                self.colorimetry = (0..8)
                    .filter(|bit| data[0] & (1 << bit) != 0)
                    .map(|bit| NAMES[bit])
                    .collect();
                if data.get(1).is_some_and(|more| more & 0x80 != 0) {
                    self.colorimetry.push("DCI-P3");
                }
            }
            // HDR static metadata
            0x06 if data.len() >= 2 => {
                let eotf = data[0];
                let max = data.get(2).filter(|v| **v != 0).map(|v| luminance(*v));
                self.hdr = Some(Hdr {
                    pq: eotf & 0x04 != 0,
                    hlg: eotf & 0x08 != 0,
                    max,
                    average: data.get(3).filter(|v| **v != 0).map(|v| luminance(*v)),
                    min: match (max, data.get(4)) {
                        (Some(max), Some(v)) => Some(max * (f32::from(*v) / 255.0).powi(2) / 100.0),
                        _ => None,
                    },
                });
            }
            _ => {}
        }
    }

    /// Takes HDR10: PQ decoding, with a peak worth the name.
    pub fn hdr10(&self) -> bool {
        self.hdr.as_ref().is_some_and(|hdr| hdr.pq)
    }

    /// The gamut its primaries cover, by the triangle's area against the
    /// standards': only an estimate, as the primaries don't say how far each
    /// one is reached.
    pub fn gamut(&self) -> Option<Gamut> {
        let [r, g, b, _] = self.primaries?;
        let area = triangle([r, g, b]);
        if area <= 0.0 {
            return None;
        }
        Some(if area >= triangle(BT2020) * 0.95 {
            Gamut::Bt2020
        } else if area >= triangle(P3) * 0.95 {
            Gamut::P3
        } else if area >= triangle(SRGB) * 1.1 {
            Gamut::Wide
        } else {
            Gamut::Srgb
        })
    }
}

/// The CTA-861 code value for a luminance: 50 · 2^(v/32) nits.
fn luminance(value: u8) -> f32 {
    50.0 * 2f32.powf(f32::from(value) / 32.0)
}

/// Three letters, five bits each, `A` being 1.
fn vendor(high: u8, low: u8) -> String {
    let id = u16::from_be_bytes([high, low]);
    [10, 5, 0]
        .into_iter()
        .map(|shift| ((id >> shift) & 0x1f) as u8)
        .filter(|letter| (1..=26).contains(letter))
        .map(|letter| char::from(b'A' + letter - 1))
        .collect()
}

/// Up to 13 characters, ended by a newline and padded with spaces.
fn text(bytes: &[u8]) -> Option<String> {
    let text: String = bytes
        .iter()
        .take_while(|byte| **byte != b'\n' && **byte != 0)
        .map(|byte| char::from(*byte))
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Red, green, blue and white xy: ten bits each, the low two packed in the
/// first two bytes.
fn primaries(bytes: &[u8]) -> [(f32, f32); 4] {
    let low = u16::from(bytes[0]) << 8 | u16::from(bytes[1]);
    let coordinate = |index: usize| {
        let high = u16::from(bytes[2 + index]) << 2;
        let low = (low >> (14 - index * 2)) & 0x03;
        f32::from(high | low) / 1024.0
    };
    [0, 1, 2, 3].map(|point| (coordinate(point * 2), coordinate(point * 2 + 1)))
}

fn triangle([a, b, c]: [(f32, f32); 3]) -> f32 {
    ((b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1)).abs() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ThinkPad T14 Gen 7's OLED panel.
    const PANEL: &[u8] = include_bytes!("atna40hq10.edid");

    #[test]
    fn reads_the_oled_panel() {
        let edid = Edid::parse(PANEL).unwrap();
        assert_eq!(edid.vendor, "LEN");
        assert_eq!(edid.name.as_deref(), Some("ATNA40HQ10-0"));
        assert_eq!(edid.bits, Some(10));
        assert_eq!(edid.interface, Some("DisplayPort"));
        assert_eq!(edid.refresh, Some((30, 120)));
        assert!(edid.freesync);
        assert_eq!(edid.colorimetry, ["BT2020RGB"]);
        assert_eq!(edid.gamut(), Some(Gamut::P3));

        let hdr = edid.hdr.unwrap();
        assert!(hdr.pq && !hdr.hlg);
        assert_eq!(hdr.max.unwrap().round(), 1107.0);
        assert_eq!(hdr.average.unwrap().round(), 497.0);
        assert!(hdr.min.unwrap() < 0.001);
    }

    #[test]
    fn the_primaries_unpack() {
        let [red, green, blue, _] = Edid::parse(PANEL).unwrap().primaries.unwrap();
        assert!((red.0 - 0.681).abs() < 0.001 && (red.1 - 0.3184).abs() < 0.001);
        assert!((green.0 - 0.220).abs() < 0.001 && (green.1 - 0.725).abs() < 0.001);
        assert!((blue.0 - 0.143).abs() < 0.001 && (blue.1 - 0.039).abs() < 0.001);
    }

    #[test]
    fn a_short_or_foreign_block_is_none() {
        assert_eq!(Edid::parse(&PANEL[..100]), None);
        assert_eq!(Edid::parse(&[0u8; 128]), None);
    }

    #[test]
    fn a_cut_extension_stops_reading_without_panicking() {
        let mut bytes = PANEL[..256].to_vec();
        // a data block claiming more than the extension holds
        bytes[128 + 2] = 127;
        bytes[128 + 4] = 0x7f;
        let edid = Edid::parse(&bytes).unwrap();
        assert_eq!(edid.name.as_deref(), Some("ATNA40HQ10-0"));
    }
}
