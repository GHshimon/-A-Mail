# 下書きアシスタントのオフライン評価

`reply-cases.json`はすべて架空の製品・顧客・予定を使う。本番メール、認証情報、Sample企画書の正式な内容を含まない。

実メール送信やAI API呼び出しはこの評価器には存在しない。モデル評価を行う場合は、別途許可されたテスト手順で得た出力を次のJSONとして渡す。

```json
[
  {
    "case_id": "customer-a-price",
    "generated_text": "返信案",
    "questions": [{"id":"q1","text":"料金","status":"answered","source_ids":["a-contract-v1"]}],
    "sources": [{"id":"a-contract-v1"}],
    "review_reasons": [],
    "truncated": false
  }
]
```

```sh
node --test evals/evaluator.test.mjs
node evals/evaluate.mjs /absolute/path/to/fictional-predictions.json
```

出力は文章を再掲せずcase IDと失敗条件だけを表示する。空入力、欠損case、重複/未知case、形式不正は合格にしない。部分実行は意図的にmissingとして失敗する。

機械評価が分かるのは文字列条件、参照ID、質問件数・状態、省略表示だけ。**内容の正確さや本人らしさを意味的に証明するものではない**。特に自然な言い換えでは文字列条件が不合格になるため、人の判定と併記する。

本人評価は次を分けて記録する。

- 事実修正が必要だったか、質問への回答漏れがあったか。
- 自分なら使わない言い回しを何箇所直したか、学習前後のどちらを好むか。
- 根拠確認と編集にかかった時間。生成待ち時間は別記録する。
- 文体改善によって事実や承諾/辞退の方針が変わっていないか。

DB境界・送信重複・保存競合・学習適用・新着queueはRust/フロントの単体テストで検証する。このファイル群のテスト成功は製品実装の成功率ではない。学習例と評価例はスレッド単位で分ける。
