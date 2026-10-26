import { useEffect, useRef } from "react";
import { useAiStore } from "@/store/aiStore";
import { useComposeStore } from "@/store/composeStore";
import { useUiStore } from "@/store/uiStore";
import { aiContextSidebar } from "@/ipc/ai";
import { IpcError } from "@/ipc/client";
import { RelatedMailList } from "./RelatedMailList";
import { ReplyDraftButton } from "./ReplyDraftButton";
import { CliTools } from "./CliTools";
import { AssistantDraftList } from "./AssistantDraftList";
import { AssistantSettingsPanel } from "./AssistantSettingsPanel";
import { WorkspaceScopePanel } from "./WorkspaceScopePanel";

const POINT_LABEL: Record<string, string> = {
  history: "経緯",
  commit: "約束",
  todo: "未対応",
};

const CONTEXT_DEBOUNCE_MS = 1200;

/**
 * AI 補助ドロワー(B)。作成時に本文エリアの右側へ出現し、関連過去メールの要点と
 * 一覧を表示。フッタに返信ドラフト生成(C)ボタン。
 *
 * ai_enabled かつキー登録済みのとき、件名/本文の変化(1.2s デバウンス)で
 * ai_context_sidebar を呼び、要点と関連メールを更新する。
 */
export function AiSidebar() {
  const points = useAiStore((s) => s.points);
  const relatedMails = useAiStore((s) => s.relatedMails);
  const loading = useAiStore((s) => s.contextLoading);
  const error = useAiStore((s) => s.contextError);
  const aiEnabled = useAiStore((s) => s.aiEnabled);
  const hasKey = useAiStore((s) => s.hasKey);
  const setContext = useAiStore((s) => s.setContext);
  const setContextLoading = useAiStore((s) => s.setContextLoading);
  const setContextError = useAiStore((s) => s.setContextError);
  const bumpContextGen = useAiStore((s) => s.bumpContextGen);
  const projects = useAiStore((s) => s.projects);
  const selectedProjectIds = useAiStore((s) => s.selectedProjectIds);
  const projectSources = useAiStore((s) => s.projectSources);
  const toggleProject = useAiStore((s) => s.toggleProject);

  const draft = useComposeStore((s) => s.draft);

  const toggleAi = useUiStore((s) => s.toggleAi);
  const provider = useAiStore((s) => s.provider);
  const model = useAiStore((s) => s.model);

  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (!aiEnabled || !hasKey) {
      setContext([], [], []);
      setContextLoading(false);
      return;
    }
    const { accountId, to, subject, body } = draft;
    if (accountId == null) return;
    if (subject.trim() === "" && body.trim() === "") {
      setContext([], [], []);
      setContextError(null);
      setContextLoading(false);
      return;
    }

    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      const generation = bumpContextGen();
      setContextLoading(true);
      setContextError(null);
      try {
        const res = await aiContextSidebar({
          account_id: accountId,
          to,
          subject,
          body,
          // Reply generation uses only Rust-validated workspace scopes. The
          // legacy ad-hoc picker remains available for new-message research.
          project_ids: draft.sourceMessageId == null ? selectedProjectIds : [],
        });
        if (useAiStore.getState().contextRequestGen !== generation) return;
        setContext(res.points, res.related, res.project_sources);
      } catch (err) {
        if (useAiStore.getState().contextRequestGen !== generation) return;
        setContextError(
          err instanceof IpcError ? err.message || err.kind : "取得に失敗しました",
        );
      } finally {
        if (useAiStore.getState().contextRequestGen === generation) {
          setContextLoading(false);
        }
      }
    }, CONTEXT_DEBOUNCE_MS);

    return () => {
      if (timer.current) clearTimeout(timer.current);
      bumpContextGen();
    };
  }, [
    draft,
    aiEnabled,
    hasKey,
    selectedProjectIds,
    bumpContextGen,
    setContext,
    setContextError,
    setContextLoading,
  ]);

  const disabled = !aiEnabled || !hasKey;

  return (
    <aside className="ai" aria-label="AI アシスト">
      <div className="ai-head">
        <div>
          <div className="ai-title">
            <span className="spark">✦</span> AI アシスト
          </div>
          <div className="ai-sub">
            <span>{provider === "openai" ? "OpenAI" : "Gemini"} · {model}</span> ·{" "}
            <span className="privacy">{disabled ? "🔒 オフ" : "🔒 オプトイン中"}</span>
          </div>
        </div>
        <button className="ai-close" aria-label="閉じる" onClick={() => toggleAi(false)}>
          ✕
        </button>
      </div>

      {projects.length > 0 && draft.sourceMessageId == null && (
        <div className="ai-sec project-context-picker">
          <h3>一時的な補助検索（新規メールのみ）</h3>
          <div className="project-chips" role="group" aria-label="AIが参照するプロジェクト">
            {projects.filter((project) => project.enabled).map((project) => {
              const selected = selectedProjectIds.includes(project.id);
              return (
                <label key={project.id} className={`project-chip${selected ? " selected" : ""}`}>
                  <input
                    type="checkbox"
                    checked={selected}
                    onChange={() => toggleProject(project.id)}
                  />
                  <span>{project.name}</span>
                </label>
              );
            })}
          </div>
          <p className="project-readonly">
            🔒 読み取り専用 · これは顧客境界の許可ではありません。返信では下の「顧客コンテキスト」を使います。
          </p>
        </div>
      )}

      <WorkspaceScopePanel />
      <AssistantDraftList />
      <AssistantSettingsPanel />

      <CliTools />

      {disabled ? (
        <div className="ai-sec">
          <p className="muted" style={{ fontSize: 12.5 }}>
            {!aiEnabled
              ? "AI 補助はオフです。設定(⚙︎)でオンにすると、作成中メールの経緯や関連メールを要約します。"
              : `${provider === "openai" ? "OpenAI" : "Gemini"} API キーが未登録です。設定(⚙︎)で登録してください。`}
          </p>
        </div>
      ) : (
        <>
          <div className="ai-sec">
            <h3>この相手との要点</h3>
            {loading ? (
              <p className="muted" style={{ fontSize: 12 }}>
                要点を生成しています…
              </p>
            ) : error ? (
              <p className="cmp-status err" style={{ fontSize: 12 }}>
                {error}
              </p>
            ) : points.length === 0 ? (
              <p className="muted" style={{ fontSize: 12 }}>
                件名や本文を書くと、関連する経緯・約束・未対応を要約します。
              </p>
            ) : (
              <ul className="points">
                {points.map((p, i) => (
                  <li key={i} className={p.category}>
                    <b>{POINT_LABEL[p.category] ?? "メモ"}</b> ： {p.text}
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="ai-sec">
            <h3>関連する過去メール</h3>
            <RelatedMailList mails={relatedMails} />
          </div>

          {draft.sourceMessageId == null && selectedProjectIds.length > 0 && (
            <div className="ai-sec">
              <h3>プロジェクトの出典</h3>
              {loading ? (
                <p className="muted" style={{ fontSize: 12 }}>ファイルを検索しています…</p>
              ) : projectSources.length === 0 ? (
                <p className="muted" style={{ fontSize: 12 }}>
                  今回の内容に対応するファイルは見つかりませんでした。
                </p>
              ) : (
                <ul className="project-sources">
                  {projectSources.map((source, index) => (
                    <li key={`${source.project_id}:${source.relative_path}:${index}`}>
                      <div className="project-source-head">
                        <span>{source.project_name}</span>
                        <code title={source.relative_path}>
                          {source.relative_path}{source.line_number != null ? `:${source.line_number}` : ""}
                        </code>
                      </div>
                      <p>{source.snippet}</p>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </>
      )}

      <div className="ai-foot">
        <ReplyDraftButton />
      </div>
    </aside>
  );
}
