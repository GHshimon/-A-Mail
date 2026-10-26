# A-mail 顧客コンテキスト付き返信実装仕様書

> 2026-09-09更新: 承認されたUI/UX・本人文体学習レビューを反映した実装指示は
> [DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md](DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md) に統合した。
> 本文は旧案として保持する。共通資料の自動公開、confirmed既定、未確定時の生成全面停止、
> スナップショット不足、評価の後回し等は新仕様で置き換える。

- ステータス: 旧版・新P0仕様を優先
- 作成日: 2026-09-08
- 対象: A-mail（Tauri v2 + Rust + React/TypeScript + SQLite）
- 対象外: この文書作成時点でのコード変更、メール自動送信、顧客情報の外部API同期

## 1. 目的と採用判断

製品が同じでも返信先の顧客情報を混在させないため、メールアドレスやフォルダを唯一の紐付けキーにしない。A-mail内部の不変ID、メールスレッドの関係、顧客ワークスペース、明示的な参照許可を組み合わせ、返信生成に渡すコンテキストを決定する。

採用する中核は次のとおり。

1. `customer_id`、`workspace_id`、`project_id`、`thread_id`、`message_id`を内部IDとして持つ。
2. `In-Reply-To`、`References`、プロバイダースレッドIDを使ってスレッド候補を作る。
3. 顧客・案件が確定した範囲だけを検索し、AI入力直前に境界チェックする。
4. 返信生成時の参照範囲をスナップショットとして記録する。
5. AIは返信案と根拠を提示するが、送信は必ず利用者が明示する。

フォルダは表示・整理用の補助情報とし、AIの顧客境界には使わない。

## 2. 用語

| 用語 | 定義 |
|---|---|
| Customer | 顧客・組織の論理的な主体 |
| Workspace | 顧客に属する、メール・案件・資料の参照境界 |
| Project | 顧客ワークスペース内の案件。任意 |
| Thread | A-mail内部で管理する会話単位 |
| Message | DBに保存された1通のメール |
| Context binding | Workspaceが参照できるメール・ファイル・プロジェクトの許可 |
| Context snapshot | 返信生成時点で実際に使ったID・出典・版の記録 |
| Common source | 全顧客で参照可能な製品資料・共通FAQ |

## 3. データモデル

### 3.1 既存テーブルへの追加

既存の`messages`はプロバイダーごとの同一UIDがあり得るため、既存の整数`messages.id`をA-mail内部の`message_id`として継続利用する。

追加列:

```sql
ALTER TABLE messages ADD COLUMN thread_id INTEGER REFERENCES threads(id);
ALTER TABLE messages ADD COLUMN workspace_id INTEGER REFERENCES workspaces(id);
ALTER TABLE messages ADD COLUMN project_id INTEGER REFERENCES projects(id);
ALTER TABLE messages ADD COLUMN context_status TEXT NOT NULL DEFAULT 'unassigned';
ALTER TABLE messages ADD COLUMN classification TEXT;
ALTER TABLE messages ADD COLUMN classification_status TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE messages ADD COLUMN classification_confidence REAL;
ALTER TABLE messages ADD COLUMN classified_at INTEGER;
```

`message_id`という外部名を新しい文字列列として追加しない。既存の数値主キーをDTOでは`id`として返し、外部メールのMessage-IDは`rfc_message_id`として区別する。

既存の`message_id TEXT`列は外部RFC Message-IDとして扱い、移行時に名前を変える場合は段階移行とする。既存コードへの影響が大きいため、初回実装ではDB列名を維持し、Rust DTO上の名前だけを明確化する。

### 3.2 新規テーブル

