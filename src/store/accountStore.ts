import { create } from "zustand";
import type { Account, Folder } from "@/ipc/types";
import { isTauri } from "@/ipc/client";
import {
  listAccounts,
  addAccount as apiAddAccount,
  removeAccount as apiRemoveAccount,
  type AddAccountInput,
} from "@/ipc/accounts";
import { listFolders } from "@/ipc/mail";
import { mockAccounts, mockFolders } from "@/lib/mockData";

interface AccountState {
  accounts: Account[];
  /** accountId -> フォルダ一覧 */
  foldersByAccount: Record<number, Folder[]>;
  loading: boolean;

  loadAccounts: () => Promise<void>;
  loadFolders: (accountId: number) => Promise<void>;
  /** アカウント追加(Rust の add_account へ委譲し、成功後に再読込)。 */
  addAccount: (input: AddAccountInput) => Promise<Account>;
  /** アカウント削除(DB + Keychain)。 */
  removeAccount: (accountId: number) => Promise<void>;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  accounts: [],
  foldersByAccount: {},
  loading: false,

  loadAccounts: async () => {
    set({ loading: true });
    try {
      const accounts = isTauri() ? await listAccounts() : mockAccounts;
      set({ accounts });
      // 全アカウントのフォルダも先読みしておく(サイドバー表示用)。
      await Promise.all(accounts.map((a) => get().loadFolders(a.id)));
    } finally {
      set({ loading: false });
    }
  },

  loadFolders: async (accountId) => {
    const folders = isTauri()
      ? await listFolders(accountId)
      : (mockFolders[accountId] ?? []);
    set((s) => ({
      foldersByAccount: { ...s.foldersByAccount, [accountId]: folders },
    }));
  },

  addAccount: async (input) => {
    const account = await apiAddAccount(input);
    await get().loadAccounts();
    return account;
  },

  removeAccount: async (accountId) => {
    await apiRemoveAccount(accountId);
    set((s) => {
      const foldersByAccount = { ...s.foldersByAccount };
      delete foldersByAccount[accountId];
      return {
        accounts: s.accounts.filter((a) => a.id !== accountId),
        foldersByAccount,
      };
    });
  },
}));
