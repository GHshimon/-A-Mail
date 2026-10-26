import { describe, expect, it, vi } from "vitest";
import type { ComposeDraft } from "./composeStore";

const { save } = vi.hoisted(() => ({ save: vi.fn() }));
vi.mock("@/ipc/assistant", () => ({ assistantSaveDraft: save }));

import { DraftSaveQueue } from "./assistantDraft";

const base = (body: string, revision: number | null): ComposeDraft => ({
  accountId: 1, to: ["demo@example.test"], cc: [], bcc: [], subject: "件名", body,
  inReplyTo: null, references: [], sourceMessageId: 1, draftId: 7, sessionId: "session-a",
  revision, workspaceId: null, intent: "", replyRunId: null, quotedBody: null,
});

describe("DraftSaveQueue", () => {
  it("serializes a newer edit using the preceding saved revision", async () => {
    save.mockReset()
      .mockResolvedValueOnce({ draft: { id: 7 }, session_id: "session-a", revision: 4, reply_run_id: null })
      .mockResolvedValueOnce({ draft: { id: 7 }, session_id: "session-a", revision: 5, reply_run_id: null });
    const queue = new DraftSaveQueue(() => undefined);
    await Promise.all([queue.save(base("first", 3)), queue.save(base("second", 3))]);
    expect(save.mock.calls[0][0].expected_revision).toBe(3);
    expect(save.mock.calls[1][0].expected_revision).toBe(4);
    expect(save.mock.calls[1][0].draft.body_text).toBe("second");
  });

  it("propagates the latest save failure through flush", async () => {
    save.mockReset().mockRejectedValueOnce(new Error("save failed"));
    const queue = new DraftSaveQueue(() => undefined);
    await expect(queue.save(base("body", 3))).rejects.toThrow("save failed");
    await expect(queue.flush()).rejects.toThrow("save failed");
  });

  it("does not let an old session response mutate the current compose session", async () => {
    const applied: string[] = [];
    let resolveOld!: (value: unknown) => void;
    save.mockReset()
      .mockImplementationOnce(() => new Promise((resolve) => { resolveOld = resolve; }))
      .mockResolvedValueOnce({ draft: { id: 8 }, session_id: "session-b", revision: 1, reply_run_id: null });
    const queue = new DraftSaveQueue((record) => applied.push(record.session_id));
    const pending = queue.save(base("old", 3));
    await Promise.resolve();
    const next = queue.save({ ...base("new", null), draftId: null, sessionId: "session-b" });
    resolveOld({ draft: { id: 7 }, session_id: "session-a", revision: 4, reply_run_id: null });
    await Promise.all([pending, next]);
    expect(applied).toEqual(["session-b"]);
    // The old response cannot replace the new session's metadata or body.
    expect(save.mock.calls[0][0].draft.body_text).toBe("old");
  });
});
