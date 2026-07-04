# -A-Mail 実装設計書 (IMPLEMENTATION)

SPEC.md(仕様確定・初版)を実装レベルに落とし込んだ設計書。実装者がそのまま着手できる粒度で記述する。

- 対象: macOS 個人利用のメールクライアント(Tauri v2 + React + TS)
- 前提: SPEC.md の技術スタック・認証方式・機能範囲を確定事項として踏襲
- 最終更新: 2026-07-04

---

## 0. 設計方針サマリ(トレードオフの結論だけ先に)

| 論点 | 採用 | 理由 |
|---|---|---|
| SQLite ドライバ | **rusqlite**(`bundled` + FTS5 feature) | 個人利用・同期処理は自前スレッドで直列化する方が単純。sqlx の非同期/コンパイル時検査は本件では過剰。バンドルで SQLite/FTS5 のバージョン差異を排除 |
| 非同期ランタイム | **tokio**(Tauri v2 標準) | async-imap / lettre / reqwest すべて tokio 前提 |
| DB アクセス形態 | **単一 writer コネクション + `Mutex`、read は都度接続 or プール** | rusqlite は `!Sync`。書き込みを直列化し WAL で読みを並行 |
| ストリーミング補完(A) | **Tauri Channel (`ipc::Channel<T>`)** でトークン逐次送信 | v2 の Channel はイベントより順序保証・型付けが楽。`emit` の名前衝突回避 |
| 状態管理(フロント) | **Zustand 採用** | Redux ほど重くなく、3ペイン間の共有状態(選択中アカウント/フォルダ/メッセージ)に最適。サーバ状態は薄い自前キャッシュ+invoke |
| HTMLメール表示 | **iframe sandbox + srcdoc + サニタイズ済みHTML + 外部リソースブロックCSP** | XSS/トラッキング防止の二重化 |
| 埋め込み検索 | **MVPは FTS5 のみ、埋め込みは将来** | gemini-embedding + sqlite-vec は M4 以降 |

---

## 1. リポジトリ / ディレクトリ構成

`npm create tauri-app@latest`(React + TS + Vite テンプレート)をベースに以下へ拡張する。

```
-A-Mail/
├── SPEC.md
├── IMPLEMENTATION.md
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── .env.local                # VITE_ 用(秘密情報は置かない)
│
├── src/                      # ── フロントエンド (React + TS) ──
│   ├── main.tsx
│   ├── App.tsx
│   ├── ipc/                  # Tauri invoke ラッパ(型付き API 層)
│   │   ├── client.ts         # invoke<T>() 薄ラッパ + エラー整形
│   │   ├── accounts.ts
│   │   ├── mail.ts           # folders/messages/sync/send/draft
│   │   ├── ai.ts             # complete(stream)/context/reply
│   │   └── types.ts          # Rust の型と対応する TS 型(手動 or ts-rs 生成)
│   ├── store/                # Zustand ストア
│   │   ├── accountStore.ts
│   │   ├── mailStore.ts
│   │   ├── composeStore.ts
│   │   ├── aiStore.ts
│   │   └── uiStore.ts        # ペイン幅・テーマ・選択状態
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppShell.tsx        # 3ペイン+補助ペインのグリッド
│   │   │   ├── ResizablePane.tsx
│   │   │   └── TitleBar.tsx        # traffic light 余白/ドラッグ領域
│   │   ├── sidebar/
│   │   │   ├── AccountList.tsx
│   │   │   └── FolderTree.tsx
│   │   ├── list/
│   │   │   ├── MessageList.tsx     # 仮想スクロール
│   │   │   └── MessageRow.tsx
│   │   ├── reader/
│   │   │   ├── MessageView.tsx
│   │   │   └── HtmlSandbox.tsx     # iframe sandbox
│   │   ├── compose/
│   │   │   ├── ComposeWindow.tsx
│   │   │   ├── GhostTextEditor.tsx # 予測入力(A)
│   │   │   └── AddressField.tsx
│   │   ├── ai/
│   │   │   ├── AiSidebar.tsx       # 関連情報(B)
│   │   │   ├── RelatedMailList.tsx
│   │   │   └── ReplyDraftButton.tsx# (C)
│   │   └── settings/
│   │       ├── AccountSettings.tsx
│   │       └── AiSettings.tsx      # オプトイン・送信範囲
│   ├── hooks/
│   │   ├── useTheme.ts             # ダーク/ライト追従
│   │   ├── useDebouncedCompletion.ts
│   │   └── useShortcuts.ts
│   ├── styles/
│   │   ├── tokens.css              # 色/フォント/spacing(native風)
│   │   └── global.css
│   └── lib/
│       └── sanitizeHtml.ts        # DOMPurify ラッパ
│
├── src-tauri/                # ── Rust バックエンド ──
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json      # v2 permission(allowlist 相当)
│   ├── icons/
│   └── src/
│       ├── main.rs           # tauri::Builder、状態注入、コマンド登録
│       ├── lib.rs            # run() 本体(モバイル互換の慣例)
│       ├── error.rs          # AppError(統一エラー型)
│       ├── state.rs          # AppState(DB プール, http クライアント等)
│       ├── commands/         # Tauri コマンド(IPC 境界)
│       │   ├── mod.rs
│       │   ├── account_cmd.rs
│       │   ├── mail_cmd.rs
│       │   ├── ai_cmd.rs
│       │   └── search_cmd.rs
│       ├── account/
│       │   ├── mod.rs
│       │   └── provider.rs   # Gmail/iCloud プリセット
│       ├── imap/
│       │   ├── mod.rs
│       │   ├── client.rs     # 接続/TLS/認証
│       │   ├── sync.rs       # 初回/増分同期
│       │   └── fetch.rs      # メッセージ取得・パース連携
│       ├── smtp/
│       │   ├── mod.rs
│       │   └── send.rs       # build + send + APPEND(Sent)
│       ├── store/
│       │   ├── mod.rs
│       │   ├── db.rs         # 接続/マイグレーション
│       │   ├── migrations.rs # DDL 適用
│       │   └── models.rs     # DB 行 <-> 構造体
│       ├── crypto/
│       │   └── keychain.rs   # keyring ラッパ
│       ├── ai/
│       │   ├── mod.rs
│       │   ├── gemini.rs     # HTTP・ストリーミング
│       │   └── prompts.rs    # A/B/C プロンプト組み立て
│       ├── search/
│       │   └── mod.rs        # FTS5 クエリ組み立て
│       └── util/
│           └── mime.rs       # mail-parser 補助
└── ...
```

