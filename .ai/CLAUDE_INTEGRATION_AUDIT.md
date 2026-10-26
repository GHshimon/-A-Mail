# 統合監査: claude ブランチ × main 未コミット変更

- 実施: Claude Code(読み取り専用監査。実装ファイルの編集・git 操作は行っていない)
- 作業タスク: `t0018`
- 対象:
  - `claude/goal-skill-implementation-b80806` @ `6247a53`(コミット済み 8 本)
  - `main` @ `3526ac8` + **未コミット 63 ファイル**(Codex/Terra が作業中)
  - 共通祖先: `af5efac`(M1: message body / reading)
- 参照: `DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md`, `d0024`〜`d0028`

> この文書は監査結果のみ。統合作業そのものは行っていない。

---

## 0. 実施しなかったこと(明示)

- 実装ファイル(`src/`, `src-tauri/`, 仕様書)の編集
- `git merge` / `cherry-pick` / `commit` / `rebase` / `push`
- 本番 DB・Keychain・実メールへの書き込み(DB は **readonly** で参照のみ)
- 生成 API / SMTP の呼び出し
- 既存の `tauri dev` プロセスへの操作(起動したまま。私のブランチのビルドが動作中)

---

## 1. 結論(先に読むべき 3 点)

### (1) 移行番号は「偶数/奇数の割当」では解決できない — 実際に同じ番号へ別スキーマが当たっている

両ブランチが `PRAGMA user_version` を線形カウンタとして使い、**同じ番号に別の DDL を割り当てている**。
しかも **実 DB は既に `user_version = 4` で私(claude)側のスキーマ**になっており、
main 側の V2〜V5 は一度も適用されていない。番号を振り直しても、
「この DB がどちらの系列を通ってきたか」を整数 1 つでは表現できない。

→ 対策は **§5 の構造検査付き互換移行**。

### (2) main の現行ビルドを実 DB に当てると、静かに壊れる

`apply()` は `if version < N` の連鎖なので、`user_version = 4` の DB に
main(`LATEST = 5`)を当てると **V5 だけが実行され、main の V2/V3/V4 が飛ばされる**。
結果 `settings.ai_provider` / `openai_model` / `ai_projects` が存在しないまま
`user_version = 5` になり、以後どのビルドでも自動修復されない。
エラーはマイグレーション時ではなく、後の SELECT で「no such column」として出る。

→ **統合前に main のビルドをこの DB へ向けないこと。** 検証は §6 のインメモリ/一時 DB で行う。

### (3) 文字化け修正は main 側に無い。取り込まないと退行する

main は `mail-parser` を `full_encoding` 無しで使い、`imap/fetch.rs` は祖先のまま。
UTF-7(フォルダ名)は main も独自に直しているが、
**RFC2047 の ISO-2022-JP と、ヘッダに生で書かれた ISO-2022-JP は未対応**。
実データで件名・本文が化けることを確認済み(`§3-A`)。

---

## 2. 競合ファイル

祖先からの変更が両側にあるファイルは **42 件**(lockfile 2 件を含む)。
規模は「追加行/削除行」。

### 2-a. 片側がほぼ単独で書き換えている(取り込み方針が明確)

| ファイル | 私 | main | 方針の目安 |
|---|---|---|---|
| `commands/mail_cmd.rs` | 408/73 | 4/1 | 私の版を土台に、main の 4 行を載せる |
| `store/mod.rs` | 977/6 | 306/13 | 追加が主。関数単位で和集合を取れる |
| `ai/mod.rs` | 272/1 | 21/1 | 私の版に main の provider 分岐を足す |
| `ai/prompts.rs` | 624/1 | 142/1 | 同上 |
| `commands/ai_cmd.rs` | 318/2 | 20/5 | 同上 |
| `state.rs` | 69/1 | 4/0 | フィールドの和集合(§2-c) |
| `store/models.rs` | 61/0 | 276/0 | main の assistant DTO + 私の `category` |
| `imap/sync.rs` | 29/1 | 4/1 | **同じバグを別実装で修正**(§2-d) |
| `imap/mod.rs` / `util/mod.rs` | 2〜3/0 | 1/0 | `pub mod` の和集合のみ |
| `commands/mod.rs` | 9/6 | 18/6 | **双方とも glob 再エクスポートへ修正済み。衝突しない** |
| `src/store/mailStore.ts` | 173/20 | 47/0 | 私の版が土台 |
| `src/components/list/MessageList.tsx` | 185/13 | 18/4 | 同上 |
| `src/components/reader/MessageView.tsx` | 176/15 | 66/7 | 要確認(main も操作バーを触っている可能性) |

