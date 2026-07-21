import { create } from "zustand";
import type { ContextPoint, MessageHeader } from "@/ipc/types";

interface AiState {
  /** オプトイン(既定 OFF)。設定と同期。 */
  aiEnabled: boolean;
  /** Gemini キー登録済みか。 */
  hasKey: boolean;

  /** A: ゴーストテキスト(予測入力)。空なら非表示。 */
  ghostText: string;
  /** A: 世代 ID。古い補完結果を破棄するために使う。 */
  requestGen: number;
  completing: boolean;

  /** B: 要点。 */
  points: ContextPoint[];
  /** B: 関連する過去メール(FTS5 結果)。 */
  relatedMails: MessageHeader[];
  contextLoading: boolean;
  /** B: 直近の要点取得のエラー(UI 表示用)。 */
  contextError: string | null;

  setAiEnabled: (v: boolean) => void;
  setHasKey: (v: boolean) => void;
  setGhostText: (t: string) => void;
  clearGhost: () => void;
  bumpGen: () => number;
  setCompleting: (v: boolean) => void;
  setContext: (points: ContextPoint[], relatedMails: MessageHeader[]) => void;
  setContextLoading: (v: boolean) => void;
  setContextError: (msg: string | null) => void;
}

export const useAiStore = create<AiState>((set, get) => ({
  aiEnabled: false,
  hasKey: false,

  ghostText: "",
  requestGen: 0,
  completing: false,

  points: [],
  relatedMails: [],
  contextLoading: false,
  contextError: null,

  setAiEnabled: (aiEnabled) => set({ aiEnabled }),
  setHasKey: (hasKey) => set({ hasKey }),
  setGhostText: (ghostText) => set({ ghostText }),
  clearGhost: () => set({ ghostText: "" }),
  bumpGen: () => {
    const next = get().requestGen + 1;
    set({ requestGen: next });
    return next;
  },
  setCompleting: (completing) => set({ completing }),
  setContext: (points, relatedMails) => set({ points, relatedMails }),
  setContextLoading: (contextLoading) => set({ contextLoading }),
  setContextError: (contextError) => set({ contextError }),
}));
