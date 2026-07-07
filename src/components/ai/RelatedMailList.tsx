import type { RelatedMail } from "@/store/aiStore";

const RELEVANCE_LABEL: Record<RelatedMail["relevance"], string> = {
  high: "◇ 関連度 高",
  mid: "◇ 関連度 中",
  low: "◇ 関連度 低",
};

interface RelatedMailListProps {
  mails: RelatedMail[];
}

/** B: 関連する過去メール一覧(FTS5 結果を併記)。 */
export function RelatedMailList({ mails }: RelatedMailListProps) {
  return (
    <div className="related">
      {mails.map((m) => (
        <div className="rel" key={m.id} tabIndex={0}>
          <div className="rt">
            <span>{m.subject}</span>
            <span className="d">{m.date}</span>
          </div>
          <div className="rs">{m.snippet}</div>
          <div className="score">{RELEVANCE_LABEL[m.relevance]}</div>
        </div>
      ))}
    </div>
  );
}
