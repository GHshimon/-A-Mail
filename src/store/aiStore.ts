import { create } from "zustand";

/** B: 関連情報サイドバーの要点 1 項目。 */
export interface ContextPoint {
  category: "history" | "commit" | "todo";
  text: string;
}

/** B: 関連する過去メール 1 件。 */
export interface RelatedMail {
  id: number;
  subject: string;
  date: string;
  snippet: string;
  relevance: "high" | "mid" | "low";
}

interface AiState {
  /** オプトイン(既定 OFF)。設定と同期。 */
  aiEnabled: boolean;
  hasKey: boolean;

  /** A: ゴーストテキスト(予測入力)。空なら非表示。 */
  ghostText: string;
  /** A: 世代 ID。古いストリームのトークンを破棄するために使う。 */
  requestGen: number;
  completing: boolean;

  /** B: 要点。 */
  points: ContextPoint[];
  relatedMails: RelatedMail[];
  contextLoading: boolean;

  setAiEnabled: (v: boolean) => void;
  setHasKey: (v: boolean) => void;
  setGhostText: (t: string) => void;
  clearGhost: () => void;
  bumpGen: () => number;
}

export const useAiStore = create<AiState>((set, get) => ({
  aiEnabled: true,
  hasKey: false,

  ghostText: "4月20日の納品に向けて準備を進めさせていただきます。",
  requestGen: 0,
  completing: false,

  // M0 の見た目確認用モック要点(mock.html と対応)。
  points: [
    { category: "history", text: "4月納品分の請求書を6/28に送付。田中様が社内確認中だった。" },
    { category: "commit", text: "こちらは「納期は先方確認後に確定」と回答済み(6/28)。" },
    { category: "todo", text: "見積_0704.pdf への署名戻しがまだ。次アクションは納品日の確定連絡。" },
  ],
  relatedMails: [
    {
      id: 201,
      subject: "納期についてのご相談",
      date: "6/28",
      snippet: "「先方の確認が取れ次第、正式な納品日をご連絡します」とお伝えしています。",
      relevance: "high",
    },
    {
      id: 202,
      subject: "4月分 請求書送付の件",
      date: "6/28",
      snippet: "請求書PDFを添付して送付。金額 ¥286,000(税込)。",
      relevance: "mid",
    },
    {
      id: 203,
      subject: "お見積りのご送付",
      date: "5/15",
      snippet: "初回見積を提示。納期は「発注後 約3週間」と記載。",
      relevance: "mid",
    },
  ],
  contextLoading: false,

  setAiEnabled: (aiEnabled) => set({ aiEnabled }),
  setHasKey: (hasKey) => set({ hasKey }),
  setGhostText: (ghostText) => set({ ghostText }),
  clearGhost: () => set({ ghostText: "" }),
  bumpGen: () => {
    const next = get().requestGen + 1;
    set({ requestGen: next });
    return next;
  },
}));
