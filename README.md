# Zaapy

**Click a position in a Ganymède guide, and your character travels there in
Dofus 3. No copy-paste, no switching windows.**

<p align="center">
  <img src="assets/icon-1024.png" width="128" alt="Zaapy">
</p>

## What it does

Clicking a coordinate in a Ganymède guide copies `/travel x,y` to your
clipboard. Normally you'd switch to Dofus, open the chat, paste, press Enter,
and switch back.

Zaapy does that part for you: the moment Ganymède copies a travel command, it
brings Dofus to the front and types the command into the chat. You never leave
the guide.

Works on **Windows** and **macOS**.

## Install

Download the latest version from the
[Releases page](https://github.com/Miniluchi/zaapy/releases):

| System  | File                                                  |
| ------- | ----------------------------------------------------- |
| Windows | `Zaapy_1.0.0_x64-setup.exe`                           |
| macOS   | `Zaapy_1.0.0_universal.dmg` (Intel and Apple Silicon) |

Zaapy isn't code signed — the certificates cost more per year than this free
project does — so both systems will warn you the first time.

### Windows

1. Run the installer. Windows shows a blue **"Windows protected your PC"**
   screen.
2. Click **More info**, then **Run anyway**.
3. No administrator rights needed; it installs just for you and fetches the
   WebView2 runtime automatically if missing.

### macOS

1. Open the `.dmg` and drag Zaapy into **Applications**.
2. **Do not double-click it.** Right-click Zaapy in Applications, choose
   **Open**, then **Open** again in the dialog. You only need to do this once.
3. Zaapy needs permission to control other windows. Open its settings from the
   menu bar icon and click **Grant permission…**, then allow it in the dialog
   macOS shows.
4. If no dialog appears — macOS only asks once — enable Zaapy by hand in
   **System Settings → Privacy & Security → Accessibility**.

Without that permission Zaapy can't bring up or type into your Dofus window,
and its settings will say so rather than failing silently.

## Using it

Follow your guide as usual and click a position. Zaapy takes over from there.

`/zaap` is greyed out for now. Ankama has announced the command but it does not
exist in the game yet; Zaapy will support it when it ships.

## Privacy

- Zaapy **only reads** your clipboard, never clears or changes it — after a
  send, the command is still there to paste manually.
- It only acts on commands copied **in Ganymède**; anything else is ignored.
- It never types into a window without confirming that window is in front.
- Nothing is sent anywhere: no account, no server, no telemetry.

## Licence and source

Zaapy is free and open source under the [MIT licence](LICENSE).

- [Contributing and building from source](docs/DEVELOPMENT.md)
- [What the product is, and what it deliberately is not](docs/VISION.md)
- [How it is built](docs/ARCHITECTURE.md)
