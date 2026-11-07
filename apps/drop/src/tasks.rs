//! What a tile does with what's let go on it, each on a thread of its own:
//! its progress shows on the tile and as an activity in the bar, with
//! Cancel. A name
//! taken where files go asks first: keep both, replace, or stop.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use std::sync::Mutex;

use amane::KdeConnect;
use amane_kit::settings::UploadTo;
use amane_status::LiveActivity;
use amane_status::{
    ActivityButton, ActivityContent, ActivityEvent, Dismiss, ItemIcon, Tint, Trailing,
};

use crate::state::{self, Outcome, Prompt};
use crate::{files, share, shared};

/// How often progress is told, at most.
const TELL_EVERY: Duration = Duration::from_millis(150);

/// Something to do, for the tile `tile`.
#[derive(Clone, Debug)]
pub enum Job {
    Copy {
        files: Vec<PathBuf>,
        folder: PathBuf,
        tile: usize,
    },
    Move {
        files: Vec<PathBuf>,
        folder: PathBuf,
        tile: usize,
    },
    /// Each file to its new path.
    Rename {
        moves: Vec<(PathBuf, PathBuf)>,
        tile: usize,
    },
    SaveText {
        text: String,
        to: PathBuf,
        tile: usize,
    },
    /// Each file uploaded, or the text as a file.
    Upload {
        what: Payload,
        to: UploadTo,
        tile: usize,
    },
    /// Sent to a phone over KDE Connect.
    Phone {
        what: Payload,
        device: String,
        device_name: String,
        tile: usize,
    },
}

/// What's let go on a sharing tile.
#[derive(Clone, Debug)]
pub enum Payload {
    Files(Vec<PathBuf>),
    Text(String),
}

impl Job {
    fn tile(&self) -> usize {
        match self {
            Self::Copy { tile, .. }
            | Self::Move { tile, .. }
            | Self::Rename { tile, .. }
            | Self::SaveText { tile, .. }
            | Self::Upload { tile, .. }
            | Self::Phone { tile, .. } => *tile,
        }
    }

    /// "Copying", "Copied", and what it says it couldn't do.
    fn words(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Copy { .. } => ("Copying", "Copied", "Couldn't copy"),
            Self::Move { .. } => ("Moving", "Moved", "Couldn't move"),
            Self::Rename { .. } => ("Moving", "Moved", "Couldn't move"),
            Self::SaveText { .. } => ("Saving", "Saved", "Couldn't save"),
            Self::Upload { .. } => ("Uploading", "Uploaded", "Couldn't upload"),
            Self::Phone { .. } => ("Sending", "Sent", "Couldn't send"),
        }
    }

    /// What it works on, "photo.jpg" or "3 items", and where it goes.
    fn what(&self) -> (String, String) {
        let items = |files: &[PathBuf]| match files {
            [one] => name(one),
            many => format!("{} items", many.len()),
        };
        match self {
            Self::Copy { files, folder, .. } | Self::Move { files, folder, .. } => {
                (items(files), format!("to {}", name(folder)))
            }
            Self::Rename { moves, .. } => {
                let files: Vec<PathBuf> = moves.iter().map(|(from, _)| from.clone()).collect();
                let folder = moves
                    .first()
                    .and_then(|(_, to)| to.parent())
                    .map(name)
                    .unwrap_or_default();
                (items(&files), format!("to {folder}"))
            }
            Self::SaveText { to, .. } => (name(to), String::from("from text")),
            Self::Upload { what, to, .. } => (payload(what), format!("to {}", to.title())),
            Self::Phone {
                what, device_name, ..
            } => (payload(what), format!("to {device_name}")),
        }
    }

    fn glyph(&self) -> &'static str {
        match self {
            Self::Copy { .. } => "copy",
            Self::Move { .. } | Self::Rename { .. } => "folder",
            Self::SaveText { .. } => "check",
            Self::Upload { .. } => "cloud-upload",
            Self::Phone { .. } => "phone",
        }
    }
}

/// "photo.jpg", "3 items" or "text".
fn payload(what: &Payload) -> String {
    match what {
        Payload::Files(files) => match files.as_slice() {
            [one] => name(one),
            many => format!("{} items", many.len()),
        },
        Payload::Text(_) => String::from("text"),
    }
}

