import { invoke } from "./client";
import type {
  CompleteReq,
  ContextReq,
  ContextResult,
  AiProject,
  ProjectFileHit,
  ReplyResult,
  ReplyReq,
  Settings,
  SettingsPatch,
  CliJob,
  CliStatus,
  CliTaskKind,
} from "./types";
import { isTauri } from "./client";

// Vite 単体で設定 UI を確認するためのインメモリ状態。実ファイルは一切読まない。
let browserProjects: AiProject[] = [
  {
    id: 1,
    name: "A-mail Demo",
    root_path: "/Volumes/NAS/projects/A-mail",
    enabled: true,
    created_at: 0,
  },
];
let nextBrowserProjectId = 2;

let browserCliJob: CliJob | null = null;
let browserCliPolls = 0;

const BROWSER_CLI_OUTPUT: Record<CliTaskKind, string> = {
  project_brief: [
    "# A-mail プロジェクト概要",
    "",
    "- Tauri + React で構成されたデスクトップメールアプリ",
    "- 通常のメールAIは API、任意のプロジェクト調査は Codex CLI が担当",
    "- CLI は登録済みルートを読み取り専用で調査",
  ].join("\n"),
  spec_gap: [
    "# 仕様差分",
    "",
    "1. README のセットアップ手順と現在の AI プロバイダー設定を同期する",
    "2. CLI 機能の制約とキャンセル動作を利用者向け仕様へ追記する",
  ].join("\n"),
  git_history: [
    "# Git 履歴の要約",
    "",
    "- メール基盤の実装",
    "- API プロバイダー切替の追加",
    "- 読み取り専用プロジェクト文脈の追加",
  ].join("\n"),
  attachment_candidates: [
    "# 添付候補",
    "",
    "- `docs/ai-integration.md` — AI構成の説明資料",
    "- `README.md` — 導入手順の共有用",
    "",
    "候補を提示しただけで、メールへの添付は行っていません。",
  ].join("\n"),
};

// AI(Gemini)関連 + 設定 + Keychain キー管理の型付きラッパ。
// M3 は非ストリーミング generateContent。SSE ストリーミングは将来対応。

/** A: 予測入力。カーソル直前までの本文から続きを返す(無効/キー無なら Ai エラー)。 */
export function aiComplete(req: CompleteReq): Promise<string> {
  return invoke<string>("ai_complete", { req });
}

/** B: 関連情報サイドバー(FTS5 → Gemini 要点化)。 */
export function aiContextSidebar(req: ContextReq): Promise<ContextResult> {
  return invoke<ContextResult>("ai_context_sidebar", { req });
}

/** C: 返信ドラフト生成。 */
export function aiGenerateReply(req: ReplyReq): Promise<ReplyResult> {
  return invoke<ReplyResult>("ai_generate_reply", { req });
}

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function updateSettings(patch: SettingsPatch): Promise<Settings> {
  return invoke<Settings>("update_settings", { patch });
}

/** Gemini API キーを Keychain に保存(DB には保存しない)。 */
export function setGeminiKey(apiKey: string): Promise<void> {
  return invoke<void>("set_gemini_key", { apiKey });
}

/** キー登録有無のみを返す(値そのものはフロントに出さない)。 */
export function hasGeminiKey(): Promise<boolean> {
  return invoke<boolean>("has_gemini_key");
}

export function setOpenAiKey(apiKey: string): Promise<void> {
  return invoke<void>("set_openai_key", { apiKey });
}

export function hasOpenAiKey(): Promise<boolean> {
  return invoke<boolean>("has_openai_key");
}

export function listAiProjects(): Promise<AiProject[]> {
  if (!isTauri()) return Promise.resolve(browserProjects.map((project) => ({ ...project })));
  return invoke<AiProject[]>("list_ai_projects");
}

export function addAiProject(name: string, rootPath: string): Promise<AiProject> {
  if (!isTauri()) {
    const project: AiProject = {
      id: nextBrowserProjectId++,
      name,
      root_path: rootPath,
      enabled: true,
      created_at: Math.floor(Date.now() / 1000),
    };
    browserProjects = [...browserProjects, project];
    return Promise.resolve({ ...project });
  }
  return invoke<AiProject>("add_ai_project", { name, rootPath });
}

export function removeAiProject(projectId: number): Promise<void> {
  if (!isTauri()) {
    browserProjects = browserProjects.filter((project) => project.id !== projectId);
    return Promise.resolve();
  }
  return invoke<void>("remove_ai_project", { projectId });
}

export function searchAiProjectFiles(
  projectIds: number[],
  query: string,
  limit: number,
): Promise<ProjectFileHit[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<ProjectFileHit[]>("search_ai_project_files", { projectIds, query, limit });
}

/** Codex CLI の利用可否。ブラウザプレビューでは安全な固定モックを返す。 */
export function getCliStatus(): Promise<CliStatus> {
  if (!isTauri()) {
    return Promise.resolve({ available: true, version: "codex-cli 0.145.0 (デモ)" });
  }
  return invoke<CliStatus>("get_cli_status");
}

/** 登録済みプロジェクト1件と固定タスクだけを指定して CLI ジョブを開始する。 */
export function startCliJob(projectId: number, taskKind: CliTaskKind): Promise<CliJob> {
  if (!isTauri()) {
    browserCliPolls = 0;
    browserCliJob = {
      id: `browser-cli-${Date.now()}`,
      project_id: projectId,
      task_kind: taskKind,
      status: "running",
      progress: "読み取り専用サンドボックスを準備しています…",
      started_at: Math.floor(Date.now() / 1000),
    };
    return Promise.resolve({ ...browserCliJob });
  }
  return invoke<CliJob>("start_cli_job", {
    req: { project_id: projectId, task_kind: taskKind },
  });
}

export function getCliJob(jobId: string): Promise<CliJob> {
  if (!isTauri()) {
    if (!browserCliJob || browserCliJob.id !== jobId) {
      return Promise.reject(new Error("CLIジョブが見つかりません"));
    }
    if (browserCliJob.status === "running") {
      browserCliPolls += 1;
      if (browserCliPolls >= 3) {
        browserCliJob = {
          ...browserCliJob,
          status: "succeeded",
          progress: "調査が完了しました",
          output: BROWSER_CLI_OUTPUT[browserCliJob.task_kind],
          completed_at: Math.floor(Date.now() / 1000),
        };
      } else if (browserCliPolls >= 1) {
        browserCliJob = { ...browserCliJob, progress: "プロジェクト内を調査しています…" };
      }
    }
    return Promise.resolve({ ...browserCliJob });
  }
  return invoke<CliJob>("get_cli_job", { jobId });
}

export function cancelCliJob(jobId: string): Promise<CliJob> {
  if (!isTauri()) {
    if (!browserCliJob || browserCliJob.id !== jobId) {
      return Promise.reject(new Error("CLIジョブが見つかりません"));
    }
    browserCliJob = {
      ...browserCliJob,
      status: "cancelled",
      progress: "キャンセルしました",
      completed_at: Math.floor(Date.now() / 1000),
    };
    return Promise.resolve({ ...browserCliJob });
  }
  return invoke<CliJob>("cancel_cli_job", { jobId });
}