**判断**: `main.rs` は薄く `lib.rs` の `run()` を呼ぶだけにする(Tauri v2 の推奨構成、将来モバイル無視でも慣例に従うと生成物との齟齬が減る)。

---

## 2. Rust モジュール設計

責務分離の原則: **commands 層は「引数検証 + ドメイン呼び出し + DTO 変換」だけ**を行い、業務ロジックは各ドメインモジュールに置く。秘密情報は `crypto::keychain` 以外から触らせない。

### 2.1 モジュール責務

| モジュール | 責務 | 依存 |
|---|---|---|
| `state` | `AppState`(DB ハンドル, `reqwest::Client`, 同期タスク管理, 設定キャッシュ)を保持し `tauri::State` で注入 | store, ai |
| `account` | アカウント CRUD、プロバイダ判定、接続設定プリセット解決 | store, crypto |
| `imap` | IMAP 接続・TLS・認証・フォルダ列挙・初回/増分同期・フラグ更新・本文取得 | store, crypto, util, account |
| `smtp` | メッセージ組み立て(MIME/ヘッダ)・送信・Sent への APPEND | crypto, account, imap(APPEND) |
| `store` | SQLite 接続、マイグレーション、CRUD、FTS5 更新 | rusqlite |
| `crypto::keychain` | Keychain への password/APIキー 保存・取得・削除 | keyring |
| `ai::gemini` | Gemini REST 呼び出し(通常/ストリーミング)、レート制御、リトライ | reqwest |
| `ai::prompts` | 機能 A/B/C の system/入力構築 | search, store |
| `search` | FTS5 検索クエリ生成・結果整形(B の土台) | store |
| `error` | `AppError` と `Result<T>` 別名、`serde` 直列化 | thiserror, serde |

### 2.2 主要な型(抜粋)

```rust
// error.rs
#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("認証に失敗しました")] Auth(String),
    #[error("ネットワークエラー")] Network(String),
    #[error("IMAP エラー")] Imap(String),
    #[error("SMTP エラー")] Smtp(String),
    #[error("DB エラー")] Db(String),
    #[error("Keychain エラー")] Keychain(String),
    #[error("AI エラー")] Ai(String),
    #[error("不正な入力")] BadInput(String),
    #[error("未検出")] NotFound,
}
pub type AppResult<T> = Result<T, AppError>;
// 変換: From<rusqlite::Error>, From<reqwest::Error> など。
// 重要: Display/Serialize にパスワード・APIキー・本文を絶対に含めない。

// account/mod.rs
pub struct Account {
    pub id: i64,
    pub email: String,
    pub provider: Provider,         // Gmail | Icloud
    pub display_name: String,
    pub imap_host: String, pub imap_port: u16,
    pub smtp_host: String, pub smtp_port: u16, pub smtp_starttls: bool,
}
pub enum Provider { Gmail, Icloud }

// store/models.rs
pub struct MessageHeader {   // 一覧表示用(軽量DTO)
    pub id: i64, pub uid: u32, pub subject: String,
    pub from: String, pub date: i64, pub snippet: String,
    pub seen: bool, pub flagged: bool, pub has_attachments: bool,
}
pub struct MessageFull {     // 本文表示用
    pub header: MessageHeader,
    pub to: Vec<String>, pub cc: Vec<String>,
    pub message_id: String, pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub body_text: Option<String>, pub body_html: Option<String>,
    pub attachments: Vec<AttachmentMeta>,
}

// ai/gemini.rs
pub struct GeminiRequest { /* contents, systemInstruction, generationConfig */ }
pub enum AiStreamEvent { Token(String), Done, Error(String) }
```

---

## 3. Tauri コマンド一覧(IPC API)

`#[tauri::command] async fn ...(state: State<'_, AppState>, ...) -> AppResult<T>`。
戻り値は `AppResult<T>` → フロントでは `invoke` の reject に `AppError`(`{kind, message}`)が入る。`ipc/client.ts` で正規化する。

