import { create } from "zustand";
import type { MessageHeader } from "@/ipc/types";
import { isTauri } from "@/ipc/client";
import { listMessages, markRead as ipcMarkRead } from "@/ipc/mail";
import { mockMessages } from "@/lib/mockData";

const PAGE = 50;

interface MailState {
  /** folderId -> ヘッダキャッシュ */
  messagesByFolder: Record<number, MessageHeader[]>;
  loadingFolderId: number | null;

  fetchMessages: (folderId: number, reset?: boolean) => Promise<void>;
  /** 既読状態の楽観更新(失敗時ロールバックは M1 で配線)。 */
  setSeenOptimistic: (folderId: number, messageId: number, seen: boolean) => void;
  markRead: (folderId: number, messageId: number, seen: boolean) => Promise<void>;
}

export const useMailStore = create<MailState>((set, get) => ({
  messagesByFolder: {},
  loadingFolderId: null,

  fetchMessages: async (folderId, reset = false) => {
    const existing = get().messagesByFolder[folderId];
    if (existing && !reset) return;

    set({ loadingFolderId: folderId });
    try {
      const offset = reset ? 0 : (existing?.length ?? 0);
      const page = isTauri()
        ? await listMessages(folderId, offset, PAGE)
        : (mockMessages[folderId] ?? []);
      set((s) => ({
        messagesByFolder: {
          ...s.messagesByFolder,
          [folderId]: reset ? page : [...(existing ?? []), ...page],
        },
      }));
    } finally {
      set({ loadingFolderId: null });
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
