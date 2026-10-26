import { useEffect, useRef, useState } from "react";
import {
  assistantBindContext,
  assistantCreateWorkspace,
  assistantGetContext,
  assistantListProjectScopes,
  assistantListWorkspaces,
  assistantSetProjectScope,
} from "@/ipc/assistant";
import type { ProjectScope, Workspace } from "@/ipc/types";
import { useAiStore } from "@/store/aiStore";
import { useComposeStore } from "@/store/composeStore";

const scopeKey = (projectId: number) => String(projectId);

/** Human-visible customer boundary. Nothing here infers a workspace from an address. */
export function WorkspaceScopePanel() {
  const draft = useComposeStore((state) => state.draft);
  const setAssistantMeta = useComposeStore((state) => state.setAssistantMeta);
  const projects = useAiStore((state) => state.projects).filter((project) => project.enabled);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [scopes, setScopes] = useState<Record<string, ProjectScope>>({});
  const [customerName, setCustomerName] = useState("");
  const [workspaceName, setWorkspaceName] = useState("");
  const [contextStatus, setContextStatus] = useState<string>("unassigned");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const request = useRef(0);

  useEffect(() => {
    if (draft.accountId == null) return;
    const generation = ++request.current;
    setError(null);
    const context = draft.sourceMessageId == null
      ? Promise.resolve(null)
      : assistantGetContext({ source_message_id: draft.sourceMessageId });
    void Promise.all([
      assistantListWorkspaces({ account_id: draft.accountId }),
      assistantListProjectScopes({ account_id: draft.accountId }),
      context,
    ]).then(([nextWorkspaces, nextScopes, nextContext]) => {
      if (request.current !== generation) return;
      setWorkspaces(nextWorkspaces);
      setScopes(Object.fromEntries(nextScopes.map((scope) => [scopeKey(scope.project_id), scope])));
      if (nextContext) {
        setContextStatus(nextContext.status);
        setAssistantMeta({ workspaceId: nextContext.workspace_id });
      } else {
        setContextStatus(draft.workspaceId == null ? "common_only" : "confirmed");
      }
    }).catch((cause) => {
      if (request.current === generation) setError(cause instanceof Error ? cause.message : "顧客コンテキストを読み込めませんでした");
    });
  }, [draft.accountId, draft.sourceMessageId, setAssistantMeta]);

  if (draft.accountId == null) return null;

  const selectWorkspace = async (raw: string) => {
    const workspaceId = raw === "" ? null : Number(raw);
    setBusy(true);
    setError(null);
    try {
      if (draft.sourceMessageId != null) {
        const context = await assistantBindContext({ source_message_id: draft.sourceMessageId, workspace_id: workspaceId });
        setContextStatus(context.status);
        setAssistantMeta({ workspaceId: context.workspace_id, replyRunId: null });
      } else {
        setContextStatus(workspaceId == null ? "common_only" : "confirmed");
        setAssistantMeta({ workspaceId, replyRunId: null });
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "案件を設定できませんでした");
    } finally {
      setBusy(false);
    }
  };

  const createWorkspace = async () => {
    if (!customerName.trim() || !workspaceName.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const created = await assistantCreateWorkspace({
        account_id: draft.accountId!,
        customer_name: customerName.trim(),
        name: workspaceName.trim(),
      });
      setWorkspaces((current) => [created, ...current]);
      setCustomerName("");
      setWorkspaceName("");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "顧客・案件を作成できませんでした");
    } finally {
      setBusy(false);
    }
  };

  const updateScope = async (projectId: number, scope: ProjectScope["scope"], confirmed: boolean) => {
    const workspaceId = scope === "workspace" ? draft.workspaceId : null;
    if (scope === "workspace" && workspaceId == null) {
      setError("先に顧客・案件を選択してください");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const saved = await assistantSetProjectScope({
        account_id: draft.accountId!, project_id: projectId, workspace_id: workspaceId, scope, confirmed,
      });
      setScopes((current) => ({ ...current, [scopeKey(projectId)]: saved }));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "資料の利用範囲を保存できませんでした");
    } finally {
      setBusy(false);
    }
  };

  return <details className="ai-sec" open>
    <summary>顧客コンテキスト</summary>
    <label className="scope-field">案件境界
      <select disabled={busy} value={draft.workspaceId ?? ""} onChange={(event) => void selectWorkspace(event.target.value)}>
        <option value="">共通資料のみ（顧客情報なし）</option>
        {workspaces.map((workspace) => <option key={workspace.id} value={workspace.id}>{workspace.customer_name} / {workspace.name}</option>)}
      </select>
    </label>
    <p className="muted scope-status">状態: <code>{contextStatus}</code>。メールアドレスや件名から自動確定しません。</p>
    <details className="scope-create">
      <summary>顧客・案件を追加</summary>
      <input value={customerName} placeholder="顧客名" onChange={(event) => setCustomerName(event.target.value)} />
      <input value={workspaceName} placeholder="案件名" onChange={(event) => setWorkspaceName(event.target.value)} />
      <button className="iconbtn" disabled={busy || !customerName.trim() || !workspaceName.trim()} onClick={() => void createWorkspace()}>追加（自動選択しない）</button>
    </details>
    {projects.length > 0 && <div className="scope-projects">
      <h3>資料ごとの利用範囲</h3>
      <p className="muted">許可を確定した資料だけを返信生成に使います。</p>
      {projects.map((project) => {
        const saved = scopes[scopeKey(project.id)] ?? { account_id: draft.accountId!, project_id: project.id, workspace_id: null, scope: "disabled", confirmed: false };
        const belongsElsewhere = saved.scope === "workspace" && saved.workspace_id !== draft.workspaceId;
        return <div className="scope-project" key={project.id}>
          <span title={project.root_path}>{project.name}</span>
          <select disabled={busy} value={belongsElsewhere ? "disabled" : saved.scope} onChange={(event) => void updateScope(project.id, event.target.value as ProjectScope["scope"], false)}>
            <option value="disabled">使わない</option>
            <option value="common">全顧客共通</option>
            <option value="workspace" disabled={draft.workspaceId == null}>選択中の案件だけ</option>
          </select>
          <label><input type="checkbox" disabled={busy || saved.scope === "disabled" || belongsElsewhere} checked={saved.confirmed && !belongsElsewhere} onChange={(event) => void updateScope(project.id, saved.scope, event.target.checked)} /> 利用を確認</label>
          {belongsElsewhere && <small className="cmp-status err">別案件に割当済み</small>}
        </div>;
      })}
    </div>}
    {error && <p className="cmp-status err" role="alert">{error}</p>}
  </details>;
}
