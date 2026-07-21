import { useEffect } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { useTheme } from "@/hooks/useTheme";
import { useShortcuts } from "@/hooks/useShortcuts";
import { useAccountStore } from "@/store/accountStore";
import { useAiStore } from "@/store/aiStore";
import { useUiStore } from "@/store/uiStore";
import { getSettings, hasGeminiKey } from "@/ipc/ai";
import { isTauri } from "@/ipc/client";

export default function App() {
  useTheme();
  useShortcuts();

  const loadAccounts = useAccountStore((s) => s.loadAccounts);
  const setAiEnabled = useAiStore((s) => s.setAiEnabled);
  const setHasKey = useAiStore((s) => s.setHasKey);

  // 起動時: AI のオプトイン状態とキー有無を反映(既定は OFF)。
  useEffect(() => {
    if (!isTauri()) return;
    void (async () => {
      try {
        const s = await getSettings();
        setAiEnabled(s.ai_enabled);
        setHasKey(await hasGeminiKey());
      } catch {
        /* 取得失敗時は既定(OFF)のまま */
      }
    })();
  }, [setAiEnabled, setHasKey]);
  const accounts = useAccountStore((s) => s.accounts);
  const foldersByAccount = useAccountStore((s) => s.foldersByAccount);
  const selectAccount = useUiStore((s) => s.selectAccount);
  const selectFolder = useUiStore((s) => s.selectFolder);
  const selectedFolderId = useUiStore((s) => s.selectedFolderId);

  // 起動時: アカウント/フォルダを読み込む。
  useEffect(() => {
    void loadAccounts();
  }, [loadAccounts]);

  // フォルダが揃った時点で、既定フォルダ(受信トレイ)を一度だけ自動選択する。
  // folders は実機では IMAP 経由で非同期に届くため foldersByAccount を依存に含める。
  // 既にユーザーが選択済みなら上書きしない。
  useEffect(() => {
    if (selectedFolderId != null) return;
    const first = accounts[0];
    if (!first) return;
    const folders = foldersByAccount[first.id];
    if (!folders || folders.length === 0) return;
    const target = folders.find((f) => f.role === "inbox") ?? folders[0];
    selectAccount(first.id);
    selectFolder(target.id);
  }, [accounts, foldersByAccount, selectedFolderId, selectAccount, selectFolder]);

  return <AppShell />;
}
