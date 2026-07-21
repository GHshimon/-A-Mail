import type { MessageFull } from "@/ipc/types";
import type { ComposeDraft } from "@/store/composeStore";

/** 件名に接頭辞が無ければ付与(既にあれば二重化しない)。 */
function withPrefix(subject: string, prefix: "Re:" | "Fwd:"): string {
  const s = subject.trim();
  const re = prefix === "Re:" ? /^re:/i : /^fwd?:/i;
  return re.test(s) ? s : `${prefix} ${s}`;
}

/** 本文を引用(各行に "> ")する。 */
function quoteBody(full: MessageFull): string {
  const text = full.body_text ?? "";
  return text
    .split("\n")
    .map((l) => `> ${l}`)
    .join("\n");
}

/**
 * 返信ドラフトを組み立てる。
 *
 * 注意: `MessageFull.header.from` は表示名 or アドレス(M1 の取得仕様)。
 * アドレス(`@` を含む)のときだけ宛先へ入れ、そうでなければ空にしてユーザーに委ねる。
 */
export function buildReplyDraft(full: MessageFull, accountId: number): Partial<ComposeDraft> {
  const from = full.header.from;
  const refs = [...full.references];
  if (full.message_id) refs.push(full.message_id);

  const header = `\n\n${new Date(full.header.date * 1000).toLocaleString()} ${from} は書きました:\n`;
  return {
    accountId,
    to: from.includes("@") ? [from] : [],
    cc: [],
    bcc: [],
    subject: withPrefix(full.header.subject, "Re:"),
    body: header + quoteBody(full),
    inReplyTo: full.message_id || null,
    references: refs,
    draftId: null,
  };
}

/** 転送ドラフトを組み立てる(In-Reply-To/References は付けない)。 */
export function buildForwardDraft(full: MessageFull, accountId: number): Partial<ComposeDraft> {
  const sep = "---------- 転送メッセージ ----------";
  const meta =
    `From: ${full.header.from}\n` +
    `Date: ${new Date(full.header.date * 1000).toLocaleString()}\n` +
    `Subject: ${full.header.subject}\n` +
    (full.to.length > 0 ? `To: ${full.to.join(", ")}\n` : "");
  return {
    accountId,
    to: [],
    cc: [],
    bcc: [],
    subject: withPrefix(full.header.subject, "Fwd:"),
    body: `\n\n${sep}\n${meta}\n${full.body_text ?? ""}`,
    inReplyTo: null,
    references: [],
    draftId: null,
  };
}
