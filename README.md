# Zaapy

**Click a position in a Ganymède guide, and your character travels there in
Dofus 3. No copy-paste, no switching windows.**

<p align="center">
  <img src="assets/icon-1024.png" width="128" alt="Zaapy">
</p>

## What it does

When you click a coordinate in a Ganymède guide, Ganymède copies `/travel x,y`
to your clipboard. Normally you would then switch to Dofus, open the chat,
paste, press Enter, and switch back.

Zaapy does that part for you. It runs quietly in the background and, the moment
Ganymède copies a travel command, it brings your Dofus client to the front and
types the command into the chat. Your character leaves, and you never looked
away from the guide.

Works on **Windows** and **macOS**.

## Install

Download the latest version from the
[Releases page](https://github.com/Miniluchi/zaapy/releases):

| System | File |
|---|---|
| Windows | `Zaapy_1.0.0_x64-setup.exe` |
| macOS | `Zaapy_1.0.0_universal.dmg` (Intel and Apple Silicon) |

Zaapy is not code signed — the certificates cost more per year than this free
project does — so both systems will warn you the first time. Here is how to get
past it.

### Windows

1. Run the installer. Windows shows a blue **"Windows protected your PC"**
   screen.
2. Click **More info**, then **Run anyway**.
3. The installer needs no administrator rights and installs just for you. If
   your PC is missing the WebView2 runtime, it downloads it automatically.

### macOS

1. Open the `.dmg` and drag Zaapy into **Applications**.
2. **Do not double-click it.** Right-click Zaapy in Applications, choose
   **Open**, then **Open** again in the dialog. You only need to do this once.
3. Zaapy needs permission to control other windows. Open its settings from the
   menu bar icon and click **Grant permission…**, then allow Zaapy in the dialog
   macOS shows.
4. If no dialog appears — macOS only asks once — turn Zaapy on by hand in
   **System Settings → Privacy & Security → Accessibility**.

Without that permission Zaapy cannot bring up your Dofus window or type into it,
and it will tell you so in its settings rather than failing silently.

> **When you update Zaapy on macOS**, remove the old Zaapy entry from the
> Accessibility list with the **−** button first, then relaunch Zaapy and add it
> again. Leaving the old entry in place looks like it works — the switch is
> still on — but Zaapy will keep saying the permission is missing.

## First launch

Zaapy has no main window. It lives in your **menu bar** (macOS) or **system
tray** (Windows), next to the clock. Click its icon and choose **Settings…**.

There are four things to set:

1. **Bridge enabled** — the master switch. Leave it on.
2. **Ganymède window** — pick Ganymède in the list. Zaapy will only react to
   commands copied while this app is in front.
3. **Dofus window** — pick the client you want the commands sent to. Running
   several accounts? Pick the character you are currently following the guide
   with; you can change it any time.
4. **Dofus chat key** — the key that opens the chat in Dofus. This is **Enter**
   unless you changed it in the game's controls. If you did change it, click the
   button and press your key.

That's it. Close the window — Zaapy keeps running in the tray.

> Ganymède must be the **desktop app**. Zaapy cannot see Ganymède running in a
> browser tab.

## Using it

Follow your guide as usual and click a position. Zaapy takes over from there.

A green line in the settings window tells you which character commands are
currently going to. When something goes wrong — your Dofus client is closed, for
example — Zaapy shows a system notification saying why. The command stays in your
clipboard, so you can always paste it yourself.

`/zaap` is greyed out for now. Ankama has announced the command but it does not
exist in the game yet; Zaapy will support it when it ships.

## If nothing happens

Work down this list — it is roughly in order of likelihood.

**Check the settings window first.** It shows a status line under the master
switch, and it is usually the answer.

- **"Select the Ganymède and Dofus windows below."** — one of the two pickers is
  still empty.
- **No window matching the selected Dofus client is open.** — the client you
  picked was closed, or you logged in on a different character. Pick it again.
- **Several windows match** — you are running more than one client with similar
  titles. Pick the specific one from the list again.

**The command appears in the chat but nothing travels.** Your chat key is
probably wrong, so the first keypress did something other than open the chat.
Check **Dofus chat key** against the keybind in the game's controls.

**Nothing happens at all, and no notification appears.**

- Make sure you clicked the coordinate **inside Ganymède**. Zaapy deliberately
  ignores anything copied while another app is in front — that is what keeps it
  from reacting to your own copy-pasting.
- On macOS, re-check the Accessibility permission, especially after an update
  (see the note above).
- On Windows, if Dofus is running as administrator and Zaapy is not, Windows
  silently discards the keystrokes. Zaapy warns you about this in its settings;
  restart Zaapy as administrator too.

**Still stuck?** The settings window has an **Open log folder** button. The log
records what Zaapy did and why, and it is the right thing to attach if you
[open an issue](https://github.com/Miniluchi/zaapy/issues).

## Your clipboard and your privacy

- Zaapy **only reads** your clipboard. It never changes it or clears it — after a
  send, the command is still there for you to paste manually.
- It only acts on a command you just copied **in Ganymède**. Anything you copy
  anywhere else is ignored.
- It never types into a window without first confirming that window is actually
  in front.
- Nothing is sent anywhere. Zaapy has no account, no server and no telemetry; it
  only ever talks to your own clipboard, windows and keyboard.

## Is this allowed?

Zaapy types a keystroke you would otherwise type yourself, in direct response to
a click you just made. It does not read game memory, does not play for you, and
does not control your character beyond delivering one chat command.

Whether that is acceptable under Ankama's terms of service is your call. This
project makes no guarantee about it.

## Licence and source

Zaapy is free and open source under the [MIT licence](LICENSE).

- [Contributing and building from source](docs/DEVELOPMENT.md)
- [What the product is, and what it deliberately is not](docs/VISION.md)
- [How it is built](docs/ARCHITECTURE.md)
