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
src/                     the settings panel (Svelte 5)
.github/workflows/ci.yml the only place the Win32 layer is linked, not just checked
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
8. Waits for the window to settle. Being in the foreground is not being ready to
   read input; a game just pulled out of the background drops the first frames'
   worth of keystrokes.
9. Plays the send sequence.

The clipboard is never written to, only read: the command stays where Ganymède
put it, available for a manual paste whether the send worked or not.

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
screen to find out, the sequence is a list of steps in the configuration file,
editable if Dofus ever behaves differently. Screen reading stays off the table
until there is evidence in game that the chat state actually varies; `InputPort`
is where a verifying implementation would slot in if it ever does.

The first step is the one exception the panel makes to that: the key that opens
the chat is a keybind Dofus lets the player move, so Zaapy has to follow it or
type into a chat that never opened. The closing `Enter` validates the line, which
Dofus does not let them move, and it stays fixed. The setting reads and writes
that first step in place rather than carrying a key of its own, because the
sequence is what actually gets played — a second copy of the same fact would
disagree with it the moment the file is hand-edited.

## The settings panel

A 420×400 window, hidden at launch and opened from the tray, because in normal
use there is nothing to look at. No navigation, five settings: the bridge switch,
the Ganymède window, the Dofus window, which commands to relay, and the key that
opens the game's chat. Both window pickers are dropdowns over the live
window list; choosing a Dofus client writes its process name *and* the character
name from its title, so the multi-client filter is implied by the pick rather
than typed.

The bridge switch exists twice — in the tray menu and at the top of the panel —
so both writes go through one funnel, `runtime::apply_config`. It saves through
`Runtime`, moves the tray's check mark (on the main thread; menus may not be
touched from anywhere else on macOS) and emits `config-changed` to the webview,
which the panel listens to. Either surface may be the one the user reaches for,
and neither is allowed to show a value the other has already changed.

It has no palette of its own. `src/app.css` uses CSS system colours (`Canvas`,
`CanvasText`, `GrayText`, `AccentColor`), the platform font stack and
`color-scheme: light dark`, so the panel follows the OS theme and accent colour
and the form controls stay the ones the webview already draws natively. Adding a
brand colour or a rounded border to a control would be a step away from that, not
towards it. The one border the stylesheet draws is the circle around the chat
key's `i` marker, which is a mark in a line of text rather than a control —
`ⓘ` itself is absent from the system font on macOS and falls through to a CJK
fallback face.

Explanations too long for a label go in a native `title` tooltip, which both
WKWebView and WebView2 render themselves: the `/zaap` row says why it is greyed
out, and the chat key says which keybind it has to match. A tooltip nobody hovers
is invisible, so the ones worth reading are marked.

Nothing is offered that cannot be acted on: `HostStatus::can_request_permission`
exists so the panel does not show a "Grant permission…" button on a platform
where there is nothing to grant.

There is no in-app journal: the rotating log file is the record, and the panel
links to it with a single button (`open_log_folder`, opened from Rust so the
front end needs no filesystem capability). The cost is that a mid-send failure
shows up only as a system notification and a log line; the status line covers the
common case — no Dofus window open — because it is recomputed every two seconds.

The rest of the send sequence and the focus timeout are deliberately absent from
it: they live in the configuration file, reachable when Dofus misbehaves, without
turning a five-line panel into a keystroke editor. The chat key earns its row by
being a setting *in Dofus* — the others only ever move to work around a bug.

## The tray icon

Not the application icon: that one is a square with a background, which is right
for the Dock and the Start menu and wrong for a menu bar. `icons/tray.png` is the
glyph alone on transparency, at 32 px for the Windows notification area and
128 px for a Retina menu bar, chosen by `cfg` and embedded at compile time with
`include_image!`. It is deliberately *not* a template image: a template is drawn
from the alpha channel alone, which would flatten the portal into a black blob.

## Distribution

`bun run tauri build` produces a `.dmg` on macOS and an NSIS `.exe` on Windows —
no MSI, which cannot install per-user without administrator rights. The `Release`
workflow does both from a `v*` tag and opens a draft GitHub release.

