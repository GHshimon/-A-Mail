import { useEffect, useRef } from "react";
import { useAiStore } from "@/store/aiStore";
import { useComposeStore } from "@/store/composeStore";
import { useUiStore } from "@/store/uiStore";
import { aiContextSidebar } from "@/ipc/ai";
import { IpcError } from "@/ipc/client";
import { RelatedMailList } from "./RelatedMailList";
import { ReplyDraftButton } from "./ReplyDraftButton";

const POINT_LABEL: Record<string, string> = {
  history: "経緯",
  commit: "約束",
  todo: "未対応",
};

const CONTEXT_DEBOUNCE_MS = 1200;

/**
 * AI 補助ドロワー(B)。作成時に本文エリアの右側へ出現し、関連過去メールの要点と
 * 一覧を表示。フッタに返信ドラフト生成(C)ボタン。
 *
 * ai_enabled かつキー登録済みのとき、件名/本文の変化(1.2s デバウンス)で
 * ai_context_sidebar を呼び、要点と関連メールを更新する。
 */
export function AiSidebar() {
  const points = useAiStore((s) => s.points);
  const relatedMails = useAiStore((s) => s.relatedMails);
  const loading = useAiStore((s) => s.contextLoading);
  const error = useAiStore((s) => s.contextError);
  const aiEnabled = useAiStore((s) => s.aiEnabled);
  const hasKey = useAiStore((s) => s.hasKey);
  const setContext = useAiStore((s) => s.setContext);
  const setContextLoading = useAiStore((s) => s.setContextLoading);
  const setContextError = useAiStore((s) => s.setContextError);

  const subject = useComposeStore((s) => s.draft.subject);
  const bodyLen = useComposeStore((s) => s.draft.body.length);

  const toggleAi = useUiStore((s) => s.toggleAi);
  const model = "Gemini Flash";

  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (!aiEnabled || !hasKey) return;
    const { accountId, to, subject, body } = useComposeStore.getState().draft;
    if (accountId == null) return;
    if (subject.trim() === "" && body.trim() === "") return;

    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      setContextLoading(true);
      setContextError(null);
      try {
        const res = await aiContextSidebar({ account_id: accountId, to, subject, body });
        setContext(res.points, res.related);
      } catch (err) {
        setContextError(
          err instanceof IpcError ? err.message || err.kind : "取得に失敗しました",
        );
      } finally {
        setContextLoading(false);
      }
    }, CONTEXT_DEBOUNCE_MS);

    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [subject, bodyLen, aiEnabled, hasKey]);

  const disabled = !aiEnabled || !hasKey;

  return (
    <aside className="ai" aria-label="AI アシスト">
      <div className="ai-head">
        <div>
          <div className="ai-title">
            <span className="spark">✦</span> AI アシスト
          </div>
          <div className="ai-sub">
            <span>{model}</span> ·{" "}
            <span className="privacy">{disabled ? "🔒 オフ" : "🔒 オプトイン中"}</span>
          </div>
        </div>
        <button className="ai-close" aria-label="閉じる" onClick={() => toggleAi(false)}>
          ✕
        </button>
      </div>

      {disabled ? (
        <div className="ai-sec">
          <p className="muted" style={{ fontSize: 12.5 }}>
            {!aiEnabled
              ? "AI 補助はオフです。設定(⚙︎)でオンにすると、作成中メールの経緯や関連メールを要約します。"
              : "Gemini API キーが未登録です。設定(⚙︎)で登録してください。"}
          </p>
        </div>
      ) : (
        <>
          <div className="ai-sec">
            <h3>この相手との要点</h3>
            {loading ? (
              <p className="muted" style={{ fontSize: 12 }}>
                要点を生成しています…
              </p>
            ) : error ? (
              <p className="cmp-status err" style={{ fontSize: 12 }}>
                {error}
              </p>
            ) : points.length === 0 ? (
              <p className="muted" style={{ fontSize: 12 }}>
                件名や本文を書くと、関連する経緯・約束・未対応を要約します。
              </p>
            ) : (
              <ul className="points">
                {points.map((p, i) => (
                  <li key={i} className={p.category}>
                    <b>{POINT_LABEL[p.category] ?? "メモ"}</b> ： {p.text}
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="ai-sec">
            <h3>関連する過去メール</h3>
            <RelatedMailList mails={relatedMails} />
          </div>
        </>
      )}

      <div className="ai-foot">
        <ReplyDraftButton />
      </div>
    </aside>
  );
}
