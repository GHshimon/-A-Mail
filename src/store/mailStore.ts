import { create } from "zustand";
import type { MessageFull, MessageHeader } from "@/ipc/types";
import { isTauri } from "@/ipc/client";
import {
  listMessages,
  syncFolder as apiSyncFolder,
  getMessage as apiGetMessage,
  markRead as ipcMarkRead,
  searchMessages as apiSearchMessages,
  loadMoreMessages as apiLoadMoreMessages,
} from "@/ipc/mail";
import { mockMessageFull, mockMessages } from "@/lib/mockData";

const PAGE = 50;

interface MailState {
  /** folderId -> ヘッダキャッシュ */
  messagesByFolder: Record<number, MessageHeader[]>;
  /** folderId -> まだ過去へ遡れるか(初回取得・読み足しの結果で更新)。 */
  hasMoreByFolder: Record<number, boolean>;
  loadingFolderId: number | null;
  /** 読み足し中のフォルダ(二重発火の抑止も兼ねる)。 */
  loadingMoreFolderId: number | null;
  searchQuery: string;
  searchResults: MessageHeader[];
  searchLoading: boolean;
  searchGeneration: number;

  /** 現在開いているメッセージの本文(get_message の結果)。 */
  openMessageId: number | null;
  openMessage: MessageFull | null;
  loadingMessage: boolean;

  fetchMessages: (folderId: number, reset?: boolean) => Promise<void>;
  /** 一覧を過去方向へ 1 ページ読み足す(無限スクロール)。 */
  loadMore: (folderId: number) => Promise<void>;
  /** メッセージ本文を取得して開く(未取得なら IMAP から遅延取得)。 */
  loadMessage: (messageId: number) => Promise<void>;
  /** 既読状態の楽観更新(失敗時ロールバックは M1 で配線)。 */
  setSeenOptimistic: (folderId: number, messageId: number, seen: boolean) => void;
  markRead: (folderId: number, messageId: number, seen: boolean) => Promise<void>;
  search: (accountId: number | null, query: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  clearSearch: () => void;
}

export const useMailStore = create<MailState>((set, get) => ({
  messagesByFolder: {},
  hasMoreByFolder: {},
  loadingFolderId: null,
  loadingMoreFolderId: null,
  searchQuery: "",
  searchResults: [],
  searchLoading: false,
  searchGeneration: 0,
  openMessageId: null,
  openMessage: null,
  loadingMessage: false,

  fetchMessages: async (folderId, reset = false) => {
    const existing = get().messagesByFolder[folderId];
    if (existing && !reset) return;

    set({ loadingFolderId: folderId });
    try {
      let page: MessageHeader[];
      if (isTauri()) {
        // 選択時に IMAP 同期して最新ヘッダを取得。失敗時は DB キャッシュへフォールバック。
        try {
          page = await apiSyncFolder(folderId, PAGE);
        } catch {
          page = await listMessages(folderId, 0, PAGE);
        }
      } else {
        page = mockMessages[folderId] ?? [];
      }
      set((s) => ({
        messagesByFolder: { ...s.messagesByFolder, [folderId]: page },
        // 1 ページ埋まったなら、まだ先がある前提で読み足しを許す。
        // 実際に残っているかは load_more_messages が EXISTS で判定する。
        hasMoreByFolder: {
          ...s.hasMoreByFolder,
          [folderId]: isTauri() && page.length >= PAGE,
        },
      }));
    } finally {
      set({ loadingFolderId: null });
    }
  },

  loadMore: async (folderId) => {
    const st = get();
    if (st.loadingMoreFolderId != null || st.loadingFolderId != null) return;
    if (st.hasMoreByFolder[folderId] === false) return;
    if (!isTauri()) return;

    const current = st.messagesByFolder[folderId] ?? [];
    set({ loadingMoreFolderId: folderId });
    try {
      const page = await apiLoadMoreMessages(folderId, current.length, PAGE);
      set((s) => {
        const existing = s.messagesByFolder[folderId] ?? [];
        const seen = new Set(existing.map((m) => m.id));
        const added = page.messages.filter((m) => !seen.has(m.id));
        return {
          messagesByFolder: {
            ...s.messagesByFolder,
            [folderId]: [...existing, ...added],
          },
          // 新しい行が 1 件も増えなかったら、それ以上引っ張っても同じなので止める。
          hasMoreByFolder: {
            ...s.hasMoreByFolder,
            [folderId]: page.has_more && added.length > 0,
          },
        };
      });
    } catch {
      // 取得失敗時は今ある分を保ったまま、読み足しを止める(無限リトライを避ける)。
      set((s) => ({
        hasMoreByFolder: { ...s.hasMoreByFolder, [folderId]: false },
      }));
    } finally {
      set({ loadingMoreFolderId: null });
    }
  },

  loadMessage: async (messageId) => {
    set({ openMessageId: messageId, openMessage: null, loadingMessage: true });
    try {
      if (isTauri()) {
        const full = await apiGetMessage(messageId);
        // 取得中に別メッセージへ切り替わっていたら破棄(レース対策)。
        if (get().openMessageId === messageId) set({ openMessage: full });
      } else {
        const full = mockMessageFull(messageId);
        if (full && get().openMessageId === messageId) set({ openMessage: full });
      }
    } catch {
      /* 本文取得失敗時はヘッダのみ表示にフォールバック */
    } finally {
      if (get().openMessageId === messageId) set({ loadingMessage: false });
    }
  },

  setSeenOptimistic: (folderId, messageId, seen) =>
    set((s) => {
      const list = s.messagesByFolder[folderId];
      if (!list) return s;
      return {
        messagesByFolder: {
          ...s.messagesByFolder,
          [folderId]: list.map((m) =>
            m.id === messageId ? { ...m, seen } : m,
          ),
        },
      };
    }),

  markRead: async (folderId, messageId, seen) => {
    get().setSeenOptimistic(folderId, messageId, seen);
    if (isTauri()) {
      try {
        await ipcMarkRead(messageId, seen);
      } catch (err) {
        // ロールバック
        get().setSeenOptimistic(folderId, messageId, !seen);
        throw err;
      }
    }
  },

  search: async (accountId, query) => {
    const normalized = query.trim();
    const generation = get().searchGeneration + 1;
    set({ searchQuery: query, searchGeneration: generation });
    if (!normalized || (accountId == null && isTauri())) {
      set({ searchResults: [], searchLoading: false });
      return;
    }
    set({ searchLoading: true });
    try {
      const results = isTauri()
        ? await apiSearchMessages(accountId as number, normalized, 100)
        : Object.values(mockMessages)
            .flat()
            .filter((m) =>
              `${m.subject} ${m.from} ${m.snippet}`.toLowerCase().includes(normalized.toLowerCase()),
            );
      if (get().searchGeneration === generation) set({ searchResults: results });
    } catch {
      if (get().searchGeneration === generation) set({ searchResults: [] });
    } finally {
      if (get().searchGeneration === generation) set({ searchLoading: false });
    }
  },

  setSearchQuery: (searchQuery) => set({ searchQuery }),

  clearSearch: () =>
    set((s) => ({
      searchQuery: "",
      searchResults: [],
      searchLoading: false,
      searchGeneration: s.searchGeneration + 1,
    })),
}));
