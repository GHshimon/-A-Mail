# 依頼: 要点が壊れる / 下書きが空になる不具合の修正

- 依頼元: Claude Code(`goal-skill-implementation-b80806`)
- 依頼先: **Codex / Terra backend + frontend**
- 起票日: 2026-09-12
- 作業リスト: `t0021`(担当 `codex`)
- 根拠: `.ai/CLAUDE_INTEGRATION_AUDIT.md` §9 / 判断ログ `d0042` / 参照実装 `d0023`
- 経緯: オーナーが架空メールで動作確認中に報告 → Claude が読み取り専用で原因特定 →
  **該当ファイルが Codex の未コミット作業と重なるため、Claude は修正せず依頼に回した**

---

## 1. 症状(オーナー報告)

> AI によるドラフト生成ボタンを押す前は受信メールに対する要点が整理されていたのに、
> ドラフト生成ボタンで要点がなくなった。そのせいでドラフト生成文が中身がない。

画面では要点欄に次が表示されていた(**切り詰められた生 JSON がそのまま**)。

```
経緯 ： [ { "category": "history", "text": "先日オンラインにて機能・サービスの説明を実施
```

閉じ括弧・閉じ引用符が無い。返信本文は宛名と挨拶だけで途切れていた。

対象ビルド: **`main` @ `4587af5`(M3: AI 補助(Gemini)を実装)**
(モデル表示が固定文字列 `"Gemini 2.5 Flash"`、文体/長さが作成ツールバー、参考ファイル欄なし、で同定)

---

## 2. 原因(1 つの原因から 2 つの症状)

```
AiSidebar の useEffect 依存に bodyLen(本文の文字数)が入っている
   ↓ ドラフト生成 → setBody() → draft.body.length が変わる
   ↓ 要点(B)を再取得してしまう
   ↓ B も C も GenConfig::generation() = maxOutputTokens 512、thinkingConfig 未指定
   ↓ gemini-2.5 系は思考トークンも maxOutputTokens を消費 → 応答が途中で切れる
     ├ B: JSON が文字列の途中で切れる → パース失敗
     │    → parse_points の最終フォールバックが生テキストを 1 件の「経緯」にする
     └ C: 挨拶を出した直後で切れる → 中身のない下書き
```

`settings.model` の実値は `gemini-flash-latest`。これは 2.5 系へのエイリアスなので**思考が有効**。

### 該当箇所(`4587af5`)

| # | 場所 | 問題 |
|---|---|---|
| P1 | `src-tauri/src/ai/gemini.rs` `GenConfig::generation()` | `max_output_tokens: 512`。`thinkingConfig` の指定が無い。**B(要点)と C(返信)の両方で使われている** |
| P2 | `src-tauri/src/commands/ai_gen_cmd.rs` `parse_points()` | 最終フォールバックが `trimmed.chars().take(300)` を `category:"history"` の 1 件にする。**壊れた出力を内容として見せてしまう** |
| P3 | `src/components/ai/AiSidebar.tsx:37,69` | `const bodyLen = …draft.body.length` が `useEffect` の依存にある。生成直後と打鍵ごとに再取得する |

---

## 3. 依頼内容

### P1: 思考を切り、出力枠を実態に合わせる(Terra backend)

`GenConfig` に思考予算を持たせ、**構造化出力(要点)では思考を無効化**する。

```rust
// 参照実装: claude ブランチ src-tauri/src/ai/gemini.rs
#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "thinkingBudget")]
    thinking_budget: u32,
}

// generationConfig の中に置く
#[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
thinking_config: Option<ThinkingConfig>,

// 組み立て
thinking_config: req.no_thinking.then_some(ThinkingConfig { thinking_budget: 0 }),
```

- **B(要点)**: 思考オフ + 出力枠を 512 → **1500〜2000**
- **C(返信)**: 思考を残すなら枠を **1024〜2048**。切るなら 0 + 1024
- **A(予測入力)**: 速さ優先なので思考オフを推奨(claude 側は `no_thinking: true` / 96 トークン)

`GenConfig::generation()` を B と C で共用している構造そのものが事故の元なので、
**用途ごとに分けること**を勧める(claude 側は A/B/C/分類で別々に持っている)。

