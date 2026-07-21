import { invoke } from "./client";
import type {
  CompleteReq,
  ContextReq,
  ContextResult,
  ReplyReq,
  Settings,
  SettingsPatch,
} from "./types";

// AI(Gemini)関連 + 設定 + Keychain キー管理の型付きラッパ。
// M3 は非ストリーミング generateContent。SSE ストリーミングは将来対応。

/** A: 予測入力。カーソル直前までの本文から続きを返す(無効/キー無なら Ai エラー)。 */
export function aiComplete(req: CompleteReq): Promise<string> {
  return invoke<string>("ai_complete", { req });
}

/** B: 関連情報サイドバー(FTS5 → Gemini 要点化)。 */
export function aiContextSidebar(req: ContextReq): Promise<ContextResult> {
  return invoke<ContextResult>("ai_context_sidebar", { req });
}

/** C: 返信ドラフト生成。 */
export function aiGenerateReply(req: ReplyReq): Promise<string> {
  return invoke<string>("ai_generate_reply", { req });
}

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