| コマンド | 引数 | 戻り値 | 概要 |
|---|---|---|---|
| `add_account` | `email:String, app_password:String, provider:Provider, display_name:String` | `Account` | Keychain へ password 保存 + accounts へ行追加。接続テスト実施 |
| `list_accounts` | — | `Vec<Account>` | 登録済みアカウント |
| `remove_account` | `account_id:i64` | `()` | DB 行 + Keychain 項目削除 |
| `test_connection` | `account_id:i64` | `ConnCheck` | IMAP/SMTP ログイン可否 |
| `list_folders` | `account_id:i64` | `Vec<Folder>` | IMAP LIST + DB 反映 |
| `sync_folder` | `account_id:i64, folder_id:i64, mode:SyncMode` | `SyncResult` | 初回/増分同期(`{new, updated, deleted}` 件数) |
| `list_messages` | `folder_id:i64, offset:u32, limit:u32` | `Vec<MessageHeader>` | DB からページング取得 |
| `get_message` | `message_id:i64` | `MessageFull` | 本文含む。未取得ならIMAPで遅延取得 |
| `mark_read` | `message_id:i64, seen:bool` | `()` | DB 更新 + IMAP STORE \Seen |
| `set_flag` | `message_id:i64, flag:Flag, on:bool` | `()` | \Flagged など汎用フラグ |
| `send_message` | `draft:OutgoingMessage` | `SentInfo` | SMTP 送信 + Sent へ APPEND |
| `save_draft` | `draft:DraftInput` | `i64`(draft_id) | drafts へ upsert |
| `list_drafts` | `account_id:i64` | `Vec<DraftHeader>` | — |
| `delete_draft` | `draft_id:i64` | `()` | — |
| `search_messages` | `account_id:i64, query:String, limit:u32` | `Vec<MessageHeader>` | FTS5 全文検索 |
| `ai_complete` | `req:CompleteReq, on_event:Channel<AiStreamEvent>` | `()` | 予測入力(A)。トークンを Channel で逐次送信 |
| `ai_context_sidebar` | `req:ContextReq` | `ContextResult` | 関連情報(B)。FTS5→Gemini要約 |
| `ai_generate_reply` | `req:ReplyReq` | `String` | 返信ドラフト(C) |
| `get_settings` | — | `Settings` | ai_enabled/ai_scope/model など |
| `update_settings` | `patch:SettingsPatch` | `Settings` | 設定更新 |
| `set_gemini_key` | `api_key:String` | `()` | Keychain へ保存(DB には保存しない) |
| `has_gemini_key` | — | `bool` | キー登録有無のみ返す(値は返さない) |

補助型:
```
enum SyncMode { Initial, Incremental }
struct OutgoingMessage { account_id, to[], cc[], bcc[], subject, body_text, body_html?, in_reply_to?, references[], attachments[], draft_id? }
struct CompleteReq { account_id, subject, to[], body_prefix, cursor_context }
struct ContextReq  { account_id, to[], subject, body }
struct ReplyReq    { source_message_id, tone?, length? }
```

**エラー型方針**: 全コマンド `AppResult<T>` に統一。`AppError` は `#[serde(tag="kind")]` でフロントが `kind` により分岐(例: `Auth` → 再ログイン導線、`Network` → リトライ提示)。パスワード・APIキー・本文は `message` に載せない。

---

## 4. データモデル / SQLite DDL

`store/migrations.rs` に `PRAGMA user_version` ベースの逐次マイグレーションで適用。起動時 `PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`。

