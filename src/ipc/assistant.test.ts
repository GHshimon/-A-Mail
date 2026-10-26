import { describe, expect, it } from "vitest";
import { assistantApplyReply, assistantCancelChatGptLogin, assistantGenerateReply, assistantGetSubscriptionStatus, assistantSaveDraft, assistantStartChatGptLogin } from "./assistant";

describe("browser-only assistant demo", () => {
  it("keeps the generated reply separate until explicit apply", async () => {
    const saved = await assistantSaveDraft({
      draft: {
        account_id: 99,
        to: ["recipient@example.test"],
        cc: [],
        bcc: [],
        subject: "【テストメール】返信確認",
        body_text: "",
        body_html: null,
        in_reply_to: "<fictional@example.test>",
        references: [],
      },
      session_id: `test-${Date.now()}-${Math.random()}`,
      expected_revision: null,
      source_message_id: 999,
      workspace_id: null,
      intent: "",
      reply_run_id: null,
    });
    const generated = await assistantGenerateReply({
      draft_id: saved.draft.id,
      expected_revision: saved.revision,
    });
    expect(generated.applied).toBe(false);
    expect(generated.draft.draft.body_text).toBe("");
    expect(generated.run.generated_text).not.toBe("");

    const applied = await assistantApplyReply({
      draft_id: saved.draft.id,
      run_id: generated.run.id,
      expected_revision: generated.draft.revision,
    });
    expect(applied.draft.body_text).toBe(generated.run.generated_text);
  });

  it("keeps the fictional ChatGPT login cancellable and external-call free", async () => {
    const request_id = `fictional-${Date.now()}`;
    const prompt = await assistantStartChatGptLogin({ request_id });
    expect(prompt.auth_url).toContain("example.invalid");
    expect((await assistantGetSubscriptionStatus()).connection).toBe("connecting");
    await assistantCancelChatGptLogin({ request_id, login_id: prompt.login_id });
    expect((await assistantGetSubscriptionStatus()).connection).toBe("disconnected");
  });
});