### 2-b. 双方が大きく書き換えている(逐行の手動統合が要る)

| ファイル | 私 | main |
|---|---|---|
| `src/styles/global.css` | 882/0 | 556/1 |
| `src/components/ai/AiSidebar.tsx` | 116/22 | 190/24 |
| `src/components/settings/AiSettings.tsx` | 119/32 | 219/9 |
| `src/components/compose/ComposeWindow.tsx` | 158/56 | 32/79 |
| `src/components/layout/TitleBar.tsx` | 98/13 | 89/6 |
| `src/store/aiStore.ts` | 132/65 | 119/48 |
| `src/store/composeStore.ts` | 242/24 | 107/13 |
| `src/ipc/types.ts` | 96/0 | 171/0 |
| `src/ipc/ai.ts` | 74/0 | 189/2 |
| `smtp/send.rs` | 231/1 | 176/1 |
| `ai/gemini.rs` | 335/1 | 117/1 |
| `store/migrations.rs` | 48/2 | 177/3 |

**`global.css` は 59 個のセレクタを双方が定義**している
(`.ai-sec` `.row` `.iconbtn` `.primary` `.field` `.tone-seg` など)。
単純な連結では後勝ちになり、片方の見た目が静かに壊れる。
セレクタ単位の差分確認が必要。

### 2-c. `AppState` のフィールド

```
共通 : db: Mutex<Connection>, http: reqwest::Client
私   : ai_generation: AtomicU64, imap: ImapPool, secrets: Mutex<HashMap<String,String>>
main : cli_jobs: CliJobManager
```
名前は衝突しない。**和集合で足りる**。ただし `secrets`(Keychain キャッシュ)と
`imap`(接続プール)は §3 の必須保持項目。

### 2-d. 同じバグを別実装で直している(重複。どちらか一方を採用)

| 課題 | 私 | main |
|---|---|---|
| IMAP フォルダ名の modified UTF-7 | `util/utf7.rs::decode_mailbox`(パス全体を復号してから末尾を取る) | `util/imap_utf7.rs::decode`(末尾を取ってから復号) |

modified UTF-7 は `/` を符号化しないため**両者は等価**。
テストは私の側が実データ由来のケースを 7 本持っている。
どちらを残すにせよ、**関数を 2 本残さない**こと(将来どちらか片方だけ直す事故になる)。

### 2-e. IPC 契約の衝突(同名・別定義)

`src/ipc/types.ts` で **15 個の型名が双方に存在**する。うち以下は中身が違う。

| 型 | 私 | main | 判断が要る点 |
|---|---|---|---|
| `ContextPoint.category` | `PointCategory`(`history\|commit\|todo` の union) | `string` | union を残すと main 側の値が型エラー。**どちらの語彙を正とするか** |
| `ContextResult.related` | `RelatedMailSummary[]`(本文を返さない要約) | `MessageHeader[]` | **私の型は「AI へ送った範囲の明示」が目的**(SPEC 6.3)。`MessageHeader` に替えると送信範囲の表示根拠が失われる |
| `ContextResult` | — | `project_sources: ProjectFileHit[]` 追加 | 和集合可 |
| `DraftInput` | — | `body_html?: string \| null` 追加 | 和集合可 |
| `Settings` | `ai_classify_enabled` | `ai_provider`, `openai_model` | 和集合可。ただし §4 の移行と対で必要 |

main だけが持つ型は 27 個(`AssistantSettings` `ReplyRun` `StyleRule` `Workspace` `CliJob` 等)。
これらは新規追加なので衝突しない。

---

## 3. 必須保持変更(失うと機能が壊れる私の変更)

「main に対応物が無い」ことを確認済みのもの。**統合時に落とすと退行する。**