pub fn name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Starts `job`, first asking what to do with names already taken.
pub fn start(job: Job) {
    let taken = match &job {
        Job::Copy { files, folder, .. } | Job::Move { files, folder, .. } => {
            files::conflicts(files, folder)
        }
        _ => Vec::new(),
    };
    if taken.is_empty() {
        run(job, false);
    } else {
        state::ask(Prompt::Conflict { job, names: taken });
    }
}

/// Runs `job` on a thread; `keep_both` gives a taken name a number, else
/// what's there is replaced.
pub fn run(job: Job, keep_both: bool) {
    let tile = job.tile();
    let (doing, done_word, failed_word) = job.words();
    let (what, whither) = job.what();
    let cancel = Arc::new(AtomicBool::new(false));
    // the link the last state shows, for its Open button
    let opened: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let mut content = ActivityContent::new("Drop", ItemIcon::Glyph(String::from(job.glyph())), "");
    content.title = format!("{doing} {what}");
    content.subtitle = whither;
    content.trailing = Trailing::Progress(0.0);
    content.progress = Some(0.0);
    content.relevance = 0.8;
    content.buttons = vec![ActivityButton {
        destructive: true,
        ..ActivityButton::new("cancel", "Cancel")
    }];
    let activity = LiveActivity::start(content.clone());
    let stop = cancel.clone();
    let link = opened.clone();
    activity.on_event(move |event| match event {
        ActivityEvent::Action(action) if action == "cancel" => {
            stop.store(true, Ordering::Relaxed);
        }
        ActivityEvent::Action(action) if action == "open" => {
            if let Some(link) = link.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                shared::open(&link);
            }
        }
        _ => {}
    });
    state::change(|ui| {
        ui.running.insert(tile, Some(0.0));
        ui.outcomes.remove(&tile);
    });

    std::thread::spawn(move || {
        let mut told = Instant::now();
        let mut tell = |done: f32| {
            if told.elapsed() < TELL_EVERY {
                return;
            }
            told = Instant::now();
            state::change(|ui| {
                ui.running.insert(tile, Some(done));
            });
            content.trailing = Trailing::Progress(done);
            content.progress = Some(done);
            activity.update(content.clone(), false);
        };
        let result = work(&job, keep_both, &cancel, &mut tell);

        let mut last = ActivityContent::new("Drop", ItemIcon::Glyph(String::from("check")), "");
        let mut lasts = 3;
        let outcome = match &result {
            Ok(links) => {
                last.title = format!("{done_word} {what}");
                last.trailing = Trailing::Text(String::from("Done"));
                last.tint = Some(Tint::Green);
                if let Some(first) = links.first() {
                    let copied = shared::delivered(&what, links);
                    last.subtitle = first.clone();
                    last.trailing =
                        Trailing::Text(String::from(if copied { "Link copied" } else { "Done" }));
                    last.buttons = vec![ActivityButton::new("open", "Open")];
                    *opened.lock().unwrap_or_else(|e| e.into_inner()) = Some(first.clone());
                    lasts = 8;
                }
                Outcome::Done
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                last.title = format!("Stopped {}", doing.to_lowercase());
                last.leading = ItemIcon::Glyph(String::from("close"));
                last.trailing = Trailing::Text(String::from("Stopped"));
                Outcome::Failed
            }
            Err(error) => {
                last.title = format!("{failed_word} {what}");
                last.subtitle = error.to_string();
                last.leading = ItemIcon::Glyph(String::from("error"));
                last.trailing = Trailing::Text(String::from("Failed"));
                last.tint = Some(Tint::Red);
                crate::attention(true);
                Outcome::Failed
            }
        };
        last.relevance = 0.8;
        activity.update(last, false);
        activity.end(Dismiss::After(if outcome == Outcome::Done {
            lasts
        } else {
            6
        }));
        state::change(|ui| {
            ui.running.remove(&tile);
            ui.outcomes
                .insert(tile, (outcome, Instant::now() + state::OUTCOME_FOR));
        });
        std::thread::sleep(state::OUTCOME_FOR);
        state::change(|ui| {
            if ui
                .outcomes
                .get(&tile)
                .is_some_and(|(_, until)| *until <= Instant::now())
            {
                ui.outcomes.remove(&tile);
            }
        });
    });
}

