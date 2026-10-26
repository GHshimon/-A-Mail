import { useEffect, useState } from "react";
import { assistantDiscardDraft, assistantListDrafts, assistantRestoreDraft } from "@/ipc/assistant";
import type { DraftRecord } from "@/ipc/types";
import { useComposeStore } from "@/store/composeStore";
import { useUiStore } from "@/store/uiStore";

/** Persistent assistant drafts, including recoverable discarded records. */
export function AssistantDraftList() {
  // Hooks must always run in the same order even when a compose account becomes
  // available after this sidebar has mounted.
  const composeAccountId = useComposeStore((s) => s.draft.accountId);
  const selectedAccountId = useUiStore((s) => s.selectedAccountId);
  const accountId = composeAccountId ?? selectedAccountId;
  const resetDraft = useComposeStore((s) => s.resetDraft); const setMode = useUiStore((s) => s.setMode);
  const [records, setRecords] = useState<DraftRecord[]>([]); const [showDiscarded, setShowDiscarded] = useState(false); const [error, setError] = useState<string | null>(null);
  const load = () => { if (accountId != null) void assistantListDrafts({ account_id: accountId, include_discarded: showDiscarded }).then(setRecords).catch((e) => setError(e instanceof Error ? e.message : "下書きを読み込めません")); };
  useEffect(load, [accountId, showDiscarded]);
  const resume = (record: DraftRecord) => { resetDraft({ accountId: record.draft.account_id, to: record.draft.to, cc: record.draft.cc, bcc: record.draft.bcc, subject: record.draft.subject, body: record.draft.body_text, inReplyTo: record.draft.in_reply_to ?? null, references: record.draft.references, draftId: record.draft.id, sessionId: record.session_id, revision: record.revision, sourceMessageId: record.source_message_id, workspaceId: record.workspace_id, intent: record.intent, replyRunId: record.reply_run_id }); setMode("compose"); };
  const transition = async (record: DraftRecord, restore: boolean) => { try { await (restore ? assistantRestoreDraft : assistantDiscardDraft)({ draft_id: record.draft.id, expected_revision: record.revision }); load(); } catch (e) { setError(e instanceof Error ? e.message : "下書きを更新できません"); } };
  return <section className="ai-sec" aria-label="ローカル下書き"><h3>ローカル下書き</h3><p className="muted" style={{ fontSize: 11 }}>IMAPのDraftsとは別の、確認型アシスタントの保存一覧です。</p>{records.length === 0 ? <p className="muted" style={{ fontSize: 12 }}>保存済み下書きはありません。</p> : <ul className="project-sources">{records.map((record) => <li key={record.draft.id}><div className="project-source-head"><span>{record.status}</span><code>{record.draft.subject || "(件名なし)"}</code></div><p>{record.review_reasons[0] || record.draft.body_text.slice(0, 90) || "本文なし"}</p>{record.status === "discarded" ? <button className="iconbtn" onClick={() => void transition(record, true)}>復元</button> : <><button className="iconbtn" onClick={() => resume(record)}>再開</button><button className="iconbtn" disabled={record.status === "sending"} onClick={() => void transition(record, false)}>破棄</button></>}</li>)}</ul>}<label className="muted" style={{ fontSize: 11 }}><input type="checkbox" checked={showDiscarded} onChange={(e) => setShowDiscarded(e.target.checked)} /> 破棄済みも表示</label>{error && <p className="cmp-status err">{error}</p>}</section>;
}