### A. 文字化け(3 段構え。1 つでも欠けると再発)

| # | 変更 | 場所 | main の状態 |
|---|---|---|---|
| A1 | `mail-parser` の `full_encoding` 有効化 | `Cargo.toml` | **無し**(素の `"0.11"`)。ISO-2022-JP / Shift_JIS / EUC-JP を復号できない |
| A2 | 生 ISO-2022-JP の後処理 | `util/jis.rs` + `imap/fetch.rs` | **無し**。RFC2047 で包まずヘッダに直接書くメールが化ける(実データで確認) |
| A3 | フォルダ名の UTF-7 復号 | `util/utf7.rs` + `imap/sync.rs` | **あり**(別実装) |

`encoding_rs = "0.8"` の依存追加も A2 に必須。

### B. IMAP 性能(体感速度の主因)

| # | 変更 | 場所 | main の状態 |
|---|---|---|---|
| B1 | 接続プール(TCP+TLS+LOGIN の再利用、SELECT の省略、NOOP による生存確認) | `imap/pool.rs`, `state.rs` | 無し |
| B2 | 差分同期(`UID FLAGS RFC822.SIZE` で下調べ → 未取得 UID のヘッダだけ取得) | `commands/mail_cmd.rs` | 無し(`fetch.rs` は祖先のまま) |
| B3 | 本文の先読み(上位 8 通 / 256KB 以下) | `commands/mail_cmd.rs` | 無し |

**注意**: `imap/fetch.rs` と `imap/actions.rs` は「SELECT 済み前提」に変更してある。
プールを採らずに B2/B3 だけ取り込むと SELECT が漏れる。**B1〜B3 は一体で扱うこと。**

### C. データ品質(祖先のバグ修正)

| # | 変更 | main の状態 |
|---|---|---|
| C1 | `from_addr` を `Name <addr>` 形で保存(祖先は表示名のみ。返信とアドレス帳でアドレスを失う) | 未修正 |
| C2 | 本文取得時に `snippet` を埋める(祖先は常に空) | 未修正 |
| C3 | 件名の `Re:`/`Fwd:` 判定のバイト切りによるパニック(日本語件名で発生) | 要確認(`util/mime.rs` は main では 1 行のスタブのまま) |
| C4 | `html_to_text` が `</script>` で再入し以降を失う | 同上 |

`util/mime.rs` は **main では未実装のスタブ**なので、私の実装をそのまま持ち込める。

### D. 機能

| # | 変更 | 備考 |
|---|---|---|
| D1 | 送信・下書き・宛先オートコンプリート(`util/reply.rs` の返信組み立て含む) | main の `smtp/send.rs` も 176 行あり **要突き合わせ** |
| D2 | 添付ファイルの保存(`safe_filename` によるパス脱出防止つき) | main に無し |
| D3 | 新着メールの AI 分類(`messages.category`) | main に無し。§4 の移行が前提 |
| D4 | 参考ファイル UI(`ContextFiles.tsx`, `lib/contextFile.ts`, `ai::clamp_context_files`) | main の `project_sources` と**目的が重なる**。§7 参照 |
| D5 | Keychain 読み出しのプロセス内キャッシュ(`state.secrets`) | main に無し。**無いと操作のたびに OS ダイアログが出る** |
| D6 | 通知(トースト)・空状態・設定画面への導線・キーボードナビ | main も UI を大きく触っており要突き合わせ |

### E. テスト

Rust 112 / フロント 111。`util/`(mime, reply, utf7, jis)と `store/` は
インメモリ DB で完結しており、**そのまま統合後の退行検知に使える**。
`ai/prompts.rs` のテストは §7 の provider 分岐で要調整。

---

## 4. migrations の実スキーマ相違

### 4-a. 同じ番号に別の内容が入っている

| version | claude(私) | main(Codex) |
|---|---|---|
| 1 | 初期スキーマ。`settings.model` 既定 **`gemini-2.5-flash`** | 初期スキーマ。`settings.model` 既定 **`gemini-flash-latest`** |
| 2 | `contacts` テーブル + index | `UPDATE settings SET model='gemini-flash-latest' WHERE model='gemini-2.5-flash'`(データのみ) |
| 3 | `messages.category` / `messages.category_reason` / index、`settings.ai_classify_enabled` | `settings.ai_provider`(CHECK 付き)/ `settings.openai_model` |
| 4 | 理由が空の `category='other'` を NULL へ戻す修復(データのみ) | `ai_projects` テーブル + index |
| 5 | — | `assistant_*` 11 テーブル + index |
| LATEST | 4 | 5 |

