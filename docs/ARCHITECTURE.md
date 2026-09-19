# Zaapy — Architecture

How the product described in [VISION.md](VISION.md) is built, and why it is
split the way it is.

## The shape of the problem

The business logic is small — validate a clipboard string, decide whether to act,
type five keystrokes. Almost all of the difficulty is in the operating system:
watching the clipboard, knowing which window had focus, raising a window owned by
another process, and injecting keystrokes into it. Windows and macOS disagree on
every one of those.

So the codebase is split along exactly that line.

## Layout

```
crates/zaapy-core/       no OS, no UI, no Tauri — the whole behaviour
crates/zaapy-platform/   the OS, and nothing else
src-tauri/               the application: tray, window, IPC, loop, logging
src/                     the settings page (Svelte 5)
```

`zaapy-core` defines four traits — `ClipboardPort`, `WindowPort`, `InputPort`,
`Clock` — and `zaapy-platform` implements them. The bridge state machine makes no
system call of its own, so the entire flow, including every failure path, runs
against in-memory fakes (`zaapy_core::mock`) on a machine with neither Dofus nor
Ganymède installed.

`zaapy-platform` deliberately does **not** depend on `tauri-build`. That keeps
`cargo check -p zaapy-platform --target x86_64-pc-windows-msvc` working from a
Mac or a Linux runner, which is how the Windows code is reviewed before it ever
reaches a PC.

## The loop

The host calls `Bridge::tick()` every 100 ms. Each tick:

1. Samples the foreground window, keeping the previous sample.
2. Compares the clipboard's *sequence number* — a cheap counter — and stops there
   if it has not moved. The content is never read on an idle tick.
3. Parses the payload. Anything that is not a known command is dropped silently.
4. **The gate**: the command is only relayed if the source application was in
   front, now or one tick ago. Two samples cover the 100 ms window during which
   the copy may have happened.
5. Drops a command identical to the previous one inside 500 ms — clipboard
   notifications fire twice for one copy.
6. Resolves the target window by process name and title substring, refusing to
   guess when several match.
7. Raises the window, then **confirms** it actually came forward before typing.
8. Plays the send sequence.
9. Empties the clipboard, but only on success and only while it still holds what
   was just pasted.

Steps 4 and 7 are the two safety properties worth protecting in any refactor:
Zaapy never acts on a copy the user did not make in the guide, and never types
into a window it has not confirmed is in front.

## Why polling

Windows can push clipboard changes via `AddClipboardFormatListener`, but that
needs a message-only window and a message loop on a dedicated thread. Polling
`GetClipboardSequenceNumber` costs a counter read every 100 ms and buys back that
whole mechanism. macOS has no notification API at all, so it would have to poll
regardless.

## Why paste rather than type

The command is already in the clipboard, so the sequence sends `Ctrl+V` instead
of typing `/travel 1,2` character by character. That sidesteps keyboard layouts
entirely — on AZERTY the comma and the digits are not where a QWERTY key code
says they are.

## Why the send sequence is data

`Enter → paste → Enter` assumes the chat is closed when the command arrives. If
it is already open, the first `Enter` closes it instead. Rather than read the
screen to find out, the sequence is a list of steps in the configuration file —
editable if Dofus ever behaves differently, but deliberately absent from the
settings panel, which stays a five-setting window. The decision to add screen
reading (open question 2 in the vision) is deferred until there is evidence it is
needed, and `InputPort` is where a verifying implementation would slot in.

## The settings panel

One window, no navigation, five settings: the bridge switch, the source
application, the Dofus window, which commands to relay, and whether to clear the
clipboard afterwards.

It has no palette of its own. `src/app.css` uses CSS system colours (`Canvas`,
`CanvasText`, `GrayText`, `AccentColor`), the platform font stack and
`color-scheme: light dark`, so the panel follows the OS theme and accent colour
and the form controls stay the ones the webview already draws natively. Adding a
brand colour or a rounded border to a control would be a step away from that, not
towards it.

There is no in-app journal: the rotating log file is the record, and the panel
links to it with a single button (`open_log_folder`, opened from Rust so the
front end needs no filesystem capability). The cost is that a mid-send failure
shows up only as a system notification and a log line; the status line covers the
common case — no Dofus window open — because it is recomputed every two seconds.

## Platform notes

**Windows.** `SetForegroundWindow` is ignored for a background process, so
`focus` attaches to the foreground thread's input queue first
(`AttachThreadInput`) — the established workaround, and the core confirms the
result rather than trusting it. `SendInput` fails *silently* when the target is
more privileged (UIPI), so elevation is detected up front and surfaced as a
warning instead of being diagnosed after a session of nothing happening.

**macOS.** Not implemented yet; `unsupported.rs` keeps the app running and
honest. It will need `NSPasteboard.changeCount`, `NSWorkspace`, `AXUIElement` and
`CGEvent`, all gated behind Accessibility permission.

## Verification

| What | How |
|---|---|
| Behaviour, all failure paths | `cargo test -p zaapy-core` |
| Win32 layer, without a PC | `cargo check -p zaapy-platform --target x86_64-pc-windows-msvc` |
| Settings page types | `bun run check` |
| Real Windows build | CI job `windows`, or `cargo tauri build` on the PC |

What none of that covers, and only a human at a keyboard can: that the keystrokes
land in the Dofus chat and the character actually leaves.
