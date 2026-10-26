import { useEffect, useMemo, useRef, useState } from "react";
import { cancelCliJob, getCliJob, getCliStatus, startCliJob } from "@/ipc/ai";
import type { CliJob, CliStatus, CliTaskKind } from "@/ipc/types";
import { useAiStore } from "@/store/aiStore";

const TASKS: { kind: CliTaskKind; label: string; hint: string; icon: string }[] = [
  { kind: "project_brief", label: "プロジェクト概要", hint: "構成と役割を短く整理", icon: "⌘" },
  { kind: "spec_gap", label: "仕様差分", hint: "文書と実装のずれを調査", icon: "≠" },
  { kind: "git_history", label: "Git 履歴", hint: "変更の流れを要約", icon: "⑂" },
  { kind: "attachment_candidates", label: "添付候補", hint: "関連ファイル候補を提示", icon: "⌁" },
];

const TERMINAL = new Set(["succeeded", "failed", "cancelled"]);

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "CLI 操作に失敗しました";
}

/**
 * API機能から独立した手動CLIツール。登録済みルート1件と固定タスクだけを渡し、
 * 結果を表示する。結果から添付・送信・ファイル変更は行わない。
 */
export function CliTools() {
  const projects = useAiStore((s) => s.projects).filter((project) => project.enabled);
  const contextSelection = useAiStore((s) => s.selectedProjectIds);
  const [open, setOpen] = useState(false);
  const [projectId, setProjectId] = useState<number | "">("");
  const [taskKind, setTaskKind] = useState<CliTaskKind>("project_brief");
  const [cliStatus, setCliStatus] = useState<CliStatus | null>(null);
  const [job, setJob] = useState<CliJob | null>(null);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const activeJobId = useRef<string | null>(null);

  // API文脈で明示選択がちょうど1件なら候補として引き継ぐ。複数件は勝手に絞らない。
  useEffect(() => {
    if (projectId !== "") return;
    const enabledSelected = contextSelection.filter((id) => projects.some((p) => p.id === id));
    if (enabledSelected.length === 1) setProjectId(enabledSelected[0]);
  }, [contextSelection, projectId, projects]);

  useEffect(() => {
    let active = true;
    void getCliStatus()
      .then((status) => { if (active) setCliStatus(status); })
      .catch((err) => {
        if (active) setCliStatus({ available: false, message: toErrorMessage(err) });
      });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!job || TERMINAL.has(job.status)) return;
    const expectedJobId = job.id;
    activeJobId.current = expectedJobId;
    let active = true;
    const timer = window.setInterval(() => {
      void getCliJob(expectedJobId)
        .then((next) => {
          if (!active || activeJobId.current !== expectedJobId) return;
          setJob((current) => current?.id === expectedJobId ? next : current);
          if (next.status === "failed") setError(next.error || "CLIジョブに失敗しました");
        })
        .catch((err) => {
          if (active && activeJobId.current === expectedJobId) setError(toErrorMessage(err));
        });
    }, 700);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [job?.id, job?.status]);

  const running = starting || job?.status === "queued" || job?.status === "running";
  const selectedProject = useMemo(
    () => projects.find((project) => project.id === projectId),
    [projectId, projects],
  );
  const selectionHint = contextSelection.length > 1 && projectId === ""
    ? "API文脈では複数選択中です。CLIで調べる1件を選んでください。"
    : projects.length === 0
      ? "設定でプロジェクトを登録すると利用できます。"
      : "CLIで調べる登録済みプロジェクトを1件選択してください。";

  const run = async () => {
    if (projectId === "" || !cliStatus?.available || running) return;
    setError(null);
    setCopied(false);
    setStarting(true);
    try {
      const next = await startCliJob(projectId, taskKind);
      activeJobId.current = next.id;
      setJob(next);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setStarting(false);
    }
  };

  const cancel = async () => {
    if (!job || !running) return;
    try {
      const next = await cancelCliJob(job.id);
      if (activeJobId.current === job.id) setJob(next);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  };

  const copyOutput = async () => {
    if (!job?.output) return;
    try {
      await navigator.clipboard.writeText(job.output);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      setError("結果をコピーできませんでした");
    }
  };

  return (
    <section className="cli-tools" aria-labelledby="cli-tools-title">
      <button className="cli-tools-toggle" type="button" onClick={() => setOpen((v) => !v)} aria-expanded={open}>
        <span>
          <span className="cli-mark" aria-hidden="true">›_</span>
          <strong id="cli-tools-title">Codex CLI ツール</strong>
        </span>
        <span className={`cli-chevron${open ? " open" : ""}`} aria-hidden="true">⌄</span>
      </button>

      {open && (
        <div className="cli-tools-body">
          <div className={`cli-availability ${cliStatus?.available ? "ready" : "unavailable"}`}>
            <span className="cli-status-dot" />
            {cliStatus == null
              ? "CLIを確認中…"
              : cliStatus.available
                ? `利用可能${cliStatus.version ? ` · ${cliStatus.version}` : ""}`
                : cliStatus.message || "Codex CLI が見つかりません"}
          </div>

          <label className="cli-field">
            <span>対象プロジェクト（1件）</span>
            <select
              value={projectId}
              onChange={(event) => {
                setProjectId(event.target.value === "" ? "" : Number(event.target.value));
                activeJobId.current = null;
                setJob(null);
              }}
              disabled={running || projects.length === 0}
            >
              <option value="">選択してください</option>
              {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
            </select>
          </label>
          {projectId === "" && <p className="cli-guidance">{selectionHint}</p>}
          {selectedProject && <code className="cli-project-path" title={selectedProject.root_path}>{selectedProject.root_path}</code>}

          <div className="cli-task-grid" role="radiogroup" aria-label="CLIタスク">
            {TASKS.map((task) => (
              <button
                key={task.kind}
                type="button"
                role="radio"
                aria-checked={taskKind === task.kind}
                className={`cli-task${taskKind === task.kind ? " selected" : ""}`}
                onClick={() => { setTaskKind(task.kind); activeJobId.current = null; setJob(null); }}
                disabled={running}
              >
                <span className="cli-task-icon" aria-hidden="true">{task.icon}</span>
                <span><strong>{task.label}</strong><small>{task.hint}</small></span>
              </button>
            ))}
          </div>

          <div className="cli-safety">
            <strong>🔒 手動実行 · 読み取り専用 · 自動添付／自動送信なし</strong>
            <span>選択プロジェクトを作業対象にしますが、端末上の厳密な読み取り隔離ではありません。</span>
          </div>

          {running ? (
            <div className="cli-running" role="status">
              <div className="cli-progress"><span /></div>
              <div><strong>{job?.progress || "Codex CLI を実行しています…"}</strong><small>この画面を閉じてもメールは送信されません</small></div>
              <button type="button" disabled={!job} onClick={() => void cancel()}>キャンセル</button>
            </div>
          ) : (
            <button className="cli-run" type="button" disabled={projectId === "" || !cliStatus?.available} onClick={() => void run()}>
              読み取り専用で調査を開始
            </button>
          )}

          {error && <p className="cmp-status err" role="alert">{error}</p>}
          {job?.status === "cancelled" && <p className="cli-result-note cancelled" role="status">ジョブをキャンセルしました。ファイルやメールは変更されていません。</p>}
          {job?.status === "succeeded" && job.output && (
            <div className="cli-result">
              <div className="cli-result-head">
                <span>調査結果</span>
                <button type="button" onClick={() => void copyOutput()}>{copied ? "コピー済み" : "コピー"}</button>
              </div>
              <pre>{job.output}</pre>
              {job.task_kind === "attachment_candidates" && (
                <p>候補の確認だけです。添付する場合は作成画面で手動選択してください。</p>
              )}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
