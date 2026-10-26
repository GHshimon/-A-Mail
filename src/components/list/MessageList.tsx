import { useEffect, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useUiStore } from "@/store/uiStore";
import { useMailStore } from "@/store/mailStore";
import { MessageRow } from "./MessageRow";

/** 推定行高(px)。可変高でも react-virtual が measureElement で補正。 */
const ESTIMATED_ROW = 84;

/** 末尾から何行手前まで見えたら次のページを読むか。 */
const LOAD_MORE_TRIGGER_ROWS = 10;

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
  const loadMore = useMailStore((s) => s.loadMore);
  const hasMoreByFolder = useMailStore((s) => s.hasMoreByFolder);
  const loadingMoreFolderId = useMailStore((s) => s.loadingMoreFolderId);
  const searchQuery = useMailStore((s) => s.searchQuery);
  const searchResults = useMailStore((s) => s.searchResults);
  const searchLoading = useMailStore((s) => s.searchLoading);

  const searching = searchQuery.trim() !== "";
  const hasMore = folderId != null && hasMoreByFolder[folderId] !== false;
  const loadingMore = folderId != null && loadingMoreFolderId === folderId;
  const messages = searching
    ? searchResults
    : folderId != null
      ? (messagesByFolder[folderId] ?? [])
      : [];

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

  // 末尾が見えたら過去方向へ読み足す。検索結果は別系統なので対象外。
  const virtualItems = virtualizer.getVirtualItems();
  const lastVisibleIndex = virtualItems.length
    ? virtualItems[virtualItems.length - 1].index
    : -1;
  useEffect(() => {
    if (searching || folderId == null || !hasMore || loadingMore) return;
    if (messages.length === 0) return;
    if (lastVisibleIndex < messages.length - LOAD_MORE_TRIGGER_ROWS) return;
    void loadMore(folderId);
  }, [
    searching,
    folderId,
    hasMore,
    loadingMore,
    lastVisibleIndex,
    messages.length,
    loadMore,
  ]);

  return (
    <section className="maillist" aria-label="メッセージ一覧">
      <div className="list-head">
        <h2>{searching ? `「${searchQuery}」の検索結果` : "受信トレイ"}</h2>
        <div className="meta">
          {searching
            ? searchLoading ? "検索中…" : `${messages.length} 件`
            : `${messages.filter((m) => !m.seen).length} 件の未読 · 最終同期 2 分前`}
        </div>
      </div>
      <div className="list-scroll" ref={scrollRef}>
        {!searching && folderId == null ? (
          <div style={{ padding: 24, color: "var(--ink-3)", fontSize: 13 }}>
            フォルダを選択してください
          </div>
        ) : messages.length === 0 ? (
          <div style={{ padding: 24, color: "var(--ink-3)", fontSize: 13 }}>
            {searchLoading ? "検索しています…" : "該当するメールはありません"}
          </div>
        ) : (
          <div
            style={{
              height: virtualizer.getTotalSize(),
              width: "100%",
              position: "relative",
            }}
          >
            {virtualItems.map((vItem) => {
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
        {!searching && messages.length > 0 && (loadingMore || !hasMore) && (
          <div className="list-more">
            {loadingMore ? "過去のメールを読み込んでいます…" : "これ以上前のメールはありません"}
          </div>
        )}
      </div>
    </section>
  );
}
