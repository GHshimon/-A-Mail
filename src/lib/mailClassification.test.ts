import { describe, expect, it } from "vitest";
import { classifyMail } from "./mailClassification";

const base = { id: 1, uid: 1, date: 0, seen: false, flagged: false, has_attachments: false };

describe("classifyMail", () => {
  it("marks questions as reply candidates", () => {
    const result = classifyMail({ ...base, subject: "日程の件", from: "佐藤", snippet: "ご都合はいかがでしょうか" });
    expect(result.category).toBe("reply_candidate");
    expect(result.tags).toContain("要返信候補");
  });

  it("does not auto-confirm unknown messages", () => {
    const result = classifyMail({ ...base, subject: "連絡", from: "取引先", snippet: "共有です" });
    expect(result.state).toBe("manual_required");
  });
});
