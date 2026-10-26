import { describe, expect, it } from "vitest";
import { isAgentRequest } from "./contract";

describe("agent contract", () => {
  it("accepts provider-neutral requests", () => {
    expect(isAgentRequest({ task: "classify_mail", schema_version: "classification_v1", input: {}, allowed_context_ids: [], max_input_chars: 4000 })).toBe(true);
  });
  it("rejects requests without bounded context", () => {
    expect(isAgentRequest({ task: "classify_mail", schema_version: "classification_v1", input: {} })).toBe(false);
  });
  it("rejects a task with a mismatched schema or unsafe limit", () => {
    expect(isAgentRequest({ task: "classify_mail", schema_version: "reply_review_v1", input: {}, allowed_context_ids: [], max_input_chars: 4000 })).toBe(false);
    expect(isAgentRequest({ task: "evaluate_reply", schema_version: "reply_review_v1", input: {}, allowed_context_ids: [], max_input_chars: 20001 })).toBe(false);
  });
});
