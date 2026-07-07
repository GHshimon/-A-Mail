import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { useComposeStore } from "@/store/composeStore";
import { useAiStore } from "@/store/aiStore";
import { useDebouncedCompletion } from "@/hooks/useDebouncedCompletion";

/**
 * 予測入力(A)エディタ: textarea + overlay 方式。
 *
 * - 下層に実テキストの <textarea>、上に同一フォント/行送りの <div> を重ね、
 *   `本文 + <span class="ghost">補完</span>` を描画してゴーストを見せる。
 * - Tab でゴーストを本文へ確定。Esc / 文字入力でゴースト破棄。
 * - IME 変換中(composition)は補完トリガを止める。
 *
 * M0 は UI 挙動のみ(ゴーストは aiStore のモック値)。実際の Gemini ストリーミング
 * 購読は M3 で useDebouncedCompletion の onTrigger に配線する。
 */
export function GhostTextEditor() {
  const body = useComposeStore((s) => s.draft.body);
  const setBody = useComposeStore((s) => s.setBody);

  const aiEnabled = useAiStore((s) => s.aiEnabled);
  const ghostText = useAiStore((s) => s.ghostText);
  const clearGhost = useAiStore((s) => s.clearGhost);

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

  // M3 で Gemini 補完へ接続する箇所。M0 は no-op(モックのゴーストを維持)。
  useDebouncedCompletion(
    body,
    () => {
      /* TODO(M3): ai_complete を Channel 購読して setGhostText する */
    },
    { delay: 300, enabled: aiEnabled && !composing },
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
