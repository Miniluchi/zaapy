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
  // Not shown in the settings panel: editable in the config file if Dofus ever
  // needs a different sequence, or a slower machine a longer wait.
  send_sequence: SendStep[];
  focus_timeout_ms: number;
  focus_settle_ms: number;
  clear_clipboard_on_success: boolean;
  clipboard_clear_delay_ms: number;
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
