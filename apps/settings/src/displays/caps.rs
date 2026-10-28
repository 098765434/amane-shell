//! What one screen can do, put together from its EDID ([`edid`]) and its
//! connector's properties ([`drm`]). Read again only when the EDID
//! changes, as when another monitor goes into the same port.

use super::drm::{self, Connector};
use super::edid::{Edid, Gamut};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Caps {
    // what it was read from, to notice a different monitor
    raw: Vec<u8>,
    pub edid: Option<Edid>,
    pub connector: Option<Connector>,
}

impl Caps {
    /// The screen on `connector`, reusing `cached` when its EDID is the
    /// same.
    pub fn read(connector: &str, cached: Option<&Caps>) -> Caps {
        let raw = drm::edid(connector).unwrap_or_default();
        if let Some(cached) = cached
            && cached.raw == raw
        {
            return cached.clone();
        }
        Caps {
            edid: Edid::parse(&raw),
            connector: drm::connector(connector),
            raw,
        }
    }

    pub fn oled(&self) -> bool {
        self.connector
            .as_ref()
            .and_then(|connector| connector.panel.as_deref())
            .is_some_and(|panel| panel.eq_ignore_ascii_case("oled"))
    }

    /// The variable refresh range, in Hz.
    pub fn refresh_range(&self) -> Option<(u16, u16)> {
        self.edid.as_ref()?.refresh
    }

    pub fn freesync(&self) -> bool {
        self.edid.as_ref().is_some_and(|edid| edid.freesync)
    }

    /// HDR10 on both ends: the panel decodes PQ and the link can carry the
    /// metadata.
    pub fn hdr10(&self) -> bool {
        self.edid.as_ref().is_some_and(Edid::hdr10)
            && self
                .connector
                .as_ref()
                .is_none_or(|connector| connector.hdr_metadata)
    }

    pub fn peak_nits(&self) -> Option<f32> {
        self.edid.as_ref()?.hdr.as_ref()?.max
    }

    pub fn gamut(&self) -> Option<Gamut> {
        self.edid.as_ref()?.gamut()
    }

    /// Bits per colour: the panel's own, else the most the link allows.
    pub fn bits(&self) -> Option<u64> {
        self.edid
            .as_ref()
            .and_then(|edid| edid.bits)
            .map(u64::from)
            .or_else(|| self.connector.as_ref()?.max_bpc)
    }

    pub fn wide_encoding(&self) -> bool {
        self.edid.as_ref().is_some_and(|edid| {
            edid.colorimetry
                .iter()
                .any(|name| name.starts_with("BT2020"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::displays::niri;

    /// Reads this machine's screens through niri, sysfs and DRM:
    /// `cargo test -p amane-settings -- --ignored --nocapture live`.
    #[test]
    #[ignore = "needs niri and a screen"]
    fn live_screens() {
        for (name, output) in niri::outputs().expect("niri answers") {
            let caps = Caps::read(&name, None);
            println!("{name}: {output:?}\n{caps:#?}");
            assert!(caps.edid.is_some(), "{name} has an EDID");
        }
    }
}
