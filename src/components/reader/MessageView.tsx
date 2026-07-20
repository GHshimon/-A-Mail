import { useEffect } from "react";
import { useUiStore } from "@/store/uiStore";
import { useMailStore } from "@/store/mailStore";
import { HtmlSandbox } from "./HtmlSandbox";
import { formatListDate } from "@/lib/formatDate";

/**
 * メール本文ビュー(閲覧モード)。
 *
 * メッセージ選択時に get_message で本文を遅延取得し、
 *  - HTML 本文があれば HtmlSandbox(DOMPurify + iframe sandbox)で安全描画、
 *  - なければプレーンテキストを表示する。
 * 取得完了までは一覧のヘッダ情報でプレースホルダを出す。
 */
export function MessageView() {
  const folderId = useUiStore((s) => s.selectedFolderId);
  const messageId = useUiStore((s) => s.selectedMessageId);

  const messagesByFolder = useMailStore((s) => s.messagesByFolder);
  const openMessage = useMailStore((s) => s.openMessage);
  const loadingMessage = useMailStore((s) => s.loadingMessage);
  const loadMessage = useMailStore((s) => s.loadMessage);

  useEffect(() => {
    if (messageId != null) void loadMessage(messageId);
  }, [messageId, loadMessage]);

  // ヘッダはキャッシュから(本文到着前のプレースホルダ用)。
  const cached =
    folderId != null && messageId != null
      ? (messagesByFolder[folderId] ?? []).find((m) => m.id === messageId)
      : undefined;

  if (messageId == null || !cached) {
    return (
      <div className="reader">
        <div className="reader-empty">
          左の一覧からメールを選択すると、ここに本文が表示されます。
        </div>
      </div>
    );
  }

  const full = openMessage && openMessage.header.id === messageId ? openMessage : null;
  const subject = full?.header.subject || cached.subject;
  const from = full?.header.from || cached.from;
  const date = full?.header.date ?? cached.date;

  return (
    <div className="reader">
      <div className="msg-head">
        <h1>{subject || "(件名なし)"}</h1>
        <div className="msg-meta">
          <span>差出人: {from}</span>
          <span>{formatListDate(date)}</span>
        </div>
        {full && full.to.length > 0 && (
          <div className="msg-meta">
            <span>宛先: {full.to.join(", ")}</span>
          </div>
        )}
        {full && full.attachments.length > 0 && (
          <div className="msg-attachments">
            {full.attachments.map((a) => (
              <span className="att-chip" key={a.id}>
                📎 {a.filename}
              </span>
            ))}
          </div>
        )}
      </div>

      {full?.body_html ? (
        <HtmlSandbox html={full.body_html} />
      ) : full?.body_text ? (
        <pre className="msg-body-text">{full.body_text}</pre>
      ) : loadingMessage ? (
        <div className="msg-body-text muted">本文を取得しています…</div>
      ) : (
        <div className="msg-body-text muted">{cached.snippet}</div>
      )}
    </div>
  );
}
