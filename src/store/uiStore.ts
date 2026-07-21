import { create } from "zustand";
import type { ThemePref } from "@/ipc/types";

/** 閲覧 / 作成 / 設定 のモード。ReaderArea の表示を切り替える。 */
export type ViewMode = "read" | "compose" | "settings";

interface PaneWidths {
  /** サイドバー(アカウント/フォルダ)幅 px */
  nav: number;
  /** メール一覧幅 px */
  list: number;
}

interface UiState {
  theme: ThemePref;
  /** システム設定が dark か(useTheme が更新)。theme==="system" 時の実効値算出に使う。 */
  systemDark: boolean;

  paneWidths: PaneWidths;
  mode: ViewMode;

  selectedAccountId: number | null;
  selectedFolderId: number | null;
  selectedMessageId: number | null;

  /** AI 補助ドロワーの開閉。 */
  aiOpen: boolean;
  /** 狭幅時にフォルダ/一覧ドロワーを開く。 */
  navOpen: boolean;

  setTheme: (t: ThemePref) => void;
  setSystemDark: (dark: boolean) => void;
  setPaneWidth: (pane: keyof PaneWidths, width: number) => void;
  setMode: (mode: ViewMode) => void;
  selectAccount: (id: number | null) => void;
  selectFolder: (id: number | null) => void;
  selectMessage: (id: number | null) => void;
  toggleAi: (open?: boolean) => void;
  toggleNav: (open?: boolean) => void;
}

const MIN_NAV = 58;
const MAX_NAV = 320;
const MIN_LIST = 240;
const MAX_LIST = 520;

const clamp = (v: number, min: number, max: number) =>
  Math.min(max, Math.max(min, v));

export const useUiStore = create<UiState>((set) => ({
  theme: "system",
  systemDark:
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches === true,

  paneWidths: { nav: 220, list: 300 },
  mode: "read",

  selectedAccountId: null,
  selectedFolderId: null,
  selectedMessageId: null,

  aiOpen: true,
  navOpen: false,

  setTheme: (theme) => set({ theme }),
  setSystemDark: (systemDark) => set({ systemDark }),
  setPaneWidth: (pane, width) =>
    set((s) => ({
      paneWidths: {
        ...s.paneWidths,
        [pane]:
          pane === "nav"
            ? clamp(width, MIN_NAV, MAX_NAV)
            : clamp(width, MIN_LIST, MAX_LIST),
      },
    })),
  setMode: (mode) => set({ mode }),
  selectAccount: (selectedAccountId) => set({ selectedAccountId }),
  selectFolder: (selectedFolderId) =>
    set({ selectedFolderId, selectedMessageId: null }),
  selectMessage: (selectedMessageId) => set({ selectedMessageId }),
  toggleAi: (open) => set((s) => ({ aiOpen: open ?? !s.aiOpen })),
  toggleNav: (open) => set((s) => ({ navOpen: open ?? !s.navOpen })),
}));

/** theme + systemDark から実効テーマ(light/dark)を導出する。 */
export function resolveTheme(theme: ThemePref, systemDark: boolean): "light" | "dark" {
  if (theme === "system") return systemDark ? "dark" : "light";
  return theme;
}
