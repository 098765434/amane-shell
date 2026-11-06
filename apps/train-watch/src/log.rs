//! A training run, as its log tells it: the lines `cactus finetune` and
//! `cactus build` print (each stamped `HH:MM:SS` by the job), read one at a
//! time into where the run is, how fast it goes and how it ended.

/// Where a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Loading, or JAX compiling the step: nothing measured yet.
    Starting,
    Training,
    /// The epochs are done; the trainer scores its held-out examples.
    Scoring,
    /// The adapter is written; `cactus build` makes the archive.
    Building,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub phase: Phase,
    pub examples: Option<u32>,
    /// The step, of how many.
    pub step: Option<(u32, u32)>,
    pub loss: Option<f32>,
    pub epoch: Option<(u32, u32)>,
    /// The validation loss after the last epoch.
    pub val: Option<f32>,
    /// "98/199": held-out calls the trainer got exactly right.
    pub accuracy: Option<String>,
    /// The dev eval, once the archive was fetched and scored.
    pub dev: Option<String>,
    /// The job's exit status.
    pub status: Option<i32>,
    /// The last line that wasn't a step: what a failure said.
    pub last: String,
    // the first step seen and the latest, as (step, seconds into the day)
    first: Option<(u32, u32)>,
    latest: Option<(u32, u32)>,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            phase: Phase::Starting,
            examples: None,
            step: None,
            loss: None,
            epoch: None,
            val: None,
            accuracy: None,
            dev: None,
            status: None,
            last: String::new(),
            first: None,
            latest: None,
        }
    }
}

impl Run {
    /// Reads one line of the log; whether anything changed.
    pub fn feed(&mut self, line: &str) -> bool {
        let before = self.clone();
        let (time, rest) = stamped(line);
        let mut words = rest.split_whitespace();
        let key = words.next().unwrap_or("");
        let words: Vec<&str> = words.collect();
        match key {
            "data" => self.examples = words.first().and_then(|n| n.parse().ok()),
            "step" => {
                if let Some((step, of)) = words.first().and_then(|w| fraction(w)) {
                    self.phase = Phase::Training;
                    self.step = Some((step, of));
                    if let Some(time) = time {
                        // the first step after a restart of the clock starts the rate over
                        if self.first.is_none_or(|(first, _)| step <= first) {
                            self.first = Some((step, time));
                        }
                        self.latest = Some((step, time));
                    }
                }
                self.loss = after(&words, "loss").or(self.loss);
                return *self != before;
            }
            "epoch" => {
                self.epoch = words.first().and_then(|w| fraction(w));
                self.loss = after(&words, "loss").or(self.loss);
                self.val = after(&words, "val").or(self.val);
                if self.epoch.is_some_and(|(epoch, of)| epoch == of) {
                    self.phase = Phase::Scoring;
                }
            }
            "accuracy" => self.accuracy = words.first().map(|w| w.to_string()),
            "adapter" => self.phase = Phase::Building,
            "dev" => self.dev = Some(words.join(" ")),
            "finished" => {
                let status = words.first().and_then(|w| w.parse().ok()).unwrap_or(1);
                self.status = Some(status);
                self.phase = if status == 0 {
                    Phase::Done
                } else {
                    Phase::Failed
                };
            }
            _ => {}
        }
        if !rest.trim().is_empty() && key != "finished" {
            self.last = rest.trim().to_string();
        }
        *self != before
    }

    /// How far along, 0 … 1, while it trains.
    pub fn progress(&self) -> Option<f32> {
        let (step, of) = self.step?;
        (of > 0).then(|| (step as f32 / of as f32).clamp(0.0, 1.0))
    }

    /// Seconds a step takes, from the steps seen so far.
    pub fn per_step(&self) -> Option<f32> {
        let (first, start) = self.first?;
        let (latest, now) = self.latest?;
        if latest <= first {
            return None;
        }
        let elapsed = (now + 86_400 - start) % 86_400;
        Some(elapsed as f32 / (latest - first) as f32)
    }

    /// Seconds of training left.
    pub fn left(&self) -> Option<u32> {
        let (step, of) = self.step?;
        Some((self.per_step()? * of.saturating_sub(step) as f32).round() as u32)
    }

    pub fn finished(&self) -> bool {
        matches!(self.phase, Phase::Done | Phase::Failed)
    }

