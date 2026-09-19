import { invoke } from "@tauri-apps/api/core";

import type { Config, Status, WindowRef } from "./types";

export const getConfig = () => invoke<Config>("get_config");
export const setConfig = (config: Config) => invoke<Config>("set_config", { config });
export const listWindows = () => invoke<WindowRef[]>("list_windows");
export const getStatus = () => invoke<Status>("get_status");
export const openLogFolder = () => invoke<void>("open_log_folder");
export const requestPermissions = () => invoke<void>("request_permissions");