**V1 も同一ではない**(既定モデル文字列)。新規 DB の初期値が分岐する。

### 4-b. 実 DB の現状(readonly で確認)

```
PRAGMA user_version = 4
tables : accounts attachments contacts drafts folders messages messages_fts settings
settings 列: id ai_enabled ai_scope_related_count ai_send_body model theme
             poll_interval_sec ai_classify_enabled
messages 列: … category category_reason
```

→ **claude 系列のみが適用済み**。`ai_provider` / `openai_model` / `ai_projects` /
`assistant_*` は存在しない。

### 4-c. 失敗モード(いずれも例外を出さずに壊れる)

| ケース | 起きること |
|---|---|
| main ビルド(LATEST=5)を実 DB(v4)へ | `version<1..4` が全て偽 → **V5 だけ実行** → `ai_provider` 等が無いまま `user_version=5` → 以後どのビルドでも修復されない |
| claude ビルド(LATEST=4)を main 系列 DB(v5)へ | `5 >= 4` で即 return → `contacts` / `category` が無いまま起動 → SELECT で失敗 |
| 単純な番号振り直し(例: claude を 6,7,8 に) | 実 DB は v4。`version<5`(main の assistant)から実行されるが、main の V2/V3/V4 は依然飛ばされる。**解決しない** |
| 偶数/奇数の割当 | 上と同じ。**「どちらの系列を通ったか」が整数に入っていない**ことが本質 |

---

## 5. 安全な解決案: 構造検査付き互換移行

### 5-1. 方針

線形カウンタをやめ、**名前付き・冪等なステップの集合**にする。
`PRAGMA user_version` は人間とツール向けの参考値に降格させる。

```
schema_migrations(name TEXT PRIMARY KEY, applied_at INTEGER NOT NULL)
```

各ステップは `(name, kind, detect, sql)`。

- `kind = structural`: テーブル/列/インデックスを作る。**`detect` で存在を実地確認できる**
- `kind = data`: 値の書き換えのみ。構造に痕跡が残らないため `detect` できない

### 5-2. 適用手順

```
1. schema_migrations が無ければ作る
2. 【系列判定】構造から、この DB がどちらの系列を通ってきたかを推定する
     claude 系列  : contacts テーブルが存在する
     codex  系列  : settings.ai_provider 列が存在する
     どちらも無い : 新規 DB(または V1 だけの DB)
3. 【ブートストラップ】既存 DB を「適用済み」として取り込む
     structural: detect() が真なら SQL を流さず schema_migrations に記録
     data      : 系列と旧 user_version から、その系列で適用済みだった範囲を記録
                 例) claude 系列かつ user_version >= 4 → "claude/repair-empty-other" を
                     実行せず記録する(再実行すると正当な other 分類を消してしまうため)
4. 【適用】schema_migrations に無いステップを、宣言順に実行して記録
5. user_version を統合後の定数へ更新(情報用)
   すべて 1 トランザクション。失敗時は schema_migrations ごとロールバックされる
```

### 5-3. detect の例(構造検査)

```rust
fn has_table(c: &Connection, t: &str) -> rusqlite::Result<bool> {
    c.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [t], |_| Ok(true)
    ).optional().map(|o| o.is_some())
}

fn has_column(c: &Connection, t: &str, col: &str) -> rusqlite::Result<bool> {
    let mut st = c.prepare("SELECT 1 FROM pragma_table_info(?1) WHERE name=?2")?;
    st.exists(rusqlite::params![t, col])
}
```

### 5-4. ステップ表(統合後の宣言順)

