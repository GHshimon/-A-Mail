import { create } from "zustand";
import type { MessageFull, MessageHeader } from "@/ipc/types";
import { isTauri } from "@/ipc/client";
import {
  listMessages,
  syncFolder as apiSyncFolder,
  getMessage as apiGetMessage,
  markRead as ipcMarkRead,
} from "@/ipc/mail";
import { mockMessages } from "@/lib/mockData";

const PAGE = 50;

interface MailState {
  /** folderId -> ヘッダキャッシュ */
  messagesByFolder: Record<number, MessageHeader[]>;
  loadingFolderId: number | null;

  /** 現在開いているメッセージの本文(get_message の結果)。 */
  openMessageId: number | null;
  openMessage: MessageFull | null;
  loadingMessage: boolean;

  fetchMessages: (folderId: number, reset?: boolean) => Promise<void>;
  /** メッセージ本文を取得して開く(未取得なら IMAP から遅延取得)。 */
  loadMessage: (messageId: number) => Promise<void>;
  /** 既読状態の楽観更新(失敗時ロールバックは M1 で配線)。 */
  setSeenOptimistic: (folderId: number, messageId: number, seen: boolean) => void;
  markRead: (folderId: number, messageId: number, seen: boolean) => Promise<void>;
}

export const useMailStore = create<MailState>((set, get) => ({
  messagesByFolder: {},
  loadingFolderId: null,
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
      }));
    } finally {
      set({ loadingFolderId: null });
    }
  },

  loadMessage: async (messageId) => {
    set({ openMessageId: messageId, openMessage: null, loadingMessage: true });
    try {
      if (isTauri()) {
        const full = await apiGetMessage(messageId);
        // 取得中に別メッセージへ切り替わっていたら破棄(レース対策)。
        if (get().openMessageId === messageId) set({ openMessage: full });
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
}));
