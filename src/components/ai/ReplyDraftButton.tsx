import { useState } from "react";
import { assistantApplyReply, assistantConfirmRecipients, assistantGenerateReply } from "@/ipc/assistant";
import { useComposeStore } from "@/store/composeStore";
import type { ReplyRun } from "@/ipc/types";

type ReviewStatus = "pass" | "needs_review" | "fail";
type ReplyCheck = { id: string; label: string; status: ReviewStatus; detail: string };

function reviewReply(body: string, quotedBody: string | null, to: string[]): ReplyCheck[] {
  const text = body.trim();
  const source = quotedBody ?? "";
  const asksQuestion = /[?？]|でしょうか|いかが|ご都合|お願いします|お願いいたします/.test(source);
  const hasAction = /確認|ご連絡|お願|対応|共有|ご返信|お知らせ/.test(text);
  const riskyFact = /(金額|料金|納期|契約|期限|確約|承諾|返金)/.test(source);
  return [
    { id: "body", label: "本文が入力されている", status: text ? "pass" : "fail", detail: text ? "本文あり" : "本文が空です" },
    { id: "recipient", label: "宛先が設定されている", status: to.length ? "pass" : "fail", detail: to.length ? `${to.length}件` : "宛先がありません" },
    { id: "question", label: "相手の要件に対応している", status: !asksQuestion || hasAction ? "pass" : "needs_review", detail: !asksQuestion ? "明確な質問は検出されませんでした" : hasAction ? "回答・次の行動に関する表現があります" : "質問への回答または次の行動が不明です" },
    { id: "facts", label: "重要事項に根拠がある", status: riskyFact ? "needs_review" : "pass", detail: riskyFact ? "金額・納期・契約などは本文だけでは根拠を確認できません" : "要確認の重要事項はありません" },
    { id: "unsupported", label: "未確認の約束がない", status: /必ず|確実に|絶対に/.test(text) ? "needs_review" : "pass", detail: /必ず|確実に|絶対に/.test(text) ? "強い断定表現があります" : "強い断定表現はありません" },
  ];
}

function ReplyReviewPanel({ body, quotedBody, to }: { body: string; quotedBody: string | null; to: string[] }) {
  const checks = reviewReply(body, quotedBody, to);
  const hasIssue = checks.some((check) => check.status !== "pass");
  return <section className="reply-evidence" aria-label="返信内容の適性評価">
    <b>返信内容のチェック</b>
    <ul>{checks.map((check) => <li key={check.id}><strong>{check.status === "pass" ? "✓" : check.status === "fail" ? "!" : "?"} {check.label}</strong><span> — {check.detail}</span></li>)}</ul>
    <p className={hasIssue ? "cmp-status" : "cmp-status ok"}>{hasIssue ? "送信前に確認が必要です" : "基本チェックを通過しました"}</p>
    <small className="muted">ローカルチェックです。内容の正確性や送信可否を保証するものではありません。</small>
  </section>;
}

