import type { MessageHeader } from "@/ipc/types";
import { formatListDate } from "@/lib/formatDate";
import { classifyMail } from "@/lib/mailClassification";

interface MessageRowProps {
  message: MessageHeader;
  selected: boolean;
  onSelect: (id: number) => void;
}

/** メール一覧の 1 行。未読(太字 + ドット)/添付/選択状態を表現。 */
export function MessageRow({ message, selected, onSelect }: MessageRowProps) {
  const classification = classifyMail(message);
  const cls = [
    "row",
    selected ? "sel" : "",
    message.seen ? "" : "unread",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={cls}
      tabIndex={0}
      onClick={() => onSelect(message.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(message.id);
        }
      }}
    >
      <span className="dot" />
      <div>
        <div className="who">
          <span>{message.from}</span>
          <span className="t">{formatListDate(message.date)}</span>
        </div>
        <div className="subj">{message.subject}</div>
        <div className="snip">{message.snippet}</div>
        {classification.tags.length > 0 && (
          <div className="tags">
            {classification.tags.map((tag) => <span className="att" key={tag}>{tag === "添付あり" ? "📎 " : ""}{tag}</span>)}
          </div>
        )}
      </div>
    </div>
  );
}