```sql
CREATE TABLE accounts (
  id            INTEGER PRIMARY KEY,
  email         TEXT NOT NULL UNIQUE,
  provider      TEXT NOT NULL CHECK(provider IN ('gmail','icloud')),
  display_name  TEXT NOT NULL DEFAULT '',
  imap_host     TEXT NOT NULL, imap_port INTEGER NOT NULL,
  smtp_host     TEXT NOT NULL, smtp_port INTEGER NOT NULL,
  smtp_starttls INTEGER NOT NULL DEFAULT 1,
  created_at    INTEGER NOT NULL
  -- パスワードは持たない(Keychain 参照)
);

CREATE TABLE folders (
  id           INTEGER PRIMARY KEY,
  account_id   INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,          -- 表示名
  imap_path    TEXT NOT NULL,          -- IMAP 上の完全パス
  role         TEXT,                   -- inbox/sent/drafts/trash/archive 等
  uidvalidity  INTEGER,                -- UIDVALIDITY
  uidnext      INTEGER,                -- 次回同期の起点候補
  highest_modseq INTEGER,             -- CONDSTORE 用
  last_synced  INTEGER,
  UNIQUE(account_id, imap_path)
);

CREATE TABLE messages (
  id            INTEGER PRIMARY KEY,
  account_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder_id     INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  uid           INTEGER NOT NULL,      -- フォルダ内 IMAP UID
  message_id    TEXT,                  -- RFC Message-ID
  in_reply_to   TEXT,
  reference_ids TEXT,                  -- References を空白区切りで保持
  from_addr     TEXT NOT NULL DEFAULT '',
  to_addrs      TEXT NOT NULL DEFAULT '',  -- JSON 配列文字列
  cc_addrs      TEXT NOT NULL DEFAULT '',
  subject       TEXT NOT NULL DEFAULT '',
  date          INTEGER NOT NULL,      -- epoch 秒
  snippet       TEXT NOT NULL DEFAULT '',
  flags         INTEGER NOT NULL DEFAULT 0,   -- ビットフラグ(seen/flagged/answered/draft/deleted)
  has_attachments INTEGER NOT NULL DEFAULT 0,
  body_fetched  INTEGER NOT NULL DEFAULT 0,    -- 本文遅延取得フラグ
  body_text     TEXT,
  body_html     TEXT,
  UNIQUE(folder_id, uid)              -- UIDVALIDITY 変化時は該当folderを作り直す
);
CREATE INDEX idx_messages_folder_date ON messages(folder_id, date DESC);
CREATE INDEX idx_messages_uid        ON messages(folder_id, uid);
CREATE INDEX idx_messages_msgid      ON messages(message_id);

-- FTS5(contentless外部コンテンツ方式でmessagesと同期)
CREATE VIRTUAL TABLE messages_fts USING fts5(
  subject, from_addr, to_addrs, body_text,
  content='messages', content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);
-- トリガで同期
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, subject, from_addr, to_addrs, body_text)
  VALUES (new.id, new.subject, new.from_addr, new.to_addrs, coalesce(new.body_text,''));
END;
CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_addr, to_addrs, body_text)
  VALUES('delete', old.id, old.subject, old.from_addr, old.to_addrs, coalesce(old.body_text,''));
END;
CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_addr, to_addrs, body_text)
  VALUES('delete', old.id, old.subject, old.from_addr, old.to_addrs, coalesce(old.body_text,''));
  INSERT INTO messages_fts(rowid, subject, from_addr, to_addrs, body_text)
  VALUES (new.id, new.subject, new.from_addr, new.to_addrs, coalesce(new.body_text,''));
END;

CREATE TABLE attachments (
  id          INTEGER PRIMARY KEY,
  message_id  INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  filename    TEXT NOT NULL,
  mime        TEXT NOT NULL,
  size        INTEGER NOT NULL DEFAULT 0,
  content_id  TEXT,                   -- インライン画像 cid:
  local_path  TEXT                    -- 遅延取得後のキャッシュパス
);
CREATE INDEX idx_attachments_msg ON attachments(message_id);

CREATE TABLE drafts (
  id          INTEGER PRIMARY KEY,
  account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  to_addrs    TEXT NOT NULL DEFAULT '',
  cc_addrs    TEXT NOT NULL DEFAULT '',
  bcc_addrs   TEXT NOT NULL DEFAULT '',
  subject     TEXT NOT NULL DEFAULT '',
  body_text   TEXT NOT NULL DEFAULT '',
  body_html   TEXT,
  in_reply_to TEXT,
  reference_ids TEXT,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE settings (            -- 単一行(id=1)想定 or key-value
  id            INTEGER PRIMARY KEY CHECK(id=1),
  ai_enabled    INTEGER NOT NULL DEFAULT 0,     -- オプトイン: 既定OFF
  ai_scope_related_count INTEGER NOT NULL DEFAULT 5, -- 関連過去メール送信件数
  ai_send_body  INTEGER NOT NULL DEFAULT 1,     -- 本文送信可否
  model         TEXT NOT NULL DEFAULT 'gemini-2.5-flash',
  theme         TEXT NOT NULL DEFAULT 'system', -- system/light/dark
  poll_interval_sec INTEGER NOT NULL DEFAULT 120
);
```

**UID / UIDVALIDITY の扱い**:
- フォルダごとに `SELECT` (or `EXAMINE`) 後 `UIDVALIDITY` を取得し `folders.uidvalidity` と比較。
- 一致 → `folders.uidnext` 以降を増分取得。
- **不一致 → 当該フォルダのキャッシュ(messages)を全削除して再同期**(UID の意味が変わるため)。
- `HIGHESTMODSEQ`(CONDSTORE 対応時)でフラグ変更の増分検出に使う。

**増分同期インデックス**: `idx_messages_folder_date`(一覧のページング)、`UNIQUE(folder_id, uid)`(重複挿入回避と upsert)、`idx_messages_msgid`(スレッド/参照解決)。

---

## 5. IMAP 同期戦略

### 5.1 接続
- `async-imap` + `async-native-tls`(または rustls)。993 は暗黙TLS、STARTTLS は使わない(IMAP は 993 固定)。
- 認証: `LOGIN`(アプリパスワード)。Gmail は `AUTHENTICATE PLAIN` 相当で可。
- 接続は**アカウント×用途で 1 本を再利用**し、同期タスク内で `select` を切り替える。取得と同期で衝突しないよう `AppState` 内でアカウント別 `Mutex<ImapSession>` を持つ。

### 5.2 初回同期(Initial)
1. `LIST` でフォルダ列挙 → `folders` upsert(role 推定: Gmail は `[Gmail]/Sent Mail` 等、iCloud は `Sent` 等をマッピング)。
2. 対象フォルダを `SELECT` → `UIDVALIDITY` / `UIDNEXT` / `HIGHESTMODSEQ` 記録。
3. **直近 N 件(例: 最新 200 UID)だけヘッダ取得**(`UID FETCH ... (UID FLAGS INTERNALDATE ENVELOPE BODYSTRUCTURE)`)→ `messages` に header のみ挿入(`body_fetched=0`)。
4. 本文は遅延: `get_message` 実行時に `UID FETCH uid (BODY.PEEK[])` → `mail-parser` でパース → `body_text/body_html/attachments` を埋め、`body_fetched=1`。`BODY.PEEK` により \Seen を勝手に立てない。

**判断**: 全件一括取得ではなく「ヘッダ先行 + 本文遅延」。個人メールボックスでも数万通あり得るため初回体感を優先。