/** Generated candidates stay separate from human text until an explicit apply. */
export function ReplyDraftButton() {
  const draft = useComposeStore((s) => s.draft); const setBody = useComposeStore((s) => s.setBody); const setAssistantMeta = useComposeStore((s) => s.setAssistantMeta); const setStatus = useComposeStore((s) => s.setStatus);
  const [run, setRun] = useState<ReplyRun | null>(null); const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null); const [recipientsConfirmed, setRecipientsConfirmed] = useState(false);
  const generate = async () => { if (draft.sourceMessageId == null) { setError("新規メールはP0の返信生成対象外です。手動で作成・保存・送信できます。"); return; } if (draft.draftId == null || draft.revision == null) { setError("先に下書きを保存してください。"); return; } if (draft.workspaceId != null && !recipientsConfirmed) { setError("顧客情報を使う前に、現在の宛先を確認してください。"); return; } setBusy(true); setError(null); try { const confirmed = draft.workspaceId == null ? { draft: { id: draft.draftId }, revision: draft.revision } : await assistantConfirmRecipients({ draft_id: draft.draftId, expected_revision: draft.revision }); const result = await assistantGenerateReply({ draft_id: confirmed.draft.id, expected_revision: confirmed.revision, force: Boolean(draft.body.trim()) }); const latest = useComposeStore.getState().draft; if (latest.sessionId !== draft.sessionId) return; setAssistantMeta({ revision: result.draft.revision, replyRunId: result.run.id }); setRun(result.run); if (result.applied && !latest.body.trim()) setBody(result.run.generated_text); } catch (e) { setError(e instanceof Error ? e.message : "生成に失敗しました"); } finally { setBusy(false); } };
  const apply = async () => { if (!run || draft.draftId == null || draft.revision == null) return; setBusy(true); try { const result = await assistantApplyReply({ draft_id: draft.draftId, run_id: run.id, expected_revision: draft.revision }); const latest = useComposeStore.getState().draft; if (latest.sessionId !== draft.sessionId) return; setBody(result.draft.body_text); setAssistantMeta({ revision: result.revision, replyRunId: result.reply_run_id }); setRun(null); setStatus({ phase: "saved", at: Date.now() }); } catch (e) { setError(e instanceof Error ? e.message : "候補を適用できませんでした"); } finally { setBusy(false); } };
  return <>{draft.workspaceId != null && <label className="muted" style={{ display: "block", fontSize: 11, marginTop: 6 }}><input type="checkbox" checked={recipientsConfirmed} disabled={busy || draft.draftId == null || draft.sourceMessageId == null} onChange={(event) => setRecipientsConfirmed(event.target.checked)} /> To・Cc・Bccを確認し、この顧客情報を使う</label>}<button className="ai-generate" disabled={busy || (draft.workspaceId != null && !recipientsConfirmed) || draft.draftId == null || draft.sourceMessageId == null} onClick={() => void generate()}>{busy ? "生成中…" : "✦ 返信案を生成"}</button>{(draft.body.trim() || run) && <ReplyReviewPanel body={run?.generated_text ?? draft.body} quotedBody={draft.quotedBody} to={draft.to} />}{run && <div className="ai-preview" aria-label="返信案の候補">
    <div className="ai-preview-title">生成候補（本文は未変更）</div>
    <div className="ai-preview-body">{run.generated_text}</div>
    {run.review_reasons.length > 0 && <section className="reply-evidence warning"><b>送信前の確認</b><ul>{run.review_reasons.map((reason, index) => <li key={index}>{reason}</li>)}</ul></section>}
    {run.questions.length > 0 && <section className="reply-evidence"><b>未回答・確認事項</b><ul>{run.questions.map((question) => <li key={question.id}>{question.text} <code>{question.status}</code></li>)}</ul></section>}
    <section className="reply-evidence"><b>使用した情報源</b>{run.sources.length === 0 ? <p className="muted">顧客固有の情報源なし</p> : <ul>{run.sources.map((source) => <li key={source.id}><span>{source.label}</span>{source.locator && <code> {source.locator}</code>} <small>{source.fact_status}</small><p>{source.snippet}</p></li>)}</ul>}</section>
    <p className="muted reply-run-meta">{run.provider} / {run.model} · 入力約{run.estimated_input_tokens} tokens{run.truncated ? " · 上限により省略あり" : ""}</p>
    <button className="primary" disabled={busy} onClick={() => void apply()}>確認して本文に反映</button><button className="iconbtn" onClick={() => setRun(null)}>候補を破棄</button>
  </div>}{error && <p className="cmp-status err">{error}</p>}<p className="muted" style={{ fontSize: 11, marginTop: 6 }}>{draft.workspaceId == null ? "共通資料だけで案を作ります。顧客固有情報は使いません。" : "生成案は人が選ぶまで既存本文を上書きしません。"}</p></>;
}
