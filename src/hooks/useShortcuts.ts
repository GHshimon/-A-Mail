import { useEffect } from "react";
import { useUiStore } from "@/store/uiStore";

/**
 * キーボードショートカット(フロント側 keydown)。
 *
 * グローバル(⌘N 等)は将来 Tauri accelerator でも受けるが、M0 では WebView 内の
 * keydown で最小限を配線する。macOS の ⌘ は metaKey。
 *
 *  ⌘N 新規作成 / ⌘R 返信(モック) / ⌘⇧A AI 開閉 / ⌘, 設定(未実装)
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
        setMode("compose");
      } else if (key === "r") {
        e.preventDefault();
        setMode("compose");
      } else if (e.shiftKey && key === "a") {
        e.preventDefault();
        toggleAi();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setMode, toggleAi]);
}
