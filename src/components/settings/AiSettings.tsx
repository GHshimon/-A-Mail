import { useState } from "react";
import { useAiStore } from "@/store/aiStore";
import { isTauri } from "@/ipc/client";
import { setGeminiKey, updateSettings } from "@/ipc/ai";

/**
 * AI 設定(オプトイン・送信範囲・キー登録)。
 *
 * プライバシー方針: AI 補助は既定 OFF。明示的にオンにしたときだけメール本文を
 * Gemini に送信する。API キーは Keychain に保存し、値はフロントへ返さない。
 *
 * M0 は骨組み。設定画面への導線(⌘,)は後続で配線する。
 */
export function AiSettings() {
  const aiEnabled = useAiStore((s) => s.aiEnabled);
  const setAiEnabled = useAiStore((s) => s.setAiEnabled);
  const hasKey = useAiStore((s) => s.hasKey);
  const setHasKey = useAiStore((s) => s.setHasKey);

  const [keyInput, setKeyInput] = useState("");
  const [saving, setSaving] = useState(false);

  const onToggleAi = async (v: boolean) => {
    setAiEnabled(v);
    if (isTauri()) await updateSettings({ ai_enabled: v });
  };

  const onSaveKey = async () => {
    if (!keyInput.trim()) return;
    setSaving(true);
    try {
      if (isTauri()) await setGeminiKey(keyInput.trim());
      setHasKey(true);
      setKeyInput("");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="settings-pane">
      <h2>AI アシスト設定</h2>

      <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <input
          type="checkbox"
          checked={aiEnabled}
          onChange={(e) => void onToggleAi(e.target.checked)}
        />
        AI 補助を有効にする(既定: オフ)
      </label>
      <p style={{ fontSize: 12, color: "var(--ink-3)" }}>
        オンにすると、作成中メールや関連する過去メールの本文が Gemini
        (Google)へ送信されます。
      </p>

      <h3>Gemini API キー</h3>
      <p style={{ fontSize: 12, color: "var(--ink-3)" }}>
        {hasKey ? "登録済み(Keychain に保存)" : "未登録"}
      </p>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          type="password"
          value={keyInput}
          placeholder="AIza... "
          onChange={(e) => setKeyInput(e.target.value)}
          style={{ flex: 1 }}
        />
        <button className="primary" disabled={saving} onClick={() => void onSaveKey()}>
          保存
        </button>
      </div>
    </div>
  );
}
