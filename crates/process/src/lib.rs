//! Running a program and reading what it said.
//!
//! One runner for every amane-shell program, so each doesn't carry its own:
//!
//! ```no_run
//! use amane_process::Cmd;
//!
//! // to its end: what it printed, or the last lines of what it said
//! let branch = Cmd::new("git").args(["branch", "--show-current"]).dir("/etc/nixos").stdout();
//!
//! // while it runs, a line at a time, and stopped from another thread
//! let mut build = Cmd::new("nix").args(["build", "--no-link"]).spawn()?;
//! let stop = build.stopper();
//! build.each_stderr_line(|line| println!("{line}"));
//! let done = build.finish();
//! # let _ = (branch, stop, done);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! Every command gets no stdin and `NO_COLOR=1`, so nothing waits on a
//! prompt and nothing paints its output. Blocking, so never from a view.

#![warn(missing_docs)]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

/// How many of a failed command's last lines its [`Failure`] keeps: the
/// error is what most programs say last.
pub const TAIL_LINES: usize = 12;

/// A program to run, with its arguments, folder and environment.
#[derive(Clone, Debug)]
pub struct Cmd {
    program: OsString,
    args: Vec<OsString>,
    dir: Option<PathBuf>,
    env: Vec<(OsString, OsString)>,
}

/// What a command that succeeded printed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// Its standard output.
    pub stdout: String,
    /// Its standard error: some programs report there even on success.
    pub stderr: String,
}

/// Why a command failed: it couldn't start, it exited with a code other
/// than 0, or a signal stopped it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Its exit code; `None` when it couldn't start or a signal stopped it.
    pub code: Option<i32>,
    /// The last lines it said (see [`tail`]), or why it couldn't start.
    pub message: String,
    /// All it printed, for callers that read past the message.
    pub output: Output,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

impl Failure {
    fn from(status: ExitStatus, output: Output) -> Self {
        Self {
            code: status.code(),
            message: tail(&output.stderr, &output.stdout, status.code()),
            output,
        }
    }

    fn start(program: &OsStr, error: &io::Error) -> Self {
        Self {
            code: None,
            message: format!("{}: {error}", program.to_string_lossy()),
            output: Output::default(),
        }
    }
}

impl Cmd {
    /// `program`, found on `PATH` unless it's a path.
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            dir: None,
            env: Vec::new(),
        }
    }

    /// One more argument.
    #[must_use]
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// More arguments.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    /// Runs it in `dir`.
    #[must_use]
    pub fn dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Sets `key` to `value` in its environment.
    #[must_use]
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    /// The program's name, as given.
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .stdin(Stdio::null())
            .env("NO_COLOR", "1");
        for (key, value) in &self.env {
            command.env(key, value);
        }
        if let Some(dir) = &self.dir {
            command.current_dir(dir);
        }
        command
    }

    /// Runs it to its end: what it printed when it succeeded.
    pub fn output(&self) -> Result<Output, Failure> {
        let out = self
            .command()
            .output()
            .map_err(|error| Failure::start(&self.program, &error))?;
        let output = Output {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        };
        if out.status.success() {
            Ok(output)
        } else {
            Err(Failure::from(out.status, output))
        }
    }

    /// Runs it to its end: its standard output when it succeeded, else
    /// the [`Failure`]'s message.
    pub fn stdout(&self) -> Result<String, String> {
        self.output()
            .map(|output| output.stdout)
            .map_err(|failure| failure.message)
    }

    /// Runs it to its end and says only whether it succeeded, printing
    /// nowhere: for an opener like `xdg-open`.
    pub fn status(&self) -> Result<(), Failure> {
        let status = self
            .command()
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| Failure::start(&self.program, &error))?;
        if status.success() {
            Ok(())
        } else {
            Err(Failure::from(status, Output::default()))
        }
    }

    /// Starts it, with its output piped back, for reading while it runs.
    pub fn spawn(&self) -> io::Result<Running> {
        let mut child = self
            .command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Ok(Running {
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            child: Arc::new(Mutex::new(child)),
        })
    }

    /// Starts it and lets it run on its own, its output going nowhere: for
    /// opening something with no answer to wait for. Its end is collected
    /// on a thread, so it leaves no zombie.
    pub fn detach(&self) -> io::Result<()> {
        let mut child = self
            .command()
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        thread::spawn(move || {
            // its exit says nothing anyone waits for
            let _ = child.wait();
        });
        Ok(())
    }
}

/// A command that is running, read as it goes.
pub struct Running {
    child: Arc<Mutex<Child>>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
}

/// Stops a [`Running`] command from another thread.
#[derive(Clone)]
pub struct Stopper(Arc<Mutex<Child>>);

impl Stopper {
    /// Kills it; nothing when it has already ended.
    pub fn stop(&self) {
        let mut child = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        // it may have ended on its own a moment ago
        let _ = child.kill();
    }
}

impl Running {
    /// What stops it from another thread.
    pub fn stopper(&self) -> Stopper {
        Stopper(Arc::clone(&self.child))
    }

