import { useUiStore } from "@/store/uiStore";
import { useMailStore } from "@/store/mailStore";
import { formatListDate } from "@/lib/formatDate";

/**
 * メール本文ビュー(閲覧モード)。
 *
 * M0 ではヘッダ情報のみ表示(mockData は本文を持たない)。M1 で `get_message` に
 * よる本文遅延取得を配線し、HTML 本文があれば HtmlSandbox で描画する。
 */
export function MessageView() {
  const folderId = useUiStore((s) => s.selectedFolderId);
  const messageId = useUiStore((s) => s.selectedMessageId);
  const messagesByFolder = useMailStore((s) => s.messagesByFolder);

  const msg =
    folderId != null && messageId != null
      ? (messagesByFolder[folderId] ?? []).find((m) => m.id === messageId)
      : undefined;

  if (!msg) {
    return (
      <div className="reader">
        <div className="reader-empty">
          左の一覧からメールを選択すると、ここに本文が表示されます。
        </div>
      </div>
    );
  }

  return (
    <div className="reader">
      <div className="msg-head">
        <h1>{msg.subject}</h1>
        <div className="msg-meta">
          <span>差出人: {msg.from}</span>
          <span>{formatListDate(msg.date)}</span>
        </div>
      </div>
      <div className="msg-body">
        {msg.snippet}
        {"\n\n"}
        <span style={{ color: "var(--ink-3)", fontSize: 12 }}>
          (M1 で本文の遅延取得を実装します)
        </span>
      </div>
    </div>
  );
}
