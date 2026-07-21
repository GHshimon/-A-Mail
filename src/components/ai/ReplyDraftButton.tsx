import { useState } from "react";
import { useAiStore } from "@/store/aiStore";
import { useComposeStore } from "@/store/composeStore";
import { useMailStore } from "@/store/mailStore";
import { aiGenerateReply } from "@/ipc/ai";
import { IpcError } from "@/ipc/client";

/**
 * C: 返信ドラフト生成ボタン。
 *
 * 直前に開いていた受信メールを元に、トーン/長さに沿った返信本文を Gemini が生成し、
 * 作成中本文の先頭へ挿入する(ユーザー編集前提)。既存本文(引用など)は下に残す。
 */
export function ReplyDraftButton() {
  const aiEnabled = useAiStore((s) => s.aiEnabled);
  const hasKey = useAiStore((s) => s.hasKey);
  const tone = useComposeStore((s) => s.tone);
  const length = useComposeStore((s) => s.length);
  const setBody = useComposeStore((s) => s.setBody);
  const openMessage = useMailStore((s) => s.openMessage);

  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const disabled = !aiEnabled || !hasKey || openMessage == null || generating;

  const onGenerate = async () => {
    const source = useMailStore.getState().openMessage;
    if (source == null) return;
    setError(null);
    setGenerating(true);
    try {
      const reply = await aiGenerateReply({
        source_message_id: source.header.id,
        tone,
        length,
      });
      const cur = useComposeStore.getState().draft.body;
      setBody(reply.trim() + (cur ? `\n\n${cur}` : ""));
    } catch (err) {
      setError(err instanceof IpcError ? err.message || err.kind : "生成に失敗しました");
    } finally {
      setGenerating(false);
    }
  };

  return (
    <>
      <button className="ai-generate" disabled={disabled} onClick={() => void onGenerate()}>
        {generating ? "生成中…" : "✦ 返信ドラフトを生成"}
      </button>
      {error && (
        <p className="cmp-status err" style={{ fontSize: 11.5, marginTop: 6 }}>
          {error}
        </p>
      )}
      {!aiEnabled || !hasKey ? (
        <p className="muted" style={{ fontSize: 11, marginTop: 6 }}>
          設定(⚙︎)で AI を有効化しキーを登録すると使えます。
        </p>
      ) : openMessage == null ? (
        <p className="muted" style={{ fontSize: 11, marginTop: 6 }}>
          受信メールを開いてから返信すると生成できます。
        </p>
      ) : null}
    </>
  );
}
