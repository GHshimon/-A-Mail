import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { useComposeStore } from "@/store/composeStore";
import { useAiStore } from "@/store/aiStore";
import { useDebouncedCompletion } from "@/hooks/useDebouncedCompletion";
import { aiComplete } from "@/ipc/ai";

/**
 * 予測入力(A)エディタ: textarea + overlay 方式。
 *
 * - 下層に実テキストの <textarea>、上に同一フォント/行送りの <div> を重ね、
 *   `本文 + <span class="ghost">補完</span>` を描画してゴーストを見せる。
 * - Tab でゴーストを本文へ確定。Esc / 文字入力でゴースト破棄。
 * - IME 変換中(composition)は補完トリガを止める。
 *
 * ai_enabled かつキー登録済みのとき、入力停止 300ms で ai_complete を呼び、
 * 返ってきた続きをゴースト表示する。世代 ID で古い結果を破棄し、A は失敗しても
 * 静かに諦める(エラートーストは出さない)。
 */
export function GhostTextEditor() {
  const body = useComposeStore((s) => s.draft.body);
  const setBody = useComposeStore((s) => s.setBody);

  const aiEnabled = useAiStore((s) => s.aiEnabled);
  const hasKey = useAiStore((s) => s.hasKey);
  const ghostText = useAiStore((s) => s.ghostText);
  const clearGhost = useAiStore((s) => s.clearGhost);
  const setGhostText = useAiStore((s) => s.setGhostText);
  const bumpGen = useAiStore((s) => s.bumpGen);

  const [composing, setComposing] = useState(false);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);

  // textarea のスクロールに overlay を追従。
  const syncScroll = useCallback(() => {
    if (overlayRef.current && taRef.current) {
      overlayRef.current.scrollTop = taRef.current.scrollTop;
      overlayRef.current.scrollLeft = taRef.current.scrollLeft;
    }
  }, []);
  useLayoutEffect(syncScroll, [body, ghostText, syncScroll]);

  // 入力停止 300ms で Gemini 予測入力を呼ぶ(A)。
  useDebouncedCompletion(
    body,
    async (val) => {
      const { accountId, subject, to } = useComposeStore.getState().draft;
      if (accountId == null || !val.trim()) {
        setGhostText("");
        return;
      }
      const gen = bumpGen();
      try {
        const text = await aiComplete({
          account_id: accountId,
          subject,
          to,
          body_prefix: val,
        });
        // 古い世代・本文が変わっていたら破棄。
        if (useAiStore.getState().requestGen !== gen) return;
        if (useComposeStore.getState().draft.body !== val) return;
        setGhostText(text.trim());
      } catch {
        // A は体感優先で静かに諦める(次のタイプで再試行)。
      }
    },
    { delay: 300, enabled: aiEnabled && hasKey && !composing },
  );

  const confirmGhost = useCallback(() => {
    if (!ghostText) return;
    setBody(body + ghostText);
    clearGhost();
  }, [body, ghostText, setBody, clearGhost]);

  return (
    <div className="ghost-editor">
      <div className="ghost-layer" ref={overlayRef} aria-hidden="true">
        {body}
        {aiEnabled && ghostText && <span className="ghost">{ghostText}</span>}
      </div>
      <textarea
        ref={taRef}
        value={body}
        spellCheck={false}
        aria-label="本文"
        onScroll={syncScroll}
        onCompositionStart={() => setComposing(true)}
        onCompositionEnd={() => setComposing(false)}
        onChange={(e) => {
          setBody(e.target.value);
          if (ghostText) clearGhost();
        }}
        onKeyDown={(e) => {
          if (e.key === "Tab" && ghostText) {
            e.preventDefault();
            confirmGhost();
          } else if (e.key === "Escape" && ghostText) {
            e.preventDefault();
            clearGhost();
          }
        }}
      />
    </div>
  );
}
