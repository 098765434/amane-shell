# Idle report

- Date: 2026-11-07T01:39+01:00
- Shell: `/nix/store/4dpwcs2f9cxf7jj4m3mls4p2nm3bagdz-amane-shell-0.1.0`, pid 1778
- Window: 60s, the desktop left alone, then 60s more to count programs under strace

| Measure | Value |
|---|---|
| Wakeups (context switches, all threads) | 116.3 per second |
| CPU time, the shell | 510 ms |
| CPU time, its exited children | 580 ms |
| PSS at the end | 72 MiB |
| Programs started | 74 (1.2 per second; exact, from strace) |

| Program | Times |
|---|---|
| nmcli | 40 |
| niri | 30 |
| rbw | 4 |

This is the baseline for Phase 7 of [the roadmap](../archive/ROADMAP.md) (no programs started while idle, under 2 wakeups a second). Measure again with `sudo scripts/idle-report 60` after any change to idle behaviour. The programs: `nmcli` from the connections service (about two calls every 3 s), `niri msg` from the shell's own niri calls, `rbw` from the vault's lock check (every 15 s).