    /// Its standard output, to read as it comes. [`Running::finish`] then
    /// has none left to read.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.stdout.take()
    }

    /// Calls `each` with every line it writes to standard error, until it
    /// closes it (usually at its end). Its standard output waits in the
    /// pipe, so this suits programs that print little there, like `nix
    /// build`.
    pub fn each_stderr_line(&mut self, mut each: impl FnMut(&str)) -> String {
        let mut all = String::new();
        if let Some(stderr) = self.stderr.take() {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                each(&line);
                all.push_str(&line);
                all.push('\n');
            }
        }
        all
    }

    /// Waits for its end: what it printed, read from both pipes at once
    /// so neither fills while the other is read. Standard error already
    /// read by [`Running::each_stderr_line`] is `stderr_seen`.
    pub fn finish_with(mut self, stderr_seen: String) -> Result<Output, Failure> {
        let errors = self.stderr.take().map(|mut stderr| {
            thread::spawn(move || {
                let mut text = Vec::new();
                // a pipe closed early leaves what was read
                let _ = stderr.read_to_end(&mut text);
                text
            })
        });
        let mut stdout = Vec::new();
        if let Some(mut out) = self.stdout.take() {
            // as above
            let _ = out.read_to_end(&mut stdout);
        }
        let mut stderr = stderr_seen;
        if let Some(errors) = errors {
            let rest = errors.join().unwrap_or_default();
            stderr.push_str(&String::from_utf8_lossy(&rest));
        }
        let output = Output {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr,
        };
        let status = self
            .child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .wait();
        match status {
            Ok(status) if status.success() => Ok(output),
            Ok(status) => Err(Failure::from(status, output)),
            Err(error) => Err(Failure {
                code: None,
                message: error.to_string(),
                output,
            }),
        }
    }

    /// Waits for its end: what it printed.
    pub fn finish(self) -> Result<Output, Failure> {
        self.finish_with(String::new())
    }
}

/// The last [`TAIL_LINES`] lines a failed command said, standard error
/// first, or its exit when it said nothing. nix's dirty-tree warning is
/// left out: it comes first and says nothing about the failure.
pub fn tail(stderr: &str, stdout: &str, code: Option<i32>) -> String {
    let said: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("warning: Git tree"))
        .collect();
    if said.is_empty() {
        return match code {
            Some(code) => format!("exited with {code}"),
            None => String::from("stopped by a signal"),
        };
    }
    said[said.len().saturating_sub(TAIL_LINES)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Cmd {
        Cmd::new("sh").args(["-c", script])
    }

    #[test]
    fn output_of_a_command_that_succeeds() {
        let out = sh("echo out; echo err >&2").output().unwrap();
        assert_eq!(out.stdout, "out\n");
        assert_eq!(out.stderr, "err\n");
    }

    #[test]
    fn a_failure_keeps_its_last_lines_and_code() {
        let failed = sh("for i in $(seq 1 20); do echo line$i >&2; done; exit 3")
            .output()
            .unwrap_err();
        assert_eq!(failed.code, Some(3));
        assert!(failed.message.starts_with("line9\n"));
        assert!(failed.message.ends_with("line20"));
    }

    #[test]
    fn a_silent_failure_says_how_it_exited() {
        assert_eq!(sh("exit 4").stdout().unwrap_err(), "exited with 4");
    }

    #[test]
    fn a_missing_program_says_so() {
        let failed = Cmd::new("amane-no-such-program").output().unwrap_err();
        assert_eq!(failed.code, None);
        assert!(failed.message.starts_with("amane-no-such-program: "));
    }

    #[test]
    fn dir_and_env_and_no_color() {
        let out = sh("pwd; echo $GREETING $NO_COLOR")
            .dir("/")
            .env("GREETING", "hi")
            .stdout()
            .unwrap();
        assert_eq!(out, "/\nhi 1\n");
    }

    #[test]
    fn stderr_lines_as_they_come_then_the_output() {
        let mut running = sh("echo a >&2; echo b >&2; echo done").spawn().unwrap();
        let mut lines = Vec::new();
        let seen = running.each_stderr_line(|line| lines.push(line.to_string()));
        assert_eq!(lines, ["a", "b"]);
        let out = running.finish_with(seen).unwrap();
        assert_eq!(out.stdout, "done\n");
        assert_eq!(out.stderr, "a\nb\n");
    }

    #[test]
    fn a_stopped_command_fails_by_signal() {
        let running = sh("sleep 30").spawn().unwrap();
        running.stopper().stop();
        let failed = running.finish().unwrap_err();
        assert_eq!(failed.code, None);
        assert_eq!(failed.message, "stopped by a signal");
    }

    #[test]
    fn both_pipes_are_read_at_once() {
        // more than a pipe holds, on each side
        let out = sh("head -c 200000 /dev/zero; head -c 200000 /dev/zero >&2")
            .spawn()
            .unwrap()
            .finish()
            .unwrap();
        assert_eq!(out.stdout.len(), 200_000);
        assert_eq!(out.stderr.len(), 200_000);
    }
}
