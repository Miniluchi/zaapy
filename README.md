# Zaapy

A click on a position in a Ganymède guide becomes an automatic travel in Dofus 3.

Ganymède already copies `/travel x,y` to the clipboard when you click a
coordinate. Zaapy sits in the tray, notices it, raises the Dofus client you
selected and types the command into its chat — so you never leave the guide to
paste it yourself. It only reacts when the command was copied while Ganymède was
in front, and never types into a window it has not confirmed is in the
foreground.

Windows and macOS are both supported. On macOS the bridge is built on the
Accessibility API, so it needs that permission before it can raise a window or
type into one; the settings panel asks for it and refuses to send until it has
it.

## Installing

Grab the latest build from the [Releases page](https://github.com/Miniluchi/zaapy/releases):
`Zaapy_x.y.z_x64-setup.exe` on Windows, `Zaapy_x.y.z_universal.dmg` on macOS.

Neither is code signed — a certificate costs more per year than this project
does — so both systems object the first time.

**Windows.** SmartScreen says "Windows protected your PC": *More info* → *Run
anyway*. The installer asks for no administrator rights and installs for the
current user, and it fetches the WebView2 runtime if the machine lacks it.

**macOS.** Gatekeeper refuses a double-click on an unsigned app. Right-click
Zaapy in Applications, choose *Open*, then *Open* again in the dialog — once per
install. From a terminal, the same thing:

```sh
xattr -dr com.apple.quarantine /Applications/Zaapy.app
```

Then grant Accessibility in System Settings → Privacy & Security →
Accessibility: without it Zaapy can neither raise a window nor type into one.

The grant is attached to this exact build, so **after updating, remove the old
Zaapy entry with `−`, then relaunch and grant it again**. Adding the new copy on
top of the old entry is not enough: macOS goes on showing a switch that is
already on while Zaapy still reports the permission as missing.

## Requirements

- [Rust](https://rustup.rs) (stable)
- [Bun](https://bun.sh)
- On Windows: Visual Studio Build Tools (MSVC) and the WebView2 runtime
- On macOS: the Xcode Command Line Tools, and Accessibility permission for Zaapy

## Running it

```sh
bun install
bun run tauri dev      # run the app
bun run tauri build    # produce an installer for the machine you are on
```

Zaapy starts in the tray with its window hidden; open the settings panel from the
tray icon.

On macOS, expect to grant Accessibility again after a rebuild: the permission is
attached to the binary, and `tauri dev` produces a new one each time. `tauri
build` gives you an app bundle that keeps the grant — and is also the only way to
see the failure notifications, which need a bundled app.

## Checks

```sh
cargo test -p zaapy-core                                        # behaviour, every failure path
cargo check -p zaapy-platform --target x86_64-pc-windows-msvc   # the Win32 layer, from any OS
cargo clippy -p zaapy-platform                                  # the macOS layer, on a Mac only
bun run check                                                   # settings panel types
```

The Windows layer is only *linked* on a Windows machine or by the `Windows build`
CI job; the cross-target check above type-checks it from a Mac or a Linux runner.
The macOS layer has no such shortcut — the Apple SDK does not travel — so it is
checked on a Mac or by the `macOS build` CI job, and nowhere else.

## Releasing

The version lives in one place — `version` in the workspace `Cargo.toml`.
`tauri.conf.json` has no version field of its own and inherits it.

```sh
# bump version in Cargo.toml, then
cargo check --workspace          # refresh Cargo.lock
git commit -am "chore: release x.y.z"
git tag vx.y.z && git push --follow-tags
```

The tag starts the `Release` workflow, which builds a universal macOS DMG and a
Windows installer and opens a **draft** GitHub release with both attached —
publish it by hand. Running the same workflow from the Actions tab builds
nothing but workflow artifacts, which is how to check the packaging without
cutting a tag.

## Layout

```
crates/zaapy-core/       the whole behaviour, behind four platform traits
crates/zaapy-platform/   the operating system, and nothing else
src-tauri/               tray, window, IPC, the 100 ms loop, logging
src/                     the settings panel (Svelte 5)
```

## Documentation

- [docs/VISION.md](docs/VISION.md) — what the product is and what it deliberately is not
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how it is built, and why it is split that way

## Compliance

Zaapy reproduces a keystroke you would otherwise type yourself, triggered by a
click you just made. It does not read game memory and does not control your
character beyond delivering a chat command. Whether that is acceptable under
Ankama's terms of service is your call, not a guarantee this project makes.