```sql
CREATE TABLE customers (
  id INTEGER PRIMARY KEY,
  display_name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active'
    CHECK(status IN ('active','archived')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE workspaces (
  id INTEGER PRIMARY KEY,
  customer_id INTEGER NOT NULL REFERENCES customers(id) ON DELETE RESTRICT,
  name TEXT NOT NULL,
  context_version INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'active'
    CHECK(status IN ('active','archived')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE projects (
  id INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE RESTRICT,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active'
    CHECK(status IN ('active','paused','completed','archived')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE threads (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  workspace_id INTEGER REFERENCES workspaces(id) ON DELETE SET NULL,
  project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
  provider_thread_id TEXT,
  subject_key TEXT NOT NULL DEFAULT '',
  context_status TEXT NOT NULL DEFAULT 'unassigned'
    CHECK(context_status IN ('unassigned','candidate','confirmed','needs_review','conflict','common_only','locked')),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_threads_workspace ON threads(workspace_id, updated_at DESC);
CREATE INDEX idx_threads_provider ON threads(account_id, provider_thread_id);

CREATE TABLE context_bindings (
  id INTEGER PRIMARY KEY,
  workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  source_type TEXT NOT NULL
    CHECK(source_type IN ('common_source','message','thread','project','file')),
  source_id TEXT NOT NULL,
  permission TEXT NOT NULL DEFAULT 'allow'
    CHECK(permission IN ('allow','deny')),
  valid_until INTEGER,
  created_at INTEGER NOT NULL,
  UNIQUE(workspace_id, source_type, source_id)
);
CREATE INDEX idx_context_bindings_lookup
  ON context_bindings(workspace_id, source_type, permission);

CREATE TABLE context_events (
  id INTEGER PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id INTEGER NOT NULL,
  before_workspace_id INTEGER,
  after_workspace_id INTEGER,
  before_project_id INTEGER,
  after_project_id INTEGER,
  reason TEXT NOT NULL,
  actor TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE reply_runs (
  id INTEGER PRIMARY KEY,
  source_message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE RESTRICT,
  thread_id INTEGER REFERENCES threads(id) ON DELETE RESTRICT,
  workspace_id INTEGER REFERENCES workspaces(id) ON DELETE RESTRICT,
  project_id INTEGER REFERENCES projects(id) ON DELETE RESTRICT,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  agent_kind TEXT NOT NULL,
  risk_level TEXT NOT NULL,
  review_status TEXT NOT NULL DEFAULT 'generated'
    CHECK(review_status IN ('generated','accepted','edited','discarded','sent')),
  input_chars INTEGER NOT NULL,
  output_chars INTEGER NOT NULL,
  estimated_input_tokens INTEGER,
  estimated_output_tokens INTEGER,
  created_at INTEGER NOT NULL
);

CREATE TABLE reply_run_sources (
  reply_run_id INTEGER NOT NULL REFERENCES reply_runs(id) ON DELETE CASCADE,
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  relative_path TEXT,
  line_number INTEGER,
  fact_status TEXT NOT NULL DEFAULT 'confirmed'
    CHECK(fact_status IN ('confirmed','stale','unknown')),
  PRIMARY KEY(reply_run_id, source_type, source_id)
);
```

### 3.3 初期移行

- 既存メールは`threads`を作成し、同じアカウント・同じMessage-ID関係から候補を生成する。
- 顧客・ワークスペースが未確定の既存メールは`context_status='unassigned'`とする。
- 既存のAIプロジェクトは顧客ワークスペースへ自動移行しない。利用者が明示的に関連付ける。
- 共通のSample資料は`common_source`として登録し、全顧客から参照可能にする。
- 自動推定だけで`confirmed`へ変更しない。初回は`candidate`または`needs_review`で止める。

## 4. スレッド解決

優先順位は次のとおり。

1. A-mail内部`thread_id`
2. 同一アカウントのプロバイダースレッドID
3. `In-Reply-To`が既存`message_id`に一致
4. `References`に既存`message_id`が含まれる
5. 件名は候補検索だけに使用し、確定キーにはしない

スレッド候補が複数ある場合は自動確定せず、利用者に候補を表示する。新規メールはコンテキスト未設定で開始し、返信または保存時にワークスペースを選択する。

返信・全員返信は元スレッドの`thread_id`、`workspace_id`、`project_id`を継承する。転送は参照元を表示するが、新規スレッド・未確定コンテキストを既定にする。

## 5. コンテキスト解決と境界チェック

### 5.1 解決順序

```text
受信message_id
  → thread_id
  → workspace_id / project_id
  → allowed context bindings
  → FTS5検索（workspace_idで必ず絞る）
  → 共通資料 + 顧客資料 + 案件資料
  → context snapshot
```

