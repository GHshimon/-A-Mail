import { useEffect } from "react";
import { useUiStore, resolveTheme } from "@/store/uiStore";

/**
 * ダーク/ライトのシステム追従。
 *
 * - `prefers-color-scheme` を監視し uiStore.systemDark を更新。
 * - theme(system/light/dark)と systemDark から実効テーマを算出し、
 *   `<html data-theme="...">` に反映(tokens.css がこの属性で色を切替)。
 *
 * Tauri の `window.theme()` / `onThemeChanged` も併用可能だが、WebView の
 * matchMedia が OS 設定に追従するため、まずは Web 標準のみで実装する。
 */
export function useTheme(): void {
  const theme = useUiStore((s) => s.theme);
  const systemDark = useUiStore((s) => s.systemDark);
  const setSystemDark = useUiStore((s) => s.setSystemDark);

  // システムのダーク設定を購読。
  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    setSystemDark(mql.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [setSystemDark]);

  // 実効テーマを DOM へ反映。
  useEffect(() => {
    const effective = resolveTheme(theme, systemDark);
    const root = document.documentElement;
    root.setAttribute("data-theme", effective);
    root.style.colorScheme = effective;
  }, [theme, systemDark]);
}
