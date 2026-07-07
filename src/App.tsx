import { useEffect } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { useTheme } from "@/hooks/useTheme";
import { useShortcuts } from "@/hooks/useShortcuts";
import { useAccountStore } from "@/store/accountStore";
import { useUiStore } from "@/store/uiStore";

export default function App() {
  useTheme();
  useShortcuts();

  const loadAccounts = useAccountStore((s) => s.loadAccounts);
  const accounts = useAccountStore((s) => s.accounts);
  const selectAccount = useUiStore((s) => s.selectAccount);
  const selectFolder = useUiStore((s) => s.selectFolder);

  // 起動時: アカウント/フォルダを読み込み、既定の受信トレイを選択。
  useEffect(() => {
    void loadAccounts();
  }, [loadAccounts]);

  useEffect(() => {
    if (accounts.length === 0) return;
    const first = accounts[0];
    const inbox = useAccountStore
      .getState()
      .foldersByAccount[first.id]?.find((f) => f.role === "inbox");
    selectAccount(first.id);
    if (inbox) selectFolder(inbox.id);
  }, [accounts, selectAccount, selectFolder]);

  return <AppShell />;
}
