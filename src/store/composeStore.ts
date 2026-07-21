import { create } from "zustand";
import type { DraftInput, OutgoingMessage } from "@/ipc/types";

/** 文体トーン(C: 返信ドラフト生成のパラメータ)。 */
export type Tone = "casual" | "standard" | "formal";
/** 長さ。 */
export type Length = "short" | "mid" | "long";

export interface ComposeDraft {
  accountId: number | null;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body: string;
  inReplyTo: string | null;
  references: string[];
  /** DB 保存済み下書き ID(オートセーブ後に設定)。 */
  draftId: number | null;
}

/** 送信/保存の進行状態(ツールバーのフィードバック表示用)。 */
export type ComposeStatus =
  | { phase: "idle" }
  | { phase: "saving" }
  | { phase: "saved"; at: number }
  | { phase: "sending" }
  | { phase: "sent" }
  | { phase: "error"; message: string };

interface ComposeState {
  draft: ComposeDraft;
  tone: Tone;
  length: Length;
  /** 直近ユーザー操作の種別(ラベル表示用)。 */
  kind: "new" | "reply" | "reply-all" | "forward";
  status: ComposeStatus;

  setBody: (body: string) => void;
  setSubject: (subject: string) => void;
  setAccountId: (id: number | null) => void;
  setTo: (to: string[]) => void;
  setCc: (cc: string[]) => void;
  setBcc: (bcc: string[]) => void;
  setDraftId: (id: number | null) => void;
  setTone: (tone: Tone) => void;
  setLength: (length: Length) => void;
  setKind: (kind: ComposeState["kind"]) => void;
  setStatus: (status: ComposeStatus) => void;
  resetDraft: (partial?: Partial<ComposeDraft>) => void;
}

const emptyDraft: ComposeDraft = {
  accountId: null,
  to: [],
  cc: [],
  bcc: [],
  subject: "",
  body: "",
  inReplyTo: null,
  references: [],
  draftId: null,
};

export const useComposeStore = create<ComposeState>((set) => ({
  // 初期は空の新規作成。返信/転送は resetDraft(...) で内容を差し込む。
  draft: { ...emptyDraft },
  tone: "standard",
  length: "mid",
  kind: "new",
  status: { phase: "idle" },

  setBody: (body) => set((s) => ({ draft: { ...s.draft, body } })),
  setSubject: (subject) => set((s) => ({ draft: { ...s.draft, subject } })),
  setAccountId: (accountId) => set((s) => ({ draft: { ...s.draft, accountId } })),
  setTo: (to) => set((s) => ({ draft: { ...s.draft, to } })),
  setCc: (cc) => set((s) => ({ draft: { ...s.draft, cc } })),
  setBcc: (bcc) => set((s) => ({ draft: { ...s.draft, bcc } })),
  setDraftId: (draftId) => set((s) => ({ draft: { ...s.draft, draftId } })),
  setTone: (tone) => set({ tone }),
  setLength: (length) => set({ length }),
  setKind: (kind) => set({ kind }),
  setStatus: (status) => set({ status }),
  resetDraft: (partial) =>
    // 配列は毎回新しい参照にする。emptyDraft の配列を使い回すと ComposeWindow の
    // [draft.to] 依存 effect が発火せず、宛先ローカル state が消えない
    // (送信後に宛先が残り、余分なオートセーブが走る)不具合を防ぐ。
    set({
      draft: { ...emptyDraft, to: [], cc: [], bcc: [], references: [], ...partial },
      status: { phase: "idle" },
    }),
}));

// ---- ComposeDraft ⇄ IPC ペイロード変換 ----

/** 送信ペイロード(OutgoingMessage)へ変換。account_id が無ければ null を返す。 */
export function toOutgoing(draft: ComposeDraft): OutgoingMessage | null {
  if (draft.accountId == null) return null;
  return {
    account_id: draft.accountId,
    to: draft.to,
    cc: draft.cc,
    bcc: draft.bcc,
    subject: draft.subject,
    body_text: draft.body,
    body_html: null,
    in_reply_to: draft.inReplyTo,
    references: draft.references,
    draft_id: draft.draftId,
  };
}

/** 下書き保存ペイロード(DraftInput)へ変換。account_id が無ければ null。 */
export function toDraftInput(draft: ComposeDraft): DraftInput | null {
  if (draft.accountId == null) return null;
  return {
    id: draft.draftId,
    account_id: draft.accountId,
    to: draft.to,
    cc: draft.cc,
    bcc: draft.bcc,
    subject: draft.subject,
    body_text: draft.body,
    body_html: null,
    in_reply_to: draft.inReplyTo,
    references: draft.references,
  };
}

/** 下書きとして保存する価値がある内容か(空同然ならオートセーブしない)。 */
export function hasContent(draft: ComposeDraft): boolean {
  return (
    draft.to.length > 0 ||
    draft.cc.length > 0 ||
    draft.subject.trim() !== "" ||
    draft.body.trim() !== ""
  );
}
