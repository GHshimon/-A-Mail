import { invoke } from "./client";
import type { Settings, SettingsPatch } from "./types";

// AI(Gemini)関連 + 設定 + Keychain キー管理の型付きラッパ。
// 補完(A)のストリーミングは Tauri Channel を使うため M3 で追加する。

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function updateSettings(patch: SettingsPatch): Promise<Settings> {
  return invoke<Settings>("update_settings", { patch });
}

/** Gemini API キーを Keychain に保存(DB には保存しない)。 */
export function setGeminiKey(apiKey: string): Promise<void> {
  return invoke<void>("set_gemini_key", { apiKey });
}

/** キー登録有無のみを返す(値そのものはフロントに出さない)。 */
export function hasGeminiKey(): Promise<boolean> {
  return invoke<boolean>("has_gemini_key");
}
