// Mirrors the serde representations in `zaapy-core`. Keep the two in step: the
// Rust side is the source of truth.

export type Modifier = "primary" | "ctrl" | "alt" | "shift" | "meta";

export type SendStep =
  | { type: "key"; key: string; modifiers: Modifier[] }
  | { type: "delay"; ms: number };

export interface TargetSelector {
  process: string;
  title_pattern: string;
}

export interface Config {
  version: number;
  enabled: boolean;
  source_processes: string[];
  target: TargetSelector;
  commands: { travel: boolean; zaap: boolean };
  // Only its first key reaches the settings panel — see `chatOpenKey`. The rest
  // is editable in the config file if Dofus ever needs a different sequence.
  send_sequence: SendStep[];
  focus_timeout_ms: number;
  focus_settle_ms: number;
}

export interface WindowRef {
  handle: number;
  pid: number;
  process: string;
  title: string;
}

export interface Status {
  enabled: boolean;
  configured: boolean;
  target: WindowRef | null;
  targetError: string | null;
  host: {
    inputPermitted: boolean;
    privilegeWarning: string | null;
    canRequestPermission: boolean;
  };
}

// The key that puts the Dofus chat into edit mode: the first keystroke of the
// send sequence.
//
// It lives in `send_sequence` rather than in a field of its own, because the
// sequence is what the platform layer actually plays — a second copy of the same
// fact would disagree with it the moment the file is hand-edited.
export function chatOpenKey(config: Config): string | null {
  const step = config.send_sequence.find((step) => step.type === "key");
  return step?.type === "key" ? step.key : null;
}

// Rebind that key, leaving the rest of the sequence — including any modifier
// someone wrote into that step by hand — exactly as it was.
export function withChatOpenKey(config: Config, key: string): Config {
  let replaced = false;
  const send_sequence = config.send_sequence.map((step) => {
    if (replaced || step.type !== "key") return step;
    replaced = true;
    return { ...step, key };
  });
  // A sequence with no keystroke at all can only come from a hand-edited file.
  // Opening the chat still has to come first, so the key goes at the front.
  if (!replaced) {
    send_sequence.unshift({ type: "key", key, modifiers: [] });
  }
  return { ...config, send_sequence };
}
