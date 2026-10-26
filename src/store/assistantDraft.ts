import type { DraftRecord } from "@/ipc/types";
import { assistantSaveDraft } from "@/ipc/assistant";
import { toDraftInput, type ComposeDraft } from "./composeStore";

/**
 * Serializes writes for one compose session.  A later edit is never overwritten
 * by an earlier reply; callers keep their own current-session check for UI state.
 */
export class DraftSaveQueue {
  private chain: Promise<void> = Promise.resolve();
  private activeSession = "";
  private latest = new Map<string, { draftId: number | null; revision: number | null }>();
  private lastResult: Promise<DraftRecord | null> = Promise.resolve(null);
  constructor(private readonly onSaved: (record: DraftRecord) => void) {}

  save(draft: ComposeDraft): Promise<DraftRecord | null> {
    const input = toDraftInput(draft);
    if (!input) return Promise.resolve(null);
    const snapshot = { ...draft, to: [...draft.to], cc: [...draft.cc], bcc: [...draft.bcc], references: [...draft.references] };
    const run = async () => {
      // Resolve revision only when this queued request actually starts.  A second
      // edit made while the first save is in flight must use that first response.
      const known = this.latest.get(snapshot.sessionId);
      const draftId = known?.draftId ?? snapshot.draftId;
      const revision = known?.revision ?? snapshot.revision;
      const record = await assistantSaveDraft({ draft: { ...input, id: draftId }, session_id: snapshot.sessionId, expected_revision: revision, source_message_id: snapshot.sourceMessageId, workspace_id: snapshot.workspaceId, intent: snapshot.intent, reply_run_id: snapshot.replyRunId });
      this.latest.set(snapshot.sessionId, { draftId: record.draft.id, revision: record.revision });
      if (this.activeSession === snapshot.sessionId) this.onSaved(record);
      return record;
    };
    this.activeSession = snapshot.sessionId;
    const result = this.chain.then(run, run);
    // Keep scheduling after a rejected write, but never hide its failure from flush.
    this.chain = result.then(() => undefined, () => undefined);
    this.lastResult = result;
    return result;
  }

  flush(): Promise<void> { return this.lastResult.then(() => undefined); }
}
