import { useAiStore, type ContextPoint } from "@/store/aiStore";
import { useUiStore } from "@/store/uiStore";
import { RelatedMailList } from "./RelatedMailList";
import { ReplyDraftButton } from "./ReplyDraftButton";

const POINT_LABEL: Record<ContextPoint["category"], string> = {
  history: "経緯",
  commit: "約束",
  todo: "未対応",
};

/**
 * AI 補助ドロワー(B)。作成時に本文エリアの右側へ出現し、関連過去メールの要点と
 * 一覧を表示。フッタに返信ドラフト生成(C)ボタン。
 */
export function AiSidebar() {
  const points = useAiStore((s) => s.points);
  const relatedMails = useAiStore((s) => s.relatedMails);
  const model = "Gemini 2.5 Flash";
  const toggleAi = useUiStore((s) => s.toggleAi);

  return (
    <aside className="ai" aria-label="AI アシスト">
      <div className="ai-head">
        <div>
          <div className="ai-title">
            <span className="spark">✦</span> AI アシスト
          </div>
          <div className="ai-sub">
            <span>{model}</span> ·{" "}
            <span className="privacy">🔒 オプトイン中</span>
          </div>
        </div>
        <button
          className="ai-close"
          aria-label="閉じる"
          onClick={() => toggleAi(false)}
        >
          ✕
        </button>
      </div>

      <div className="ai-sec">
        <h3>この相手との要点</h3>
        <ul className="points">
          {points.map((p, i) => (
            <li key={i} className={p.category}>
              <b>{POINT_LABEL[p.category]}</b> ： {p.text}
            </li>
          ))}
        </ul>
      </div>

      <div className="ai-sec">
        <h3>関連する過去メール</h3>
        <RelatedMailList mails={relatedMails} />
      </div>

      <div className="ai-foot">
        <ReplyDraftButton />
      </div>
    </aside>
  );
}
