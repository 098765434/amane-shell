# Assistant

Spotlight reads a query as a system command and offers it as the card over the
results: "turn the volume down" shows **Turn Volume Down**, and Return runs it. A key
held, or pressed twice, does the same by voice. Everything runs on this machine, on the
CPU, through [Cactus](https://cactuscompute.com/): **Needle 3** for commands (35 MB) and
**Whistle** for speech (17 MB).

It covers what the shell already does, and nothing else:
- Control Center's switches: Wi‑Fi, Bluetooth, Focus, Dark Style, Night Light, Airplane
  Mode, Keep Awake, VPN, Tailscale, Screenshot, and so on.
- The volume and screen brightness, set or stepped, with the OSD.
- Muting.
- Opening an app.
- Play, pause, next and previous.
- The session: lock, log out, sleep, restart, power off.
- A timer.
- The power mode.

Questions and anything else go to Spotlight's usual rows and the web search.

## How a query is read

1. **Rules** ([`rules.rs`](../../apps/assistant/src/rules.rs)) read the common ways of
   saying the common things, at once and with no model: "louder", "volume 40", "next
   song", "turn on night light", "open firefox", "timer 5 minutes".
   - A rule matches the whole query or nothing.
   - A negated query ("don't let it sleep") is left to Needle.
   - A rule's reading is certain.
2. **Needle 3**, 180 ms after the last key, for longer queries the rules don't read.
   - It's offered five tools (`set_switch`, `set_level`, `open_app`, `media`, `session`,
     [`tools.rs`](../../apps/assistant/src/tools.rs)).
   - Their arguments are `enum`s of what this machine has: its switches by name, its apps
     with the most used first.
   - A call is read back only when every argument names something real. Calls Needle
     holds back as unsure are dropped, and so are calls under 0.3 confidence.
   - Below 0.75 the card says "Assistant · Suggested".
3. **Spotlight's rows**, as always. The assistant stands aside for:
   - a sum or a conversion (they keep the card);
   - a single word (that's a name);
   - a row that already matches outright, when the reading is Needle's.

What's here is a [`Snapshot`](../../apps/assistant/src/snapshot.rs): the switches and
whether they're on, the apps with their kinds ("terminal", "browser", from their desktop
categories and keywords), the power modes, and what logind allows. The launcher builds
it ([`launcher/assistant.rs`](../../src/surfaces/launcher/assistant.rs)).

## Running a command

A command runs through the shell's own path:
- the Quick Settings toggle (`quick_toggle(id).fire()`);
- the OSD's volume and brightness keys (`osd::set_volume`, `osd::ipc`), so the OSD shows;
- `DesktopApp::launch`;
- MPRIS (`Media::play_pause`, `next`, `previous`);
- `Session` for the session;
- `timer::start`;
- `PowerProfile::set`.

Each command carries a [`Risk`](../../apps/assistant/src/command.rs):

| Risk | Commands | Typed | Spoken |
|---|---|---|---|
| Safe | switches, levels, mute, media, timer, power mode | Return runs it | runs at once if a rule read it |
| Notable | open an app, lock | Return runs it | runs at once if a rule read it |
| Destructive | log out, sleep, restart, power off | Return asks first ("Restart?", Cancel) | waits on the card; Return asks first |

Something already done shows as a card that does nothing: "Night Light Is Already On".
The card's hint is "Return to run", or "Return to confirm" for a destructive command.

## Voice

`assistant listen start` opens Spotlight on a fresh search and starts listening:
- The magnifier becomes the microphone, on an accent disc that swells with the level.
- What's heard so far shows as the field's placeholder.

`assistant listen stop` ends it:
- What was heard is typed into the field, so Spotlight searches it.
- A rule's reading of a non-destructive command runs at once.
- Anything less certain waits on its card for Return.

Escape, or closing Spotlight, cancels.

- **Silence:** Whistle makes words up from it ("Bye-bye."). The daemon passes nothing to
  Whistle until a piece is louder than the room (`VOICE`), and keeps 0.3 s before that
  as lead-in. Silence the whole way through is heard as nothing.
- **Keywords:** Whistle is told the names of what's here (switch titles, app names) as
  keywords. With them, espeak's "turn on night light" is heard right. Without them it
  came out as "Turn on, I'd like".
- **Privacy indicator:** recording is a PulseAudio stream named "Assistant", in the
  daemon, so the bar's microphone indicator shows it. Nothing is written to disk.

niri can't run a bind on a key's release, so a key **toggles**:

```kdl
binds {
    Mod+Shift+Space hotkey-overlay-title="Assistant: listen" {
        spawn "sh" "-c" "printf 'assistant\\nlisten\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock";
    }
}
```

## The daemon

`amane-assistant` ([`apps/assistant`](../../apps/assistant)) is a user service beside
the shell:
- It holds Cactus, listens to the microphone while asked to, and answers over
  `$XDG_RUNTIME_DIR/amane-assistant.sock` (mode 0600).