### 5.3 増分同期(Incremental / ポーリング)
- 既定 **120 秒間隔**のバックグラウンドポーリング(`settings.poll_interval_sec`、tokio interval タスク)。UI 操作時(フォルダ選択)は即時トリガ。
- 新着: `SELECT` 後 `UID SEARCH UID <uidnext>:*`(or `UIDNEXT` 差分)で新規 UID を取得 → ヘッダ FETCH → 挿入。`folders.uidnext` 更新。
- **フラグ同期**:
  - CONDSTORE 対応サーバ: `UID FETCH 1:* (FLAGS) (CHANGEDSINCE <highest_modseq>)` で変更のみ取得 → `flags` 更新 → `HIGHESTMODSEQ` 更新。
  - 非対応/簡易: 直近ウィンドウの UID について `FLAGS` を再取得し差分反映。
- 削除検出: `UID SEARCH` 結果に存在しない既知 UID を「消えた」とみなし論理削除 or 物理削除(Gmail はラベル移動で消えることに留意、初期は物理削除で可)。

### 5.4 将来: IMAP IDLE
- M4 で `IDLE` を導入。フォルダ選択中は IDLE で待機し `EXISTS`/`EXPUNGE` 通知で即時増分同期。IDLE は 29 分でリフレッシュ。ポーリングはフォールバックとして残す。

---

## 6. SMTP 送信フロー

### 6.1 メッセージ組み立て(`smtp/send.rs`, lettre `MessageBuilder`)
- 共通ヘッダ: `From`(アカウント), `To/Cc/Bcc`, `Subject`, `Date`, `Message-ID`(自前生成 `<uuid@domain>`), `MIME-Version`。
- 本文: text のみ → `text/plain`。HTML併用 → `multipart/alternative`。添付あり → `multipart/mixed`(alternative をネスト)。
- **返信(Reply / Reply-All)**:
  - `In-Reply-To: <元Message-ID>`
  - `References: <元References...> <元Message-ID>`(元の References に元 Message-ID を追記)
  - `Subject`: 既に `Re:` が無ければ付与。
  - Reply-All は元 `To`+`Cc` から自分のアドレスを除いて宛先化。
- **転送(Forward)**: `Subject: Fwd:`、本文に元ヘッダ+本文を引用、添付は元を引き継ぐ(本文の再取得が必要)。`In-Reply-To`/`References` は付けない。

### 6.2 送信
- Gmail: 465(暗黙TLS) 優先、fallback 587(STARTTLS)。iCloud: 587(STARTTLS)。lettre の `SmtpTransport`(tokio async) + `Tls::Wrapper`/`Tls::Required`。
- 認証: アカウントの Keychain パスワード。

### 6.3 送信後処理
1. 送信成功 → 生成した RFC822 バイト列を **Sent フォルダへ `APPEND`**(`\Seen` フラグ付き)。多くのサーバは自動保存しないため必須。Gmail は SMTP 送信分を自動で Sent に入れる場合があり**二重化を避けるため provider 別に APPEND 要否を切替**(Gmail=スキップ、iCloud=APPEND)。
2. 返信の場合、元メッセージに `\Answered` を STORE。
3. `draft_id` があれば drafts から削除。
4. Sent フォルダの増分同期をトリガ。

### 6.4 下書き
- `save_draft` は DB(drafts) にローカル保存(MVP は IMAP Drafts への APPEND はしない)。オートセーブはフロントで数秒デバウンス。将来 IMAP Drafts 同期を追加。

---

## 7. Gemini 連携設計

### 7.1 エンドポイント / モデル
- Base: `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent`(通常)
- ストリーミング: `:streamGenerateContent?alt=sse`(SSE で逐次受信)
- 認証: ヘッダ `x-goog-api-key: <Keychainのキー>`(URL クエリにキーを載せない＝ログ混入回避)。
- 共通 `generationConfig`: 補完は `temperature 0.2, maxOutputTokens 48, stopSequences=["\n\n"]`。要約/返信は `temperature 0.4, maxOutputTokens 512`。

### 7.2 リクエスト/レスポンス形(要点)
```jsonc
// request
{
  "systemInstruction": { "parts": [{ "text": "<system>" }] },
  "contents": [{ "role": "user", "parts": [{ "text": "<入力>" }] }],
  "generationConfig": { "temperature": 0.2, "maxOutputTokens": 48 }
}
// response(non-stream): candidates[0].content.parts[0].text
// stream(SSE): data: {"candidates":[{"content":{"parts":[{"text":"..."}]}}]} が逐次
```

### 7.3 プロンプト設計

**A. 予測入力(ゴーストテキスト)**
- system: 「あなたはメール作成の続きを予測する補完エンジン。ユーザーが書いた文の**自然な続きのみ**を短く(最大1文〜十数語)出力。挨拶や説明・引用符を付けない。既に書かれた文字は繰り返さない。」
- 入力: 件名 + 宛先 + **カーソル直前の本文(末尾 500〜800 文字に制限)**。カーソル以降は送らない。
- 出力: 続きテキストのみ。フロントでゴースト表示、Tab確定。
- **プライバシー**: `ai_enabled` かつ `ai_send_body` が真のときのみ。過去メールは送らない。