| name | kind | detect | 由来 |
|---|---|---|---|
| `base/v1` | structural | `has_table("messages")` | 共通 |
| `codex/settings-provider` | structural | `has_column("settings","ai_provider")` | main V3 |
| `codex/settings-openai-model` | structural | `has_column("settings","openai_model")` | main V3 |
| `codex/ai-projects` | structural | `has_table("ai_projects")` | main V4 |
| `codex/assistant-*`(テーブルごとに 1 ステップ) | structural | `has_table("assistant_customers")` 等 | main V5 |
| `claude/contacts` | structural | `has_table("contacts")` | claude V2 |
| `claude/message-category` | structural | `has_column("messages","category")` | claude V3 |
| `claude/settings-ai-classify` | structural | `has_column("settings","ai_classify_enabled")` | claude V3 |
| `codex/normalize-model-name` | data | (不可) | main V2。冪等なので再実行可 |
| `claude/repair-empty-other` | data | (不可) | claude V4。**冪等でない**。§5-2 の 3 でブートストラップ時に「実行せず記録」する |

`assistant_*` を 1 ステップにまとめず**テーブル単位**に割ると、
途中で失敗した DB が次回の起動で自己修復できる。

### 5-5. V1 の既定モデル文字列

`settings.model` の既定は **main の `gemini-flash-latest` を採る**ことを推奨する
(main の `codex/normalize-model-name` が旧値を移行するため、両立する)。

ただし私の `src/components/settings/AiSettings.tsx` は
`<option value="gemini-2.5-flash">` を直書きしている。**統合時に選択肢を
Rust 側の定義へ寄せる必要がある**(放置すると設定画面で旧モデル名が選べてしまう)。

### 5-6. この案の限界(正直な範囲)

- `detect` は**存在の有無しか見ない**。列の型や CHECK 制約の違いは検出しない。
  両系列で同名・別定義の列が生まれた場合は取りこぼす(現時点では該当なし)。
- `data` ステップの取り込みは、系列判定と旧 `user_version` の**推定**に依存する。
  推定を誤ると `claude/repair-empty-other` が再実行され、
  正当な `other` 分類が未分類に戻る(害は限定的だが、AI 呼び出しが再発生する)。
- 実 DB での検証は**別の明示操作**とすること。§6 は全てインメモリ/一時 DB。

---

## 6. インメモリ DB テスト案

`Connection::open_in_memory()` のみを使い、実 DB・Keychain・ネットワークに触れない。

### 6-a. 移行そのもの

| ID | 内容 | 合格条件 |
|---|---|---|
| M01 | 新規 DB に apply | 両系列の全テーブル/列が存在し、`schema_migrations` に全ステップ名がある |
| M02 | 二重適用 | apply を 2 回。エラー無し、列の重複無し、行数不変 |
| M03 | **claude 系列の取り込み** | claude ステップだけ適用した DB(`user_version=4`)に統合版を apply → codex 側が追加され、`contacts` の番兵行が残る |
| M04 | **codex 系列の取り込み** | codex ステップだけ適用した DB(`user_version=5`)に apply → claude 側が追加され、`ai_projects` / `assistant_*` と `settings.model` が保たれる |
| M05 | **修復ステップの非再実行** | M03 の DB に `category='other', category_reason=NULL` の行を置いて apply → **その行が NULL に戻らない** |
| M06 | 部分適用からの自己修復 | `assistant_*` の一部だけ存在する DB → 欠けたテーブルだけ作られ、既存は変更されない |
| M07 | 失敗時のロールバック | 途中のステップを故意に失敗させる → テーブルも `schema_migrations` も変化しない |
| M08 | 旧 `user_version=0` かつ表は存在 | 構造検査だけで正しく取り込む(カウンタに依存しない) |
| M09 | FK 健全性 | apply 後に `PRAGMA foreign_key_check` が空 |
| M10 | データ保全 | accounts/folders/messages/drafts に種を入れて apply → 件数と主要列のハッシュが不変 |
| M11 | スキーマ固定 | apply 後の `sqlite_master.sql` を正規化して golden と比較(意図しない差分の検知) |

### 6-b. 統合後の退行検知(既存テストの流用)

- 私の `store::tests`(24 本)は `open_in_memory` + `migrations::apply` で完結。
  **ステップ名方式へ移行しても、そのまま合格すべき**。
