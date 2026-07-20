import { useEffect, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useUiStore } from "@/store/uiStore";
import { useMailStore } from "@/store/mailStore";
import { MessageRow } from "./MessageRow";

/** 推定行高(px)。可変高でも react-virtual が measureElement で補正。 */
const ESTIMATED_ROW = 84;

/**
 * メッセージ一覧(仮想スクロール)。
 * 選択中フォルダのヘッダを mailStore から取得し、@tanstack/react-virtual で描画。
 */
export function MessageList() {
  const folderId = useUiStore((s) => s.selectedFolderId);
  const selectedMessageId = useUiStore((s) => s.selectedMessageId);
  const selectMessage = useUiStore((s) => s.selectMessage);

  const messagesByFolder = useMailStore((s) => s.messagesByFolder);
  const fetchMessages = useMailStore((s) => s.fetchMessages);

  const messages = folderId != null ? (messagesByFolder[folderId] ?? []) : [];

  useEffect(() => {
    if (folderId != null) void fetchMessages(folderId);
  }, [folderId, fetchMessages]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ESTIMATED_ROW,
    overscan: 8,
  });

  return (
    <section className="maillist" aria-label="メッセージ一覧">
      <div className="list-head">
        <h2>受信トレイ</h2>
        <div className="meta">
          {messages.filter((m) => !m.seen).length} 件の未読 · 最終同期 2 分前
        </div>
      </div>
      <div className="list-scroll" ref={scrollRef}>
        {folderId == null ? (
          <div style={{ padding: 24, color: "var(--ink-3)", fontSize: 13 }}>
            フォルダを選択してください
          </div>
        ) : (
          <div
            style={{
              height: virtualizer.getTotalSize(),
              width: "100%",
              position: "relative",
            }}
          >
            {virtualizer.getVirtualItems().map((vItem) => {
              const m = messages[vItem.index];
              return (
                <div
                  key={m.id}
                  data-index={vItem.index}
                  ref={virtualizer.measureElement}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    transform: `translateY(${vItem.start}px)`,
                  }}
                >
                  <MessageRow
                    message={m}
                    selected={m.id === selectedMessageId}
                    onSelect={selectMessage}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}