**B. 関連情報サイドバー**
1. `search` が作成中の `to/subject/body` からキーワード抽出(宛先ドメイン/固有名詞/件名語)→ FTS5 で上位 `ai_scope_related_count`(既定5) 件を取得。
2. system: 「以下は作成中メールと、関連する過去メール抜粋。**経緯・約束事(コミット)・未対応事項・次アクション**を日本語の箇条書きで簡潔に要約。推測は避け、根拠が無い項目は出さない。」
3. 入力: 作成中メールの要旨 + 過去メール n 件(各: 日付/相手/件名/本文抜粋を**トークン節約のため各 800 字上限**)。
4. 出力: 構造化要点("経緯"/"約束"/"未対応")+ フロントは関連メール一覧を別途 FTS 結果から併記。

**C. 返信ドラフト生成**
- system: 「受信メールに対する返信の下書きを作成。相手の要件に対応し、日本語ビジネスメールとして自然な文面。件名は書かず本文のみ。トーン=<tone>、長さ=<length>。」
- 入力: 元メール(差出人/件名/本文、本文は 2000 字上限)+ 任意で作成者名。
- 出力: 返信本文。エディタに挿入(ユーザーが編集前提)。

### 7.4 ストリーミングとデバウンス(A)
- **デバウンス 300ms**(タイピング停止後)。連続タイプ中はリクエストを発行しない。推奨: 入力停止 300ms + 直近リクエストが in-flight ならキャンセル。
- Rust 側: `ai_complete` は tokio task で SSE を読みつつ `Channel<AiStreamEvent>` に `Token` を送る。新規リクエスト時は前 task を `CancellationToken` で中断(フロントからは同一 Channel の再呼び出しで置換)。
- フロント: `AbortController` 相当としてリクエスト世代 ID を持ち、古い世代のトークンは破棄。

### 7.5 レート制御・失敗フォールバック
- クライアント側スロットル: A は最短間隔 500ms・同時 1 本。B/C は同時 1 本。
- 429/5xx: 指数バックオフ(0.5s,1s,2s、最大3回)。A は**リトライせず黙って諦める**(体感優先、次のタイプで再試行)。
- キー未登録/AI無効: コマンドは `AppError::Ai("disabled")` 相当を返し、UI は補完を無効化(エラートースト出さない、Aは静かにOFF)。
- タイムアウト: A=4s、B/C=20s。

---

## 8. フロントエンド設計

### 8.1 コンポーネントツリー(主要)
```
<App>
 └ <AppShell>                     // CSS grid: sidebar | list | reader(+ai)
    ├ <TitleBar/>                 // traffic light 余白確保・ドラッグ領域
    ├ <Sidebar> <AccountList/> <FolderTree/> </Sidebar>
    ├ <ResizablePane> <MessageList/> </ResizablePane>   // 仮想スクロール
    └ <ReaderArea>
        ├ <MessageView> <HtmlSandbox/> </MessageView>  // 閲覧時
        └ <ComposeWindow>                               // 作成時
            ├ <AddressField/> <SubjectField/>
            ├ <GhostTextEditor/>          // 予測入力(A)
            ├ <ReplyDraftButton/>         // (C)
            └ <AiSidebar> <RelatedMailList/> </AiSidebar>  // (B)
```

### 8.2 状態管理(Zustand)
- `uiStore`: theme, ペイン幅, 選択中 account/folder/message, モード(read/compose)。
- `accountStore`: accounts, current。
- `mailStore`: folder ごとの message header キャッシュ(`Map<folderId, MessageHeader[]>`)、`fetchMessages(folderId, page)`、既読更新の楽観反映。
- `composeStore`: 現在の下書き(to/cc/subject/body/inReplyTo/references)、オートセーブ。
- `aiStore`: aiEnabled, completion 状態(ghost text, requestGen), sidebar 要点, related mails。
- **判断**: サーバ状態は React Query 等を入れず、`mailStore` の軽量キャッシュ+invoke で十分(データ源は自前 SQLite で高速、外部 API 的な再検証が不要)。

### 8.3 データ取得とキャッシュ
- 全アクセスは `src/ipc/*.ts` の型付きラッパ経由(`invoke` 直呼び禁止)。
- 一覧は `list_messages(folderId, offset, limit)` でページング + 仮想スクロール(`@tanstack/react-virtual`)。
- 本文は選択時に `get_message` で取得しコンポーネントローカル + 軽キャッシュ。
- 既読は楽観更新(即UI反映 → `mark_read` 失敗時ロールバック)。

### 8.4 ゴーストテキスト補完エディタ
- 実装方針: **contentEditable ではなく `<textarea>` + オーバーレイ方式**を推奨(実装が単純・IME 事故が少ない)。
  - 下層に実テキストの `<textarea>`、その上に同一フォント/行送りの重ねた `<div>` を絶対配置し、`本文 + <span class="ghost">補完</span>` を描画。
  - `useDebouncedCompletion`(300ms)でカーソル前文脈を取り `ai_complete` を Channel 購読。
  - **Tab**: ゴーストを本文へ確定挿入。**Esc/文字入力**: ゴースト破棄。IME 変換中(`compositionstart`〜`end`)は補完トリガを止める。
- リッチ HTML 編集は MVP スコープ外(送信は text/plain 主体、必要なら簡易 HTML)。

