import type { MessageHeader } from "@/ipc/types";
import { formatListDate } from "@/lib/formatDate";

interface RelatedMailListProps {
  mails: MessageHeader[];
}

/** B: 関連する過去メール一覧(FTS5 結果を rank 順で併記)。 */
export function RelatedMailList({ mails }: RelatedMailListProps) {
  if (mails.length === 0) {
    return (
      <p className="muted" style={{ fontSize: 12 }}>
        関連する過去メールは見つかりませんでした。
      </p>
    );
  }
  return (
    <div className="related">
      {mails.map((m) => (
        <div className="rel" key={m.id} tabIndex={0}>
          <div className="rt">
            <span>{m.subject || "(件名なし)"}</span>
            <span className="d">{formatListDate(m.date)}</span>
          </div>
          <div className="rs">{m.snippet || m.from}</div>
        </div>
      ))}
    </div>
  );
}
