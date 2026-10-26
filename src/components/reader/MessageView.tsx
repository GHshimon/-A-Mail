import { useEffect, useState } from "react";
import type { MessageHeader } from "@/ipc/types";
import { useUiStore } from "@/store/uiStore";
import { useMailStore } from "@/store/mailStore";
import { useComposeStore } from "@/store/composeStore";
import { buildForwardDraft, buildReplyDraft } from "@/lib/replyDraft";
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
  const accountId = useUiStore((s) => s.selectedAccountId);
  const setMode = useUiStore((s) => s.setMode);

  const messagesByFolder = useMailStore((s) => s.messagesByFolder);
  const searchResults = useMailStore((s) => s.searchResults);
  const openMessage = useMailStore((s) => s.openMessage);
  const loadingMessage = useMailStore((s) => s.loadingMessage);
  const loadMessage = useMailStore((s) => s.loadMessage);

  // 検索結果を開いた直後に検索を解除しても、本文取得までヘッダを保持する。
  // accountId もキーに含め、アカウント切替後に別アカウントの情報を残さない。
  const [preservedSearchHeader, setPreservedSearchHeader] = useState<{
    accountId: number | null;
    messageId: number;
    header: MessageHeader;
  } | null>(null);

  const resetDraft = useComposeStore((s) => s.resetDraft);
  const setKind = useComposeStore((s) => s.setKind);

  useEffect(() => {
    if (messageId != null) void loadMessage(messageId);
  }, [messageId, loadMessage]);

  // ヘッダはキャッシュから(本文到着前のプレースホルダ用)。
  const cached =
    folderId != null && messageId != null
      ? (messagesByFolder[folderId] ?? []).find((m) => m.id === messageId)
      : undefined;
  const searchHeader = searchResults.find((m) => m.id === messageId);
  useEffect(() => {
    if (messageId != null && searchHeader) {
      setPreservedSearchHeader({ accountId, messageId, header: searchHeader });
    }
  }, [accountId, messageId, searchHeader]);

  const full = openMessage && openMessage.header.id === messageId ? openMessage : null;
  const preserved =
    preservedSearchHeader?.accountId === accountId &&
    preservedSearchHeader.messageId === messageId
      ? preservedSearchHeader.header
      : undefined;
  const header = cached ?? searchHeader ?? full?.header ?? preserved;

  if (messageId == null || !header) {
    return (
      <div className="reader">
        <div className="reader-empty">
          左の一覧からメールを選択すると、ここに本文が表示されます。
        </div>
      </div>
    );
  }

  const subject = full?.header.subject || header.subject;
  const from = full?.header.from || header.from;
  const date = full?.header.date ?? header.date;

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

        {full && (
          <div className="msg-actions">
            <button
              className="iconbtn"
              disabled={accountId == null}
              onClick={() => {
                if (accountId == null) return;
                resetDraft(buildReplyDraft(full, accountId));
                setKind("reply");
                setMode("compose");
              }}
            >
              ↩︎ 返信
            </button>
            <button
              className="iconbtn"
              disabled={accountId == null}
              onClick={() => {
                if (accountId == null) return;
                resetDraft(buildForwardDraft(full, accountId));
                setKind("forward");
                setMode("compose");
              }}
            >
              ➡︎ 転送
            </button>
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
        <div className="msg-body-text muted">{header.snippet}</div>
      )}
    </div>
  );
}