Neither bundle carries a real certificate, but the macOS one **must** still be
signed ad-hoc (`bundle.macOS.signingIdentity: "-"`). Without it the bundler ships
an app whose executable is merely linker-signed: `codesign --verify` answers
"code object is not signed at all", the `Info.plist` is unbound and no resources
are sealed. macOS still runs it, and still lists it in Privacy & Security with a
switch the user can turn on — but `AXIsProcessTrusted` keeps answering false,
because there is no code identity for the grant to attach to. The symptom is a
permission the settings pane says was given and the app says was not.

Ad-hoc signing fixes that, per build: the identity is the bundle's `cdhash`, so
every new version is a new app as far as TCC is concerned and the Accessibility
entry has to be removed and re-added. Only a Developer ID signature, which
survives across versions, removes that step — along with the Gatekeeper warning
and the need to notarize.

## Platform notes

**Windows.** `SetForegroundWindow` is ignored for a background process, so
`focus` attaches to the foreground thread's input queue first
(`AttachThreadInput`) — the established workaround, and the core confirms the
result rather than trusting it. `SendInput` fails *silently* when the target is
more privileged (UIPI), so elevation is detected up front and surfaced as a
warning instead of being diagnosed after a session of nothing happening.

**macOS.** The ports map to `NSPasteboard.changeCount` for the clipboard,
`NSWorkspace` for the foreground application, `AXUIElement` for window titles and
raising — chosen over `CGWindowListCopyWindowInfo`, which would demand Screen
Recording on top — and `CGEvent` for keystrokes. Four differences are worth
knowing before touching the file:

*A window is identified by its process id*, and an application contributes one
row rather than one per window. macOS has no public, stable window identifier,
and the core only ever compares `handle` against the foreground sample to confirm
a raise — a question a pid answers exactly, since the frontmost window on macOS
*is* the focused window of the frontmost application. The alternative was the
private `_AXUIElementGetWindow`. The cost is a multi-window application appearing
once in the picker; Dofus runs one process per client, so multi-account selection
is untouched.

*Only the title and the raise need Accessibility.* `foreground()` reads
`NSWorkspace`, which needs nothing, so the source gate is honest from the first
launch and the window picker stays populated — falling back to the application
name — while the panel is still asking for the permission.

*Raising is done twice.* `AXRaise` on the window and `activate` on the
application: since macOS 14 the system may ignore an activation request from an
app that is not itself active, and Zaapy runs as an accessory with no window of
its own. Neither call is dependable alone, and the core confirms the result
regardless.

*`CGEventPost` returns nothing and drops every event silently* when the process
is not a trusted Accessibility client — the same shape of trap as UIPI on
Windows, so `send` asks `AXIsProcessTrusted` up front and fails with
`PermissionDenied` instead. A chord also needs both the modifier key events and
the flags set on the key event by hand, because a synthesised modifier press does
not change the flags the system stamps onto what follows it.

All of it is gated behind Accessibility permission, which is what
`can_request_permission` is there to surface.

**Anything else.** `unsupported.rs` is selected by `cfg` wherever no port module
applies. It fails with a plain reason rather than refusing to build, so the app
still starts, the panel still works, and a Linux CI runner can compile the crate.

## Verification

| What | How |
|---|---|
| Behaviour, all failure paths | `cargo test -p zaapy-core` |
| Win32 layer, without a PC | `cargo check`/`cargo clippy -p zaapy-platform --target x86_64-pc-windows-msvc` |
| Settings panel types | `bun run check` |
| Win32 layer, actually linked | the `Windows build` CI job, or `bun run tauri build` on the PC |
| macOS layer | `cargo clippy -p zaapy-platform` on a Mac, or the `macOS build` CI job |

The two platform layers are not equally reachable. Windows can be type-checked
from anywhere because its SDK ships with the toolchain; the Apple SDK does not
travel, so `macos.rs` compiles on a Mac and on the macOS runner, and nowhere
else. A change to it cannot be reviewed from a PC the way the reverse works.

What none of that covers, and only a human at a keyboard can: that the keystrokes
land in the Dofus chat and the character actually leaves.