- `util::{mime,reply,utf7,jis}` の 40 本は DB 非依存。**無改造で通ること**を統合の合格条件にできる。
- `§3-A` の退行検知として、`util::jis` の実データ由来ケース
  (`\x1b$B%d%^%O%;%_%J!<\x1b(B` → `ヤマハセミナーについて`)を必ず残す。

### 6-c. 仕様書 §11 との関係

`DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md` の受け入れテスト(D/S/C/R/L/A/U)は
assistant 機能の検証。**上の M01〜M11 は前提条件**にあたるので、
D01 等を走らせる前に M03/M04/M05 を通しておくことを勧める。

---

## 7. その他のリスク

### 7-1. 開発中の DB と Keychain をブランチ横断で共有している

- DB: `~/Library/Application Support/com.a-mail.app/a-mail.sqlite`
- Keychain: サービス `com.a-mail.app`

**統合が済むまで、main のビルドをこの DB へ向けないこと**(§1-(2))。
分けるなら `tauri.conf.json` の `identifier` を開発用に変えるのが確実だが、
これは実装ファイルの変更なので**この監査では行っていない**。

なお `tauri dev` はビルドのたびにバイナリの署名が変わるため、
Keychain の許可ダイアログが繰り返し出る。これは dev 特有で不具合ではない。

### 7-2. 「参考ファイル」と `project_scopes` / `ProjectFileHit` の重複

私の D4(作成画面でファイルを選ぶ)と、main の `ai_projects` /
`project_scopes` / `source_confirmations`(登録ルートを検索し確認する)は
**同じ課題への別の段階**。`d0003`/`d0004` のガードレールは共通している。

私の実装は「明示選択のみ・非永続・上限つき」で、main の設計の**部分集合**にあたる。
統合時は **main の枠組みに寄せ、私の UI を入口の 1 つとして残す**のが素直。
両方を別経路として残すと、送信範囲の表示が二重になり
「何を送ったか」が利用者に分からなくなる(SPEC 6.3 に反する)。

### 7-3. `ContextResult.related` の型

私の `RelatedMailSummary` は **本文を返さない**(id / 件名 / 日付 / 抜粋のみ)。
これは「AI へ送った範囲を画面に出す」ための型で、意図的に本文を落としている。
main の `MessageHeader[]` に置き換えると、
**表示している一覧が「送った範囲」なのか「単なる検索結果」なのかが区別できなくなる**。
どちらを採るかは仕様判断が要る。

### 7-4. `global.css` の 59 セレクタ重複

後勝ちで片方の見た目が壊れる。統合時はセレクタ単位で差分を取り、
`.ai-sec` `.row` `.iconbtn` `.primary` `.field` `.tone-seg` `.titlebar` 周りを
重点的に確認すること。

---

## 8. 未確認・この監査の限界

- **`smtp/send.rs` の突き合わせは未実施**(双方 176 行 / 231 行。ヘッダ組み立てと
  Sent への APPEND の扱いが異なる可能性がある)。
- **フロントの大規模競合 6 ファイル**(`AiSidebar` `AiSettings` `ComposeWindow`
  `TitleBar` `aiStore` `composeStore`)は行単位の突き合わせをしていない。
  規模と重複セレクタから「手動統合が要る」と判定したのみ。
- main は未コミットのため、**Codex/Terra の作業中に内容が変わる**。
  この監査は `main@3526ac8` + 参照時点の作業ツリーに対するもの。
- 分類プロンプトの精度、参考ファイル UI の見た目は**実機未確認**。
- 移行案 §5 は**設計のみ**。実装・実行はしていない。

---

## 9. 追加調査: 「ドラフト生成で要点が消える / 下書きが空」の原因

オーナーが架空メールで動作確認中に報告した症状の切り分け(読み取りのみ)。

### 9-0. 対象ビルドの特定

画面の文言から **main の `4587af5`(M3: AI 補助を実装)のビルド**と判定した。

| 手がかり | `4587af5` | main@HEAD `3526ac8` | claude ブランチ |
|---|---|---|---|
| サイドバーのモデル表示 | `"Gemini 2.5 Flash"`(固定文字列) | `"Gemini Flash"` | `{settings.model}`(実値) |
| 過去メールの見出し | 関連する過去メール | 同じ | 参照した過去メール |
| 文体/長さの位置 | 作成ツールバー | 同じ | AI ドロワー |
| 参考ファイル欄 | 無し | 無し | 有り |

