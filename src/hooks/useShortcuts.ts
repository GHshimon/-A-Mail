import { useEffect } from "react";
import { useUiStore } from "@/store/uiStore";
import { useComposeStore } from "@/store/composeStore";
import { useMailStore } from "@/store/mailStore";
import { buildReplyDraft } from "@/lib/replyDraft";

/**
 * キーボードショートカット(フロント側 keydown)。
 *
 * グローバル(⌘N 等)は将来 Tauri accelerator でも受けるが、まずは WebView 内の
 * keydown で配線する。macOS の ⌘ は metaKey。
 *
 *  ⌘N 新規作成 / ⌘R 返信(開いているメール)/ ⌘⇧A AI 開閉
 *  送信(⌘↩)は ComposeWindow 内で処理する。
 */
export function useShortcuts(): void {
  const setMode = useUiStore((s) => s.setMode);
  const toggleAi = useUiStore((s) => s.toggleAi);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      const key = e.key.toLowerCase();
      if (key === "n") {
        e.preventDefault();
        // 新規メール: 現在のアカウントで空の作成を開始。
        const accountId = useUiStore.getState().selectedAccountId;
        useComposeStore.getState().resetDraft({ accountId });
        useComposeStore.getState().setKind("new");
        setMode("compose");
      } else if (key === "r") {
        e.preventDefault();
        // 返信: 開いているメールから返信ドラフトを生成。無ければ空の作成。
        const accountId = useUiStore.getState().selectedAccountId;
        const open = useMailStore.getState().openMessage;
        if (open && accountId != null) {
          useComposeStore.getState().resetDraft(buildReplyDraft(open, accountId));
          useComposeStore.getState().setKind("reply");
        } else {
          useComposeStore.getState().resetDraft({ accountId });
          useComposeStore.getState().setKind("new");
        }
        setMode("compose");
      } else if (e.shiftKey && key === "a") {
        e.preventDefault();
        toggleAi();
      } else if (e.key === ",") {
        e.preventDefault();
        // ⌘, で設定の開閉。
        const cur = useUiStore.getState().mode;
        setMode(cur === "settings" ? "read" : "settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setMode, toggleAi]);
}
