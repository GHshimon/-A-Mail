import type { MessageHeader } from "@/ipc/types";

export type MailCategory = "reply_candidate" | "notification" | "informational" | "unknown";
export type MailReviewState = "candidate" | "manual_required" | "classified";

export interface MailClassification {
  category: MailCategory;
  state: MailReviewState;
  tags: string[];
  reasons: string[];
}

/** Cheap, deterministic pre-filter. AI agents may refine this result but never send from it. */
export function classifyMail(message: MessageHeader): MailClassification {
  const text = `${message.subject} ${message.from} ${message.snippet}`;
  const tags: string[] = [];
  const reasons: string[] = [];
  const reply = /[?？]|いかが|ご都合|お願いします|確認/.test(text);
  const notification = /github|apple|no-reply|通知|領収書|ci passed/i.test(text);
  if (reply) { tags.push("要返信候補"); reasons.push("質問または確認依頼を含みます"); }
  if (notification) { tags.push("通知候補"); reasons.push("通知・自動配信らしい送信元または件名です"); }
  if (message.has_attachments) tags.push("添付あり");
  const category: MailCategory = reply ? "reply_candidate" : notification ? "notification" : message.seen ? "informational" : "unknown";
  return { category, state: category === "unknown" ? "manual_required" : "candidate", tags, reasons };
}