### P2: 壊れた出力を内容として見せない(Terra backend)

`parse_points` の**最終フォールバックを廃止**し、パースできなければ空を返す。

```rust
// 参照実装: claude ブランチ src-tauri/src/ai/prompts.rs::parse_points
pub fn parse_points(raw: &str) -> Vec<ContextPoint> {
    let text = strip_code_fence(raw);
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();          // ← 生テキストを 1 件に化かさない
    };
    // {"points":[…]} と素の配列 […] の両方を受ける
    let arr = v.get("points").and_then(|p| p.as_array()).or_else(|| v.as_array());
    let Some(arr) = arr else { return Vec::new() };
    …
}
```

**なぜ重要か**: 壊れた JSON を「経緯」として見せると、利用者が誤った内容を
事実として読んでしまう。空にして「要点を取得できませんでした」と出す方が安全。
仕様書 §10-5「読み取れていないのに『参照済み』と表示しない」と同じ趣旨。

### P3: 生成直後の再取得をやめる(Terra frontend)

`AiSidebar` の `useEffect` 依存から `bodyLen` を外す。

```ts
// 参照実装: claude ブランチ src/components/ai/AiSidebar.tsx
// 宛先・件名・参考ファイルが変わったときだけ取り直す。
// body の変化では投げ直さない(打鍵のたびに送信するのを避ける)。
const key = `${accountId}|${to.join(",")}|${subject}|${contextFiles.map(f => f.name).join(",")}`;
useEffect(() => { … }, [key]);
```

### P4(小): モデル表示を実値にする(Terra frontend)

`src/components/ai/AiSidebar.tsx` の `const model = "Gemini 2.5 Flash"` は固定文字列で、
実際の `settings.model`(現在 `gemini-flash-latest`)と一致していない。
仕様書 §10-4「モデル名は詳細欄へ」に合わせつつ、**実値を出す**こと。

---

## 4. 受け入れ条件

架空データ・インメモリ/一時 DB・偽 transport で確認すること(仕様書 §11 準拠)。

| ID | 合格条件 |
|---|---|
| X01 | 長い日本語メール(本文 2000 文字以上・末尾に質問が複数)で要点が JSON として解釈でき、生 JSON が画面に出ない |
| X02 | 応答を故意に途中で切った固定入力で `parse_points` が**空を返す**。`category:"history"` の 1 件に化けない |
| X03 | 要点が空のとき、画面に「取得できませんでした」等が出る。空欄や壊れた文字列にならない |
| X04 | 返信ドラフト生成後に**要点が再取得されない**(呼び出し回数を数える) |
| X05 | 返信本文が宛名・挨拶だけで途切れない(生成トークン数が枠内に収まる) |
| X06 | 本文への打鍵で要点の取得が発生しない |
| X07 | サイドバーのモデル表示が `settings.model` と一致する |

---

## 5. 依頼範囲外 / 注意

- **claude ブランチ(`claude/goal-skill-implementation-b80806`)は触らないでください。**
  同じ原因は `d0023` で既に修正済みです。参照実装として読むだけにしてください。
- **`store/migrations.rs` には手を入れないでください。**
  監査 §1-(2) のとおり、実 DB は `user_version = 4` で claude 系列のスキーマです。
  `LATEST = 5` のビルドをこの DB へ向けると main の V2〜V4 が飛ばされて静かに壊れます。
  移行方針(監査 §5 の構造検査付き互換移行)の採否はオーナー判断待ちです。
- 実メール送信・本番 DB の破壊的変更・API キーの自動有効化は対象外です。

---

## 6. Claude 側で実施済み / 未実施

| | |
|---|---|
| 実施 | 原因特定(読み取りのみ)、監査 §9 への記録、`t0021` 起票、`d0042` 記録、本依頼書の作成 |
| 未実施 | **実装ファイルの修正・git 操作**(該当ファイルが Codex の未コミット 63 件に含まれるため) |
| 未確認 | 修正後の実機動作(API キーが必要なため Claude 側では検証していない) |

不明点があれば `.ai/tasks.jsonl` の `t0021` に `note` を追記してください。
Claude 側は読み取り専用の監査担当として待機します。
