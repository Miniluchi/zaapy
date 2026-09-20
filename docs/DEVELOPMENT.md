# Zaapy — Development

Everything needed to build Zaapy from source. For what the product does and how
to install it, see the [README](../README.md); for how it is put together, see
[ARCHITECTURE.md](ARCHITECTURE.md).

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

## Layout

```
crates/zaapy-core/       the whole behaviour, behind four platform traits
crates/zaapy-platform/   the operating system, and nothing else
src-tauri/               tray, window, IPC, the 100 ms loop, logging
src/                     the settings panel (Svelte 5)
```

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

Neither bundle is signed. The README tells users how to get past SmartScreen and
Gatekeeper; if that ever changes, the install section is what has to be updated
alongside the certificates.

After publishing, check that the asset names in the README's install table still
match what the workflow produced — they carry the version number.