メールアドレスは候補表示に利用できるが、確定判定には利用しない。

### 5.2 不変条件

- 返信生成に渡すメールは、元スレッドまたは同一`workspace_id`に属するものだけ。
- `workspace_id`が異なるメール・ファイルは、検索結果から除外する。
- `workspace_id`が未確定の場合、共通資料のみ利用可能。
- `context_status='conflict'`のスレッドは顧客固有情報を使わず、共通資料のみ、または手動確認とする。
- `project_id`は同じワークスペース内のものだけ利用可能。
- 参照元が`stale`または`unknown`の場合、返信案に断定表現を作らず、未確認事項として表示する。
- 入力境界検査に失敗した場合、AI呼び出しを行わない。

### 5.3 AI入力構造

返信生成には全文検索結果をそのまま渡さず、次の根拠カードへ変換する。

```json
{
  "source_type": "common_source|message|project_file",
  "source_id": "message:441",
  "workspace_id": 3,
  "project_id": 12,
  "path": "Sample/サンプルサービス企画書.docx",
  "fact": "スタータープランは5名まで、月額9,800円",
  "fact_status": "confirmed",
  "valid_until": null
}
```

## 6. AIルーティング

### 6.1 エージェント種別

| agent_kind | 用途 | 外部CLI |
|---|---|---|
| `reply_local` | 定型問い合わせ、短い返信、日程調整 | なし |
| `reply_contextual` | 顧客・案件・過去メールを参照する返信 | なし |
| `research_codex` | 仕様、実装、Git履歴の確認 | Codex CLI 1回まで |
| `research_claude` | 複数ファイルの設計整理 | Claude Code 1回まで、手動起動 |
| `manual_required` | 顧客不明、契約・金額・高リスク | 返信生成を停止 |

外部CLIの出力は返信文ではなく、出典付きの調査カードとして取り込む。外部CLIにメール送信権限、添付権限、DB書込み権限を与えない。

### 6.2 ルーティング規則

- `workspace_id`確定、共通資料のみで回答可能: `reply_local`
- 顧客固有の過去メール・資料が必要: `reply_contextual`
- 現行コードや仕様差分の確認が必要: `research_codex`
- 設計判断・複数ファイルの整理が必要: `research_claude`
- 金額、契約、納期、個別約束に未確認事項がある: `manual_required`
- `context_status='conflict'`または顧客・案件が未確定: `manual_required`

## 7. トークン・検索予算

初期値。実測後に設定へ昇格する。

| 項目 | 上限 |
|---|---:|
| 受信メール本文 | 4,000文字 |
| 関連メール | 5通 |
| 1関連メール本文 | 800文字 |
| プロジェクト出典 | 8件 |
| 1出典抜粋 | 1,000文字 |
| Codex/Claude Code実行 | 返信1件につき1回 |
| 返信生成リトライ | 1回 |
| 1返信の総入力文字数 | 16,000文字 |

重要な受信本文、スレッド、料金・契約事実を予算超過で削った場合は、`truncated=true`かつ`needs_review=true`にする。タイムアウトなど一時障害以外の自動リトライや、GeminiからOpenAI/Codex/Claude Codeへの無断フェイルオーバーは行わない。

予算超過時は追加検索を止め、UIに「情報不足／参照上限」と表示する。リクエストごとに入力文字数、出力文字数、推定トークン、レイテンシ、使用ソース数を記録する。入力本文・APIキー・CLI全出力を通常ログへ保存しない。

## 8. UI/IPC仕様

### 8.1 追加IPC

```text
list_customers() -> Customer[]
create_workspace(customer_id, name) -> Workspace
list_workspaces() -> Workspace[]
resolve_thread_context(message_id) -> ContextResolution
confirm_thread_context(thread_id, workspace_id, project_id, reason) -> Thread
search_context_sources(workspace_id, project_id, query, limit) -> ContextSource[]
generate_reply(req: ReplyContextReq) -> ReplyDraftWithSources
record_reply_review(reply_run_id, status) -> ()
```

既存`ai_generate_reply`は直ちに削除せず、`generate_reply`内部へ委譲する互換層を置く。旧APIからの呼び出しは、元メールのコンテキストが`confirmed`または共通資料のみの場合に限る。

