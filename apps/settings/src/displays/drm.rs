//! What the kernel knows about a connector that its EDID doesn't say: the
//! panel's kind (amdgpu marks OLED panels), whether the link can do
//! variable refresh, how many bits per colour the driver may send, and the
//! colour encodings it can signal. Read through the card's DRM node
//! without becoming its master, so it never disturbs the compositor.

use std::fs::{self, File, OpenOptions};
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};

use drm::control::{Device as ControlDevice, property};

/// A connector's properties; each `None` when the driver has no such one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Connector {
    /// `panel_type`: "OLED" on an OLED panel amdgpu knows.
    pub panel: Option<String>,
    pub vrr_capable: Option<bool>,
    /// The most bits per colour `max bpc` allows.
    pub max_bpc: Option<u64>,
    /// The `Colorspace` encodings the link can signal.
    pub colorspaces: Vec<String>,
    /// Whether it takes `HDR_OUTPUT_METADATA`, which HDR output needs.
    pub hdr_metadata: bool,
}

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Card {}
impl ControlDevice for Card {}

/// The sysfs folder of a connector by niri's name for it (`eDP-1`), on
/// whichever card has it.
pub fn sysfs(connector: &str) -> Option<PathBuf> {
    fs::read_dir("/sys/class/drm")
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.split_once('-'))
                .is_some_and(|(card, rest)| card.starts_with("card") && rest == connector)
        })
}

/// The connector's EDID, as the kernel read it.
pub fn edid(connector: &str) -> Option<Vec<u8>> {
    let bytes = fs::read(sysfs(connector)?.join("edid")).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

/// Its properties, from the card it's on.
pub fn connector(connector: &str) -> Option<Connector> {
    let folder = sysfs(connector)?;
    let card = folder.file_name()?.to_str()?.split_once('-')?.0.to_string();
    read(&Path::new("/dev/dri").join(card), connector)
}

fn read(node: &Path, wanted: &str) -> Option<Connector> {
    let card = Card(OpenOptions::new().read(true).open(node).ok()?);
    let resources = card.resource_handles().ok()?;

    let handle = resources.connectors().iter().copied().find(|handle| {
        card.get_connector(*handle, false).is_ok_and(|info| {
            format!("{}-{}", info.interface().as_str(), info.interface_id()) == wanted
        })
    })?;

    let mut found = Connector::default();
    let set = card.get_properties(handle).ok()?;
    let (handles, values) = set.as_props_and_values();
    for (prop, raw) in handles.iter().zip(values) {
        let Ok(info) = card.get_property(*prop) else {
            continue;
        };
        let name = info.name().to_string_lossy();
        let kind = info.value_type();
        match name.as_ref() {
            "panel_type" => found.panel = enum_name(&kind, *raw),
            "vrr_capable" => found.vrr_capable = Some(*raw != 0),
            "max bpc" => {
                if let property::ValueType::UnsignedRange(_, most) = kind {
                    found.max_bpc = Some(most);
                }
            }
            "Colorspace" => {
                if let property::ValueType::Enum(values) = &kind {
                    found.colorspaces = values
                        .values()
                        .1
                        .iter()
                        .map(|value| value.name().to_string_lossy().into_owned())
                        .filter(|name| name != "Default")
                        .collect();
                }
            }
            "HDR_OUTPUT_METADATA" => found.hdr_metadata = true,
            _ => {}
        }
    }
    Some(found)
}

fn enum_name(kind: &property::ValueType, raw: property::RawValue) -> Option<String> {
    match kind {
        property::ValueType::Enum(values) => values
            .get_value_from_raw_value(raw)
            .map(|value| value.name().to_string_lossy().into_owned())
            .filter(|name| name != "unknown"),
        _ => None,
    }
}
