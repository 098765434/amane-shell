# emaki-process

The one way emaki's programs run another program and read what it said.
It uses std only, so the kit, Emaki Settings' NixOS jobs and the small tools
beside the shell all share it.

```rust
use emaki_process::Cmd;

// to its end: stdout on success, else the last lines it said
let status = Cmd::new("git").args(["status", "--short"]).dir(repo).stdout()?;

// while it runs: stderr a line at a time, stoppable from another thread
let mut build = Cmd::new("nix").args(["build", "--no-link"]).spawn()?;
let stop = build.stopper(); // stop.stop() from a Cancel button
let seen = build.each_stderr_line(|line| progress(line));
let output = build.finish_with(seen)?;
```

- Every command gets no stdin and `NO_COLOR=1`.
- A `Failure` has the exit code, the last 12 lines the program said (`tail`), and
  everything it printed.
- `finish` reads stdout and stderr at once, so a full pipe never stalls the
  program.
- `status` and `detach` are for openers like `xdg-open`, whose output nobody reads.

In the kit, `emaki_kit::platform::process` re-exports it next to `run`, `run_now`,
`installed` and `Latest`.

Programs that are fed on stdin (`wl-copy`, Drop's `curl`) and long-running
supervised children (the mirror's `adb`, the shell's `child_stream`) keep their own
`std::process` code.