/// Does `job`, telling how far along it is (0 to 1); the links it gave.
fn work(
    job: &Job,
    keep_both: bool,
    cancel: &AtomicBool,
    tell: &mut dyn FnMut(f32),
) -> std::io::Result<Vec<String>> {
    match job {
        Job::Copy { files, folder, .. } | Job::Move { files, folder, .. } => {
            let moving = matches!(job, Job::Move { .. });
            let total: u64 = files
                .iter()
                .map(|file| files::size_of(file))
                .sum::<u64>()
                .max(1);
            let mut done = 0u64;
            for file in files {
                let to = files::target(file, folder, keep_both)?;
                let mut count = |bytes: u64| {
                    done += bytes;
                    tell(done as f32 / total as f32);
                };
                if moving {
                    files::move_to(file, &to, &mut count, cancel)?;
                } else {
                    files::copy(file, &to, &mut count, cancel)?;
                }
            }
            Ok(Vec::new())
        }
        Job::Rename { moves, .. } => {
            let mut nothing = |_| {};
            for (index, (from, to)) in moves.iter().enumerate() {
                let folder = to.parent().unwrap_or(Path::new("/"));
                let to = files::free_name(folder, &name(to));
                files::move_to(from, &to, &mut nothing, cancel)?;
                tell((index + 1) as f32 / moves.len() as f32);
            }
            Ok(Vec::new())
        }
        Job::SaveText { text, to, .. } => {
            let folder = to.parent().unwrap_or(Path::new("/"));
            std::fs::write(files::free_name(folder, &name(to)), text)?;
            Ok(Vec::new())
        }
        Job::Upload {
            what: Payload::Text(text),
            to,
            ..
        } => Ok(share::upload_text(text, *to, cancel, tell)?
            .into_iter()
            .collect()),
        Job::Upload {
            what: Payload::Files(files),
            to,
            ..
        } => {
            let mut links = Vec::new();
            let count = files.len().max(1) as f32;
            for (index, file) in files.iter().enumerate() {
                let mut part = |done: f32| tell((index as f32 + done) / count);
                links.extend(share::upload(file, *to, cancel, &mut part)?);
            }
            Ok(links)
        }
        Job::Phone { what, device, .. } => {
            match what {
                Payload::Files(files) => KdeConnect::share_files_now(device, files),
                Payload::Text(text) => KdeConnect::share_text_now(device, text),
            }
            .map_err(std::io::Error::other)?;
            tell(1.0);
            Ok(Vec::new())
        }
    }
}

/// The new paths Rename & Move gives `files` in `folder` from the name
/// typed: one file takes it whole (its extension kept when the name has
/// none), several are numbered "name 1.jpg", "name 2.jpg".
pub fn renamed(files: &[PathBuf], folder: &Path, typed: &str) -> Vec<(PathBuf, PathBuf)> {
    let typed = typed.trim();
    let extension = |file: &Path| {
        file.extension()
            .map(|extension| format!(".{}", extension.to_string_lossy()))
            .unwrap_or_default()
    };
    match files {
        [one] => {
            let name = if typed.contains('.') || one.is_dir() {
                typed.to_string()
            } else {
                format!("{typed}{}", extension(one))
            };
            vec![(one.clone(), folder.join(name))]
        }
        many => many
            .iter()
            .enumerate()
            .map(|(index, file)| {
                let name = format!("{typed} {}{}", index + 1, extension(file));
                (file.clone(), folder.join(name))
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renaming_keeps_the_extension_and_numbers_many() {
        let folder = Path::new("/out");
        let one = renamed(&[PathBuf::from("/in/IMG_1.jpg")], folder, " Beach ");
        assert_eq!(
            one,
            [(
                PathBuf::from("/in/IMG_1.jpg"),
                PathBuf::from("/out/Beach.jpg")
            )]
        );
        let own = renamed(&[PathBuf::from("/in/a.jpg")], folder, "b.png");
        assert_eq!(own[0].1, PathBuf::from("/out/b.png"));
        let many = renamed(
            &[PathBuf::from("/in/a.jpg"), PathBuf::from("/in/b.mov")],
            folder,
            "Trip",
        );
        assert_eq!(many[0].1, PathBuf::from("/out/Trip 1.jpg"));
        assert_eq!(many[1].1, PathBuf::from("/out/Trip 2.mov"));
    }

    #[test]
    fn a_job_says_what_it_does() {
        let job = Job::Copy {
            files: vec![PathBuf::from("/a/x.txt"), PathBuf::from("/a/y.txt")],
            folder: PathBuf::from("/home/me/Downloads"),
            tile: 0,
        };
        assert_eq!(
            job.what(),
            (String::from("2 items"), String::from("to Downloads"))
        );
        assert_eq!(job.words().0, "Copying");
    }
}