- It runs no command itself.
- It runs as its own process because Cactus keeps one model set and one conversation for
  the whole process, and its calls block. Restarting the shell doesn't reload the
  models, and a crash in Cactus can't take the shell down.
- The shell works without it: rules still read what they can, and nothing waits.

The protocol ([`protocol.rs`](../../apps/assistant/src/protocol.rs)) is JSON lines. The
shell sends requests with an id, and the daemon answers with the id. While listening it
also sends `level` and `partial`.

```text
→ {"id":1,"op":"hello"}
← {"kind":"hello","id":1,"engine":"cactus 4.0.0","models":3}
→ {"id":2,"op":"complete","system":"date: …","tools":"[…]","text":"turn the screen warmer"}
← {"kind":"answer","id":2,"answer":{"function_calls":[…],"confidence":0.8,…},"ms":64}
→ {"id":3,"op":"listen_start","keywords":["Night Light","Firefox"]}
← {"kind":"level","rms":0.31}   {"kind":"partial","text":"turn on night"}
→ {"id":4,"op":"listen_stop"}
← {"kind":"heard","id":4,"text":"Turn on night light.","ms":28}
```

Of several `complete`s queued while Needle was busy, only the newest is answered. The
others get `superseded`.

- **Models:** from `$AMANE_ASSISTANT_MODELS` (the package sets it to the store copy),
  else `$XDG_DATA_HOME/amane-assistant/models`. A fine-tuned `needle3.cact` can go there.
- **Tool index:** `$XDG_CACHE_HOME/amane-assistant/tools-<hash>.idx`. Cactus caches a
  tool set's embeddings there, so setting up a tool set costs 240 ms once and nothing
  after.
- **Test hook:** `AMANE_ASSISTANT_FAKE_MIC=<raw 16 kHz f32le>` plays a recording as the
  microphone, for tests.

### Cactus

- **Version:** Cactus 4.0.0 (`cactus-compute`, Apache-2.0). Its prebuilt
  `linux-x86_64/libcactus.a` is linked statically with the LLVM libc++ it was built
  against (`build.rs`: `CACTUS_LIB_DIR`, `LIBCXX_DIR`). Without them the daemon builds
  anyway and answers that it has no engine.
- **Pinning:** the flake pins the library, header, licence and both models to Hugging
  Face commits and hashes (`cactusFor`). Nothing is downloaded at run time.
- **Hardware:** CPU only. Cactus has no Vulkan or ROCm backend on Linux. Its general
  chat engine (`cactus-engine` 2.x) is ARM and Apple only.
- **Network:** `libcactus.a` has no network code (no `socket` or `connect` among its
  symbols). The wrapper still sets `CACTUS_TELEMETRY=0` and `DO_NOT_TRACK=1`, and the
  unit can be held to `RestrictAddressFamilies=AF_UNIX`.

### Measured (Ryzen AI 7 PRO 450, 2026-11-06)

| | |
|---|---|
| Loading both models | 40–60 ms |
| A Needle answer, loaded | 48–78 ms (about 1,500 tok/s prefill, 700–1,000 tok/s decode) |
| Whistle, a 2 s clip | about 65 ms; the final text 20–30 ms after stop |
| Memory | about 80 MB for Needle alone, 140 MB with both |

## Evaluating

```sh
amane-assistant eval apps/assistant/eval/cases.jsonl apps/assistant/eval/laptop.json
```

This reads each case, by the rules and then Needle, against the command it should give.
Negatives should give nothing. `AMANE_ASSISTANT_DEBUG=1` prints Needle's raw answers.

On 2026-11-06 it scored:
- **40 of 60 commands right, all by the rules.**
- **None wrong.**
- **12 of 12 negatives left alone.**

The base Needle 3 offers none of the 20 it misses above 0.3 confidence. These are
paraphrases like "turn the screen warmer", "I can't hear anything" and "check my email".
On its own, with five tools, it got 14 of 28 (and ran "skip this song" as **log out**).
That's why its readings wait for Return, and destructive ones always ask.

Next: fine-tune Needle on phrasings generated from these tools (`cactus finetune`, then
`cactus build --lora`), and drop the result into the models folder.

## Files

| File | What's in it |
|---|---|
| `apps/assistant/src/command.rs` | commands and their risk (tested) |
| `apps/assistant/src/rules.rs` | the phrasings read without a model (tested) |
| `apps/assistant/src/snapshot.rs` | what's here, and finding things in it by name (tested) |
| `apps/assistant/src/tools.rs` | Needle's tools, and reading its answer (tested) |
| `apps/assistant/src/protocol.rs`, `client.rs` | the socket's messages, the shell's end |
| `apps/assistant/src/engine.rs` | Cactus, through its C API |
| `apps/assistant/src/server.rs`, `audio.rs` | the socket, the engine's thread, the microphone |
| `apps/assistant/src/eval.rs` | scoring |
| `src/services/assistant.rs` | the shell's client: rules, then Needle a moment later; listening |
| `src/surfaces/launcher/assistant.rs` | the snapshot, and a reading as its card and action |
