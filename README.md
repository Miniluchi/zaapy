# Zaapy

A click on a position in a Ganymède guide becomes an automatic travel in Dofus 3.

Ganymède already copies `/travel x,y` to the clipboard when you click a
coordinate. Zaapy sits in the tray, notices it, raises the Dofus client you
selected and types the command into its chat — so you never leave the guide to
paste it yourself. It only reacts when the command was copied while Ganymède was
in front, and never types into a window it has not confirmed is in the
foreground.

Windows is the platform the bridge targets first. The macOS ports go through the
Accessibility API and are not wired up.

## Requirements

- [Rust](https://rustup.rs) (stable)
- [Bun](https://bun.sh)
- On Windows: Visual Studio Build Tools (MSVC) and the WebView2 runtime

## Running it

```sh
bun install
bun run tauri dev      # run the app
bun run tauri build    # produce an installer
```

Zaapy starts in the tray with its window hidden; open the settings panel from the
tray icon.

## Checks

```sh
cargo test -p zaapy-core                                        # behaviour, every failure path
cargo check -p zaapy-platform --target x86_64-pc-windows-msvc   # the Win32 layer, from any OS
bun run check                                                   # settings panel types
```

The Windows layer is only *linked* on a Windows machine or by the `Windows build`
CI job; the cross-target check above type-checks it from a Mac or a Linux runner.

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
