//! Keeps the screen's brightness in HDR across a config reload and a new
//! session. In HDR, amane's `Brightness` moves niri's SDR white (the
//! panel no longer follows its backlight), which is niri's temporary kind
//! of change: the next config read puts back what `displays.kdl` says. So
//! once a level has rested for a moment it's kept in the shell's own
//! `hdr-white.json`, and set in niri again at the start and after each
//! config read. Shell Settings alone writes `displays.kdl`. A level idle
//! dimmed to ([`dimmed`]) is never kept.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use std::time::{Duration, SystemTime};

use amane::niri::{self, OutputChange};
use amane::serde::{Deserialize, Serialize};
use amane::{Brightness, Service};

use crate::services::Wake;
use crate::services::state::{self, State};

// how long a level stays put before it counts as rested
use amane_kit::sync::Locked as _;
const REST: Duration = Duration::from_secs(1);

static DIMMED: AtomicBool = AtomicBool::new(false);

// the panel's SDR white as last seen, and when it changed
type Seen = (Option<(String, f64)>, SystemTime);
static LATEST: Mutex<Option<Seen>> = Mutex::new(None);
static WAKE: Wake = Wake::new();

/// Whether idle has the screen dimmed, so its level isn't kept.
pub fn dimmed(on: bool) {
    DIMMED.store(on, Ordering::Relaxed);
}

fn hdr_white(brightness: &Brightness) -> Option<(String, f64)> {
    brightness
        .hdr()
        .map(|panel| (panel.output.clone(), panel.sdr_white.round()))
}

/// Starts watching, once.
pub fn watch() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        Brightness::subscribe(|brightness| {
            let now = hdr_white(brightness);
            let mut latest = LATEST.locked();
            if latest.as_ref().is_some_and(|(seen, _)| *seen == now) {
                return;
            }
            *latest = Some((now, SystemTime::now()));
            drop(latest);
            WAKE.nudge();
        })
        .forever();

        // niri starts from displays.kdl's level, and goes back to it at each config read
        std::thread::spawn(|| {
            put_back();
            niri::follow_config_loads(|failed| {
                if !failed {
                    put_back();
                }
            });
        });

        std::thread::spawn(|| {
            // the level as it is now is the one kept
            let mut kept = hdr_white(&Brightness::read());
            loop {
                let latest = LATEST.locked().clone();
                let Some((now, at)) = latest else {
                    WAKE.wait(None);
                    continue;
                };
                // not rested yet: wait out the rest, or the next change
                let rested = at + REST;
                if SystemTime::now() < rested {
                    WAKE.wait(Some(rested));
                    continue;
                }
                if now != kept
                    && !DIMMED.load(Ordering::Relaxed)
                    && let Some((output, nits)) = &now
                {
                    keep(output, *nits);
                    kept.clone_from(&now);
                }
                WAKE.wait(None);
            }
        });
    });
}

/// The SDR white each panel was last left at, by connector.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(crate = "amane::serde")]
struct Kept {
    nits: BTreeMap<String, f64>,
}

impl State for Kept {
    const FILE: &'static str = "hdr-white.json";
    const VERSION: u32 = 1;
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

fn with_kept<R>(f: impl FnOnce(&mut Kept) -> R) -> R {
    let mut kept = KEPT.locked();
    f(kept.get_or_insert_with(|| state::load().unwrap_or_default()))
}

fn keep(output: &str, nits: f64) {
    let kept = with_kept(|kept| {
        kept.nits.insert(output.to_string(), nits);
        kept.clone()
    });
    state::save(&kept);
}

/// Sets each kept level in niri again: niri's own is `displays.kdl`'s.
fn put_back() {
    if !niri::features().has("hdr") {
        return;
    }
    for (output, nits) in with_kept(|kept| kept.nits.clone()) {
        // an output that's gone or out of HDR has no SDR white to set
        if let Err(error) = niri::set_output(&output, OutputChange::SdrBrightness(nits)) {
            log::debug!("SDR white of {output}: {error}");
        }
    }
}