### 8.2 返信画面

返信生成ボタンの前に以下を表示する。

```text
顧客: 株式会社A       [変更]
案件: 導入2026         [変更]
対象スレッド: thread_0182
参照: 過去メール3通 / 共通資料1件 / 顧客資料2件
状態: 確定
```

変更後は既存プレビューを破棄し、再生成を要求する。生成結果には「使用した出典」「未確認事項」「AI担当」「推定リスク」を表示する。

## 9. 送信前安全策

- 生成結果は本文へ自動反映しない。プレビューのまま保持する。
- 顧客・案件・スレッドが変更されたら、古い`reply_run`を無効化する。
- 送信時に`reply_run`の`workspace_id`と下書きのコンテキストが一致するか検証する。
- 未確認の金額、契約、納期、個別約束を含む場合は送信確認を必須にする。
- AI生成結果が送信を直接起動するIPCを持たないようにする。
- 送信後は`review_status='sent'`へ更新し、送信された外部Message-IDを記録する。

## 10. 監査・プライバシー

保存するもの:

- 返信生成日時
- agent/provider/model
- workspace/project/thread/messageのID
- 使用した出典IDと版
- 入出力の文字数・推定トークン・レイテンシ
- 利用者のレビュー状態

保存しないもの:

- APIキー
- CLI認証情報
- 顧客本文の無制限な複製
- 外部CLIの全出力

顧客紐付け変更、ワークスペースの許可変更、返信破棄・送信は`context_events`または`reply_runs`へ記録する。

## 11. テスト仕様

### DB/移行

- v4既存DBから新スキーマへ移行できる。
- 既存メールの本文・RFC Message-ID・Referencesが変わらない。
- 顧客削除でメール本文が連鎖削除されない。
- 同じ外部Message-IDが別アカウントにあっても衝突しない。

### スレッド

- In-Reply-Toで正しいthread_idへ継承される。
- Referencesが欠落したメールは自動確定しない。
- 件名だけが同じ別顧客メールは同一スレッドにならない。
- 転送は元顧客を自動継承しない。

### 境界

- 顧客Aの返信生成に顧客Bのメールが入らない。
- project_idが別workspaceの場合、入力前に拒否される。
- workspace未確定時は共通資料だけが使われる。
- stale/unknown情報は未確認として表示される。

### AI/CLI

- 定型問い合わせで外部CLIが起動しない。
- 仕様差分だけCodexへ渡り、顧客メール本文が不要に渡らない。
- Claude Code/Codexの結果から自動送信・自動添付・ファイル変更が起きない。
- トークン予算超過時に呼び出しが停止する。
- 生成結果に使用出典と未確認事項が付く。

### UI/送信

- 顧客変更後に旧プレビューが無効化される。
- 送信直前のworkspace不一致で送信が拒否される。
- 利用者が破棄したドラフトを送信できない。
- AIオフまたは本文共有オフでは本文が外部へ送信されない。

## 12. 段階的実装順

### Phase 1: 内部IDとスレッド境界

`threads`、既存messagesへのthread_id、Message-ID関係の解決、返信継承、未確定状態を実装する。

### Phase 2: Customer/Workspace

customers、workspaces、projects、UIでの明示選択、workspace単位のFTS5絞り込みを実装する。

### Phase 3: 出典・スナップショット

context_bindings、reply_runs、出典表示、未確認事項、境界チェックを実装する。

### Phase 4: エージェントルーティング

定型／文脈付き／調査付き／手動確認の分類と、Codex・Claude Codeの手動調査連携を追加する。

### Phase 5: 監査と評価

20から30通のテストメールで、顧客混在、修正量、レイテンシ、トークン量、外部CLI呼び出し率を計測する。

## 13. 未決定事項

- GmailのプロバイダースレッドIDをどのAPI/IMAP拡張で安定取得するか
- 顧客・案件の初回作成UI
- 共通資料の版管理方法
- トークン上限を文字数からモデル別トークンへ移す時期
- Claude Codeを手動UIから起動する具体的な接続方式
- 監査ログの保持期間

これらは実装前に、テスト用メールとA-mailの実環境で確認する。未確認の項目を自動化仕様として確定しない。