    /// One line saying where it is, for the bar's subtitle and the terminal.
    pub fn summary(&self) -> String {
        match self.phase {
            Phase::Starting => match self.examples {
                Some(examples) => format!("{examples} examples · compiling"),
                None => String::from("starting"),
            },
            Phase::Training => {
                let mut parts = Vec::new();
                if let Some((step, of)) = self.step {
                    parts.push(format!("step {step}/{of}"));
                }
                if let Some(loss) = self.loss {
                    parts.push(format!("loss {loss:.3}"));
                }
                if let Some(val) = self.val {
                    parts.push(format!("val {val:.3}"));
                }
                if let Some(left) = self.left() {
                    parts.push(format!("{} left", duration(left)));
                }
                parts.join(" · ")
            }
            Phase::Scoring => format!(
                "scoring held-out calls · val {:.3}",
                self.val.unwrap_or(f32::NAN)
            ),
            Phase::Building => String::from("building the archive"),
            Phase::Done => {
                let mut parts = Vec::new();
                if let Some(accuracy) = &self.accuracy {
                    parts.push(format!("held-out {accuracy}"));
                }
                if let Some(val) = self.val {
                    parts.push(format!("val {val:.3}"));
                }
                if let Some(dev) = &self.dev {
                    parts.push(format!("dev {dev}"));
                }
                parts.join(" · ")
            }
            Phase::Failed => self.last.clone(),
        }
    }
}

/// "4 min", "1 h 05 min", "40 s".
pub fn duration(seconds: u32) -> String {
    match seconds {
        s if s < 60 => format!("{s} s"),
        s if s < 3600 => format!("{} min", (s + 30) / 60),
        s => format!("{} h {:02} min", s / 3600, (s % 3600) / 60),
    }
}

// "20:58:11   step ..." as (seconds into the day, "step ...")
fn stamped(line: &str) -> (Option<u32>, &str) {
    let Some((stamp, rest)) = line.split_once(' ') else {
        return (None, line);
    };
    let parts: Vec<u32> = stamp.split(':').filter_map(|p| p.parse().ok()).collect();
    match parts[..] {
        [h, m, s] if stamp.len() == 8 => (Some(h * 3600 + m * 60 + s), rest),
        _ => (None, line),
    }
}

fn fraction(word: &str) -> Option<(u32, u32)> {
    let (a, b) = word.split_once('/')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn after(words: &[&str], key: &str) -> Option<f32> {
    let at = words.iter().position(|w| *w == key)?;
    words.get(at + 1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str]) -> Run {
        let mut run = Run::default();
        for line in lines {
            run.feed(line);
        }
        run
    }

    #[test]
    fn a_run_is_followed_to_its_end() {
        let mut r = run(&[
            "20:59:30   data      1992 examples  seq_len 512  cap 512",
            "20:59:50   step      6/339  loss 0.8613",
            "20:59:53   step      12/339  loss 0.7000",
        ]);
        assert_eq!(r.phase, Phase::Training);
        assert_eq!(r.step, Some((12, 339)));
        assert_eq!(r.per_step(), Some(0.5));
        assert_eq!(r.left(), Some(164));
        assert_eq!(r.summary(), "step 12/339 · loss 0.700 · 3 min left");

        r.feed("21:02:01   epoch     3/3  loss 0.6715  val 0.1977");
        assert_eq!(r.phase, Phase::Scoring);
        r.feed("21:03:00   accuracy  98/199 held-out calls exact, scored on the W4 weights");
        r.feed("21:03:00   adapter   adapter.safetensors");
        assert_eq!(r.phase, Phase::Building);
        r.feed("21:03:20   finished  0");
        assert!(r.finished());
        assert_eq!(r.summary(), "held-out 98/199 · val 0.198");
        r.feed("21:04:00   dev       41/60 right, 0 wrong");
        assert_eq!(
            r.summary(),
            "held-out 98/199 · val 0.198 · dev 41/60 right, 0 wrong"
        );
    }

    #[test]
    fn a_failure_says_what_went_wrong() {
        let r = run(&[
            "20:58:11 cactus finetune: error: argument --epochs: expected one argument",
            "20:58:11   finished  2",
        ]);
        assert_eq!(r.phase, Phase::Failed);
        assert_eq!(
            r.summary(),
            "cactus finetune: error: argument --epochs: expected one argument"
        );
    }

    #[test]
    fn the_rate_spans_midnight() {
        let r = run(&[
            "23:59:50   step 10/100  loss 1",
            "00:00:10   step 20/100  loss 1",
        ]);
        assert_eq!(r.per_step(), Some(2.0));
    }

    #[test]
    fn durations_read_plainly() {
        assert_eq!(duration(40), "40 s");
        assert_eq!(duration(250), "4 min");
        assert_eq!(duration(3900), "1 h 05 min");
    }
}