### 8.5 HTML メールのサンドボックス表示
- `<iframe sandbox="allow-popups-to-escape-sandbox" srcdoc={sanitized}>`(`allow-scripts` は**付けない**)。
- 表示前に **DOMPurify でサニタイズ**(`FORBID_TAGS: script, iframe, object`, イベント属性除去)。
- **外部画像デフォルトブロック**: iframe 内 CSP `default-src 'none'; img-src data: cid-store` 等で外部読み込み遮断。ユーザーが「画像を表示」を押すと `img-src https:` を許可した srcdoc に差し替え。
- `cid:` インライン画像は取得済み添付を data URL / カスタムプロトコルで解決。
- リンククリックは親でハンドルし `shell.open`(既定ブラウザ)で開く。

---

## 9. macOS ネイティブ UI 実装

### 9.1 ウィンドウ設定(`tauri.conf.json`)
```jsonc
"app": { "windows": [{
  "title": "-A-Mail",
  "width": 1200, "height": 800, "minWidth": 900, "minHeight": 600,
  "titleBarStyle": "Overlay",     // タイトルバー透過、traffic light を内容に重ねる
  "hiddenTitle": true,
  "transparent": true,            // vibrancy のため
  "decorations": true,
  "theme": null                   // システム追従
}]}
```
- traffic light 位置: `titleBarStyle: Overlay` + サイドバー先頭に約 `72px` の余白(ドラッグ領域 `data-tauri-drag-region`)を確保。細かい位置は `window-vibrancy` / `tauri-plugin` or 起動時に `ns_window` へ `setTrafficLightPosition`(macos-private-api)で調整。

### 9.2 Vibrancy(半透明サイドバー)
- **`window-vibrancy` crate** を採用。`apply_vibrancy(&window, NSVisualEffectMaterial::Sidebar, None, None)` を setup で呼ぶ。
- サイドバー DOM 背景は透過(`background: transparent`)にし、下地の vibrancy を透かす。`tauri.conf.json` の `macOSPrivateApi: true` を有効化。
- **判断**: `macos-private-api` の生 API より `window-vibrancy` の方が material 指定が簡潔。App Store 非申請なので private API 使用可。

### 9.3 システムフォント / テーマ
- フォント: `font-family: -apple-system, "SF Pro Text", system-ui;`(`tokens.css`)。
- ダーク/ライト: `window.matchMedia('(prefers-color-scheme: dark)')` を `useTheme` で監視 + CSS 変数切替。`settings.theme` が `system` 以外なら強制。Tauri の `window.theme()` / `on_theme_changed` も併用。

### 9.4 キーボードショートカット(`useShortcuts`)
- `⌘N` 新規作成 / `⌘R` 返信 / `⇧⌘R` 全員返信 / `⌘⇧F` 転送 / `⌘↩` 送信 / `⌘⌫` 削除 / `⌘F` 検索 / `J/K` 一覧移動 / `⌘,` 設定。
- グローバルは Tauri accelerator、リスト内ナビはフロント keydown。

---

## 10. セキュリティ

### 10.1 Keychain 命名
- サービス名: `com.a-mail.app`
- アカウントパスワード: account = `imap:{email}` / `smtp:{email}`(同一パスワードなら `credential:{email}` 単一でも可)。
- Gemini APIキー: account = `gemini_api_key`。
- 取得は `crypto::keychain` のみ。値はコマンド戻り値に**絶対に含めない**(`has_gemini_key` は bool のみ)。

### 10.2 非ログ化
- `AppError` の `Display`/`Serialize` にパスワード・APIキー・本文を含めない。
- `reqwest`/`lettre` のデバッグログを本番ビルドで抑制。Gemini リクエストボディはログ出力しない(本文が含まれるため)。
- `tracing` 導入時は `secret` フィールドを `#[instrument(skip(...))]` で除外。

### 10.3 HTML メール XSS 対策
- 二重防御: (1) DOMPurify サニタイズ、(2) iframe `sandbox`(no allow-scripts)+ CSP。
- リンクは `rel="noopener noreferrer"`、遷移は親側で `shell.open`。

### 10.4 Tauri v2 権限(capabilities)/ CSP
- `capabilities/default.json` で許可を**最小限**に: `core:window`, `core:event`, 使用する plugin(`shell:allow-open` を url 限定, `os`, `notification`(M4))。ファイルシステムは添付キャッシュディレクトリのみに scope 限定。
- `tauri.conf.json` の `security.csp`:
  `default-src 'self'; img-src 'self' data: asset:; connect-src 'self' ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'`
  Gemini への外部通信は **Rust 側のみ**(WebView から直接叩かない)。
- `dangerousDisableAssetCspModification` は使わない。

---

## 11. ビルド / 開発環境

### 11.1 必要ツール
- macOS + **Xcode Command Line Tools**(`xcode-select --install`)
- **Rust**(`rustup`, stable)
- **Node.js 20+** / npm(or pnpm)
- 事前: 各アカウントのアプリパスワード、Gemini API キー。

### 11.2 初期化(`npm create tauri-app` ベース)
```bash
npm create tauri-app@latest -A-Mail -- --template react-ts
cd -A-Mail
# Rust 依存を Cargo.toml に追加:
#  tauri(v2), tauri-build, tokio, async-imap, async-native-tls,
#  lettre(tokio1-native-tls), mail-parser, rusqlite(bundled, functions),
#  keyring, reqwest(json, stream), serde/serde_json, thiserror,
#  window-vibrancy, uuid, tracing
# フロント依存:
npm i zustand @tanstack/react-virtual dompurify
```
- `rusqlite` は `features = ["bundled"]`(FTS5 は bundled の SQLite に含まれる／必要なら `bundled-full`)。