→ **私のブランチではない。main 系列の不具合。**

### 9-1. 根本原因は 1 つ、症状は 2 つ

```
AiSidebar の useEffect 依存に bodyLen が入っている
        ↓
返信ドラフト生成 → setBody() で本文が入る → draft.body.length が変わる
        ↓
要点(B)を再取得する
        ↓
B も C も GenConfig::generation() = maxOutputTokens 512 / thinkingConfig 未指定
        ↓
gemini-2.5 系は思考トークンも maxOutputTokens を消費するため応答が途中で切れる
        ├─ B: JSON が文字列の途中で切れる → パース失敗
        │     → parse_points の最終フォールバックが生テキストを 1 件の「経緯」にする
        └─ C: 挨拶を出した直後で切れる → 中身のない下書き
```

該当箇所(`4587af5`):

| 場所 | 内容 |
|---|---|
| `src/components/ai/AiSidebar.tsx:37,69` | `const bodyLen = …draft.body.length` を `useEffect` の依存に入れている |
| `src-tauri/src/ai/gemini.rs:38` | `GenConfig::generation()` の `max_output_tokens: 512`。`thinkingConfig` の指定が無い |
| `src-tauri/src/commands/ai_gen_cmd.rs` | `parse_points` の最終フォールバックが `trimmed.chars().take(300)` を `category:"history"` の 1 件にする |

`GenConfig::generation()` は **B(要点)と C(返信)の両方**で使われている。

### 9-2. 実 DB で確認した補足

- `settings.model = gemini-flash-latest`。これは 2.5 系へのエイリアスなので**思考が有効**。
  サイドバーの「Gemini 2.5 Flash」は固定文字列で、実際の設定値を表示していない
  (表示と実体の不一致。それ自体も小さな不具合)。
- `user_version` は **4 のまま**。`4587af5` の `LATEST` は 2 なので `apply()` は即 return しており、
  **この DB は壊されていない**(claude 系列の余分な列は main の SELECT に影響しない)。
  危険なのは §1-(2) のとおり main の**作業ツリー側**(`LATEST = 5`)のビルド。

### 9-3. この不具合は claude ブランチでは修正済み(参照実装)

同じ原因を実データで踏み、`d0023` として修正してある。移植の参考になる。

| 対策 | claude ブランチでの実装 |
|---|---|
| 思考を切る | `ai/gemini.rs` の `GenRequest.no_thinking` → `generationConfig.thinkingConfig.thinkingBudget = 0` |
| 出力枠を実態に合わせる | 分類は件数 × 120 トークン(`clamp(512, 8192)`) |
| 壊れた出力を内容として見せない | `parse_points` / `parse_classifications` は**パース失敗時に空を返す**。生テキストを 1 件に化けさせない |
| 一時的な失敗を恒久化しない | `ai_classify_folder` は「1 件も読めなければ未分類のまま次回へ回す」 |
| 打鍵・生成での再送を避ける | `AiSidebar` の再取得キーを `accountId \| to \| subject \| 参考ファイル名` にし、**body を含めない** |

### 9-4. 推奨する直し方(main 側での作業。この監査では実施しない)

1. `GenConfig` に思考予算を持たせ、**要点(B)は思考オフ**にする。
2. B の出力枠を 512 → 1500〜2000 程度へ。C は思考を残すなら 1024〜2048 へ。
3. `parse_points` の**最終フォールバックを廃止**し、失敗時は空 + 「要点を取得できませんでした」を表示する。
   壊れた JSON を「経緯」として見せると、利用者が誤った内容を信じる危険がある。
4. `AiSidebar` の依存から `bodyLen` を外す。生成直後の再取得をやめる。
5. サイドバーのモデル表示を `settings.model` の実値にする。

`src-tauri/src/commands/ai_gen_cmd.rs` と `src/components/ai/AiSidebar.tsx` は
**main の作業ツリーで Codex/Terra が編集中**(未コミット 63 件に含まれる)。
衝突するため、この監査からは一切編集していない。
