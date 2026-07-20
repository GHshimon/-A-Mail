import type { MessageHeader } from "@/ipc/types";
import { formatListDate } from "@/lib/formatDate";

interface MessageRowProps {
  message: MessageHeader;
  selected: boolean;
  onSelect: (id: number) => void;
}

/** メール一覧の 1 行。未読(太字 + ドット)/添付/選択状態を表現。 */
export function MessageRow({ message, selected, onSelect }: MessageRowProps) {
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
        {message.has_attachments && (
          <div className="tags">
            <span className="att">📎 添付あり</span>
          </div>
        )}
      </div>
    </div>
  );
}
