/**
 * C: 返信ドラフト生成ボタン。
 *
 * M0 は見た目のみ。M3 で `ai_generate_reply` を呼び、結果を composeStore.body へ
 * 挿入する(ユーザー編集前提)。
 */
export function ReplyDraftButton() {
  return (
    <button
      className="ai-generate"
      onClick={() => {
        /* TODO(M3): ai_generate_reply を呼んで本文へ挿入 */
      }}
    >
      ✦ 返信ドラフトを生成
    </button>
  );
}
