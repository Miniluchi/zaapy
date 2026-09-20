# Zaapy — Product Vision

> A bridge between two windows: Ganymède speaks "clipboard", Dofus listens to "keyboard", Zaapy does the translation.

## 1. The problem

Following a guide in **Ganymède** means clicking a coordinate, then leaving the guide: switch to the Dofus client, open the chat, paste, press Enter, switch back. Ganymède already puts `/travel x,y` in the clipboard on click — the last mile is entirely manual.

That round trip costs a few seconds and, more importantly, one break in attention. Repeated dozens of times per session, it is the single most tedious part of following a guide.

## 2. Vision in one sentence

**A click on a position in Ganymède becomes an automatic travel in Dofus 3 — no manual copy-paste, eyes never leave the guide.**

## 3. The nominal flow

1. The user follows a guide in Ganymède and clicks a coordinate.
2. Ganymède places `/travel 1,2` in the clipboard — native behavior, nothing to change on its side.
3. Zaapy, running in the background, detects the new clipboard content **and checks that the foreground window at that moment was Ganymède**. If it was not, nothing happens.
4. Zaapy focuses the selected Dofus client window, opens the chat, pastes the command, and validates it.
5. The character starts traveling. Focus stays on Dofus, so the user sees the character leave.

Step 3 is the safety gate of the whole product: Zaapy only ever acts as the direct consequence of a deliberate user click inside Ganymède.

## 4. Target user

A Dofus 3 player following a Ganymède guide, on **Windows or macOS**, possibly running several clients side by side (multi-account). They want the guide to drive the game, not to babysit a clipboard.

## 5. Guiding principles

- **Invisible in normal use.** No window to manage, no step to remember. When it works, the user should forget it exists.
- **Never act without a human click.** Every action Zaapy takes traces back to something the user just did in Ganymède.
- **Fail loudly, not silently.** If the command cannot be delivered, say so immediately — a silent failure in the middle of a guide is worse than no automation at all.
- **The clipboard belongs to the user.** Zaapy reads it and writes nothing back.
- **Multi-account is explicit, never guessed.** The user says which client is the target; Zaapy does not infer it.

## 6. Scope — v1 (MVP)

| Area | Decision |
|---|---|
| Platforms | Windows **and** macOS |
| macOS permissions | **Accessibility**, asked for from the settings window. The source gate works without it, so nothing is ever sent from an unpermitted install — the panel says what is missing instead |
| Trigger | Fully automatic and immediate, **gated**: the command is only bridged if Ganymède was the foreground window when it appeared in the clipboard |
| Source app | Ganymède **desktop app** (detected by process/window, no browser support) |
| Commands relayed | `/travel` today; `/zaap` designed for but not active (see open questions) |
| Dofus target | **Manual selection** of the client window inside Zaapy — the user picks it, Zaapy remembers it |
| Focus after send | **Stays on Dofus** — no automatic return to Ganymède |
| Clipboard after send | **Untouched** — the command stays where Ganymède put it, ready for a manual paste. Zaapy reads the clipboard to know there is something to relay, and that is the whole of its business with it |
| Interface | System tray / menu bar icon **plus** a compact settings window that follows the OS theme: which two windows to bridge, which commands to relay, and a link to the log folder |
| Send sequence | **Blind typing** — open the chat, paste, validate — made safe by confirming the Dofus window is actually in front rather than by reading the screen |
| Failures | **System notification** with a clear cause (e.g. "Dofus window not found"); the command stays available for a manual paste |
| Audience | **Personal use first, public later** — MVP for the author, architecture kept clean enough for a community release |

## 7. Out of scope for v1

- Any form of travel bot, pathfinding, or combat automation. Zaapy does not play the game.
- Account management, login automation, or credential handling.
- Broadcasting a command to several Dofus clients at once.
- Ganymède in a browser tab — desktop app only.
- Automatic focus return to Ganymède after sending.
- An in-app activity journal. The rotating log file is the record, and the settings window only links to it.
- Guide content of any kind. Zaapy has no opinion on where the user should go.

## 8. Open questions

These are deliberately left undecided here:

1. **`/zaap` support.** Announced but not yet implemented in-game. Only `/travel` works today, so the command handling must be able to accept `/zaap` later without a redesign — but the exact trigger and formatting cannot be locked in until Ankama ships it.
2. **Distribution.** Signed installers, notarization, and auto-update only become relevant at the public-release step — but they constrain packaging choices made earlier.

## 9. Beyond v1 — non-committal directions

- Broadcasting to several clients for multi-account play.
- A configurable allowlist of relayed command prefixes.
- Optional focus return to Ganymède, for users who prefer to stay in the guide.
- Signed installer and auto-update for a community release.
- Ganymède web support, if the desktop app stops being the common case.

## 10. Risk and compliance

Zaapy reproduces a keystroke the user would otherwise type themselves, triggered by an explicit click they just made. It does not read game memory, does not control the character beyond delivering a chat command, and never acts on its own initiative.

That said, whether this kind of tooling is acceptable under Ankama's terms of service is the user's responsibility, not a guarantee this project makes.

## 11. Glossary

- **Ganymède** — companion app providing Dofus guides; copies `/travel` commands to the clipboard on click.
- **Dofus 3** — the game client Zaapy sends commands to.
- **`/travel x,y`** — in-game chat command that sends the character to the map at those coordinates.
- **Zaap** — in-game teleportation network; the basis for the announced `/zaap` command.
- **Bridge** — Zaapy's core job: turning a clipboard event in one window into a keyboard event in another.
- **Target window** — the specific Dofus client the user selected to receive commands.