### 11.3 開発コマンド
```bash
npm run tauri dev      # フロント HMR + Rust ホットリロード
npm run tauri build    # リリースビルド
```

### 11.4 .app ビルド
- `npm run tauri build` → `src-tauri/target/release/bundle/macos/-A-Mail.app`。
- 個人利用のため署名/公証は不要。Gatekeeper 回避は初回 `右クリック→開く` or `xattr -dr com.apple.quarantine`。
- Apple Silicon 前提なら `--target aarch64-apple-darwin`。

---

## 12. 実装タスク分解(M0〜M4)

### M0 土台
- [ ] `create tauri-app`(react-ts)で雛形生成、依存追加
- [ ] `tauri.conf.json`: window(Overlay/transparent)・CSP・macOSPrivateApi 設定
- [ ] `window-vibrancy` でサイドバー vibrancy 適用、traffic light 余白
- [ ] `AppShell` 3ペイン+補助ペイン(リサイズ・仮想スクロール骨組み)
- [ ] `useTheme`(ダーク/ライト追従)、`tokens.css`(システムフォント)
- [ ] `error.rs`(AppError)・`state.rs`(AppState)・DB 初期化+マイグレーション骨組み
- [ ] `crypto::keychain` ラッパ + `set_gemini_key`/`has_gemini_key`
- [ ] `ipc/client.ts`(型付き invoke + エラー正規化)

### M1 閲覧
- [ ] `add_account`/`list_accounts`/`test_connection` + provider プリセット + Keychain 保存
- [ ] `imap::client` 接続/TLS/LOGIN、`list_folders`(LIST + folders upsert, role 推定)
- [ ] `sync_folder`(Initial): UIDVALIDITY/UIDNEXT 記録 + 直近ヘッダ FETCH → messages 挿入
- [ ] `list_messages`(ページング)+ `MessageList`/`MessageRow`
- [ ] `get_message`(本文遅延取得 + mail-parser パース + attachments 行)
- [ ] `HtmlSandbox`(DOMPurify + iframe sandbox + 外部画像ブロック/表示切替)
- [ ] `mark_read`/`set_flag`(DB + IMAP STORE、楽観更新)
- [ ] 増分同期(Incremental)ポーリング(interval + UIDNEXT 差分 + フラグ同期)

### M2 作成 / 送信
- [ ] `ComposeWindow` + `AddressField`/`SubjectField`、compose ストア
- [ ] `smtp::send` ヘッダ組み立て(新規/返信=In-Reply-To/References/転送)
- [ ] `send_message`(SMTP 送信 + provider 別 Sent APPEND + \Answered + Sent 同期)
- [ ] `save_draft`/`list_drafts`/`delete_draft` + オートセーブ
- [ ] ⌘系ショートカット(新規/返信/送信/削除)

### M3 AI 補助
- [ ] `ai::gemini`(generateContent + streamGenerateContent SSE、x-goog-api-key、非ログ化)
- [ ] `ai::prompts`(A/B/C の system/入力構築)
- [ ] (A) `ai_complete`(Channel ストリーミング) + `GhostTextEditor`(textarea+overlay, 300ms debounce, Tab確定, IME 対応)
- [ ] `search`(FTS5) + `search_messages`
- [ ] (B) `ai_context_sidebar`(FTS5→要約) + `AiSidebar`/`RelatedMailList`
- [ ] (C) `ai_generate_reply` + `ReplyDraftButton`
- [ ] `AiSettings`(オプトイン既定OFF・送信範囲・件数・キー登録)+ プライバシー制御配線

### M4 拡張
- [ ] 複数アカウント同時利用(アカウント別接続/同期タスク)
- [ ] フォルダ/ラベル操作(移動/アーカイブ/削除/スター)
- [ ] 検索 UI(結果ハイライト・フィルタ)
- [ ] 添付ダウンロード/送信添付
- [ ] macOS 通知(notification plugin)
- [ ] IMAP IDLE プッシュ受信
- [ ] スレッド表示(message_id/references でスレッド化)
- [ ] 埋め込みセマンティック検索(gemini-embedding + sqlite-vec、B 高精度化)

---

## 付録: 主要トレードオフの補足

- **rusqlite vs sqlx**: 本件は単一ローカル DB・書き込みは同期タスクで直列化でき、非同期 DB の恩恵が薄い。rusqlite + WAL + writer Mutex が最小構成。将来並行度が問題化したら `r2d2` プールを追加。
- **ストリーミング方式(Channel vs Event)**: v2 `ipc::Channel<T>` は型付き・順序保証・購読解除が明快で、補完トークンの逐次配信に最適。グローバル `emit` は名前空間衝突と多重購読の管理が煩雑。
- **埋め込み検索の将来対応**: FTS5 はキーワード一致に強いが言い換えに弱い。B の精度不足時に `gemini-embedding` でベクトル化し `sqlite-vec` 拡張でコサイン近傍検索を追加(DDL に `embeddings(message_id, vec BLOB)` を足すだけで移行可能な設計にしておく)。
- **ゴーストテキスト(textarea+overlay vs contentEditable)**: contentEditable は IME・カーソル・選択の制御が難しくバグ源。overlay 方式は描画同期のコストはあるが挙動が予測可能で、text/plain 主体の本件に合致。
