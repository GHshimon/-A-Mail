import { create } from "zustand";

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

interface ComposeState {
  draft: ComposeDraft;
  tone: Tone;
  length: Length;
  /** 直近ユーザー操作の種別(ラベル表示用)。 */
  kind: "new" | "reply" | "reply-all" | "forward";

  setBody: (body: string) => void;
  setSubject: (subject: string) => void;
  setTone: (tone: Tone) => void;
  setLength: (length: Length) => void;
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
  // M0 の見た目確認用に、モックの返信下書きを初期値として置く。
  draft: {
    ...emptyDraft,
    accountId: 1,
    to: ["田中 亮 <tanaka@yamate-corp.jp>"],
    subject: "Re: 4月納品分の請求書について",
    body:
      "田中様\n\nお世話になっております。ドウチです。\n" +
      "ご連絡ありがとうございます。納期の件、社内でご確認いただけたとのこと、承知いたしました。それでは予定通り",
    inReplyTo: "<msg-6001@yamate-corp.jp>",
  },
  tone: "standard",
  length: "mid",
  kind: "reply",

  setBody: (body) => set((s) => ({ draft: { ...s.draft, body } })),
  setSubject: (subject) => set((s) => ({ draft: { ...s.draft, subject } })),
  setTone: (tone) => set({ tone }),
  setLength: (length) => set({ length }),
  resetDraft: (partial) =>
    set({ draft: { ...emptyDraft, ...partial } }),
}));
