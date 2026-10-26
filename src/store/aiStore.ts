import { create } from "zustand";
import type {
  AiProject,
  AiProvider,
  ContextPoint,
  MessageHeader,
  ProjectFileHit,
} from "@/ipc/types";
import { addAiProject, listAiProjects, removeAiProject } from "@/ipc/ai";

interface AiState {
  /** オプトイン(既定 OFF)。設定と同期。 */
  aiEnabled: boolean;
  /** Gemini キー登録済みか。 */
  hasKey: boolean;
  provider: AiProvider;
  model: string;

  /** Phase 2: 読み取り専用 AI 文脈として登録されたプロジェクト。 */
  projects: AiProject[];
  selectedProjectIds: number[];
  projectSources: ProjectFileHit[];
  projectsLoading: boolean;
  projectsError: string | null;

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
  contextRequestGen: number;

  setAiEnabled: (v: boolean) => void;
  setHasKey: (v: boolean) => void;
  setProvider: (provider: AiProvider, model: string) => void;
  loadProjects: () => Promise<void>;
  addProject: (name: string, rootPath: string) => Promise<AiProject>;
  removeProject: (projectId: number) => Promise<void>;
  toggleProject: (projectId: number) => void;
  setGhostText: (t: string) => void;
  clearGhost: () => void;
  bumpGen: () => number;
  setCompleting: (v: boolean) => void;
  setContext: (
    points: ContextPoint[],
    relatedMails: MessageHeader[],
    projectSources: ProjectFileHit[],
  ) => void;
  setContextLoading: (v: boolean) => void;
  setContextError: (msg: string | null) => void;
  bumpContextGen: () => number;
}

export const useAiStore = create<AiState>((set, get) => ({
  aiEnabled: false,
  hasKey: false,
  provider: "gemini",
  model: "gemini-flash-latest",

  projects: [],
  selectedProjectIds: [],
  projectSources: [],
  projectsLoading: false,
  projectsError: null,

  ghostText: "",
  requestGen: 0,
  completing: false,

  points: [],
  relatedMails: [],
  contextLoading: false,
  contextError: null,
  contextRequestGen: 0,

  setAiEnabled: (aiEnabled) => set({ aiEnabled }),
  setHasKey: (hasKey) => set({ hasKey }),
  setProvider: (provider, model) => set({ provider, model }),
  loadProjects: async () => {
    set({ projectsLoading: true, projectsError: null });
    try {
      const projects = await listAiProjects();
      const enabledIds = projects.filter((project) => project.enabled).map((project) => project.id);
      const previous = get().selectedProjectIds.filter((id) => enabledIds.includes(id));
      set({
        projects,
        // 外部ファイルの送信範囲は、登録済みでもユーザーが明示選択するまで空。
        selectedProjectIds: previous,
        projectsLoading: false,
      });
    } catch (err) {
      set({
        projectsLoading: false,
        projectsError: err instanceof Error ? err.message : "プロジェクトを取得できませんでした",
      });
    }
  },
  addProject: async (name, rootPath) => {
    set({ projectsLoading: true, projectsError: null });
    try {
      const project = await addAiProject(name, rootPath);
      set((state) => ({
        projects: [...state.projects, project],
        selectedProjectIds: state.selectedProjectIds,
        projectsLoading: false,
      }));
      return project;
    } catch (err) {
      const message = err instanceof Error ? err.message : "プロジェクトを登録できませんでした";
      set({ projectsLoading: false, projectsError: message });
      throw err;
    }
  },
  removeProject: async (projectId) => {
    set({ projectsLoading: true, projectsError: null });
    try {
      await removeAiProject(projectId);
      set((state) => ({
        projects: state.projects.filter((project) => project.id !== projectId),
        selectedProjectIds: state.selectedProjectIds.filter((id) => id !== projectId),
        projectSources: state.projectSources.filter((source) => source.project_id !== projectId),
        projectsLoading: false,
      }));
    } catch (err) {
      const message = err instanceof Error ? err.message : "プロジェクトを解除できませんでした";
      set({ projectsLoading: false, projectsError: message });
      throw err;
    }
  },
  toggleProject: (projectId) => set((state) => ({
    selectedProjectIds: state.selectedProjectIds.includes(projectId)
      ? state.selectedProjectIds.filter((id) => id !== projectId)
      : [...state.selectedProjectIds, projectId],
  })),
  setGhostText: (ghostText) => set({ ghostText }),
  clearGhost: () => set({ ghostText: "" }),
  bumpGen: () => {
    const next = get().requestGen + 1;
    set({ requestGen: next });
    return next;
  },
  setCompleting: (completing) => set({ completing }),
  setContext: (points, relatedMails, projectSources) =>
    set({ points, relatedMails, projectSources }),
  setContextLoading: (contextLoading) => set({ contextLoading }),
  setContextError: (contextError) => set({ contextError }),
  bumpContextGen: () => {
    const next = get().contextRequestGen + 1;
    set({ contextRequestGen: next });
    return next;
  },
}));
