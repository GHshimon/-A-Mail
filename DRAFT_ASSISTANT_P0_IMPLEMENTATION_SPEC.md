# A-mail 確認型・本人文体学習 下書きアシスタント P0実装仕様書

- 版: 2.1 / 2026-09-10
- 状態: オーナー承認済みレビューに基づく実装指示。実装完了・実機検証済みを意味しない。
- 対象: `/Users/shimon/project/A-mail` の現在の共有checkout。既存の未コミット変更を保持する。
- 旧仕様: [CONTEXT_REPLY_IMPLEMENTATION_SPEC.md](CONTEXT_REPLY_IMPLEMENTATION_SPEC.md)。本書を優先する。
- 追加仕様: [MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md](MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md)。文体学習、AI認証、Gmail認証、起動中限定の実行管理は追加仕様を優先する。
- 製品目的: **本人が確認して送る下書きの、事実確認・書き直しの負担を減らす**。
- 承認: 2026-09-09「修正案を承認します。実装仕様書を作成しterraサブエージェントに実装依頼して」。

## 1. 対象と対象外

今回のP0は、下書き保存・再開と送信安全性、顧客境界、根拠と回答漏れ、本人が任意に実行する文体プロファイル学習、自動下書き作業一覧・利用量制限。モデルや外部AIの数を増やすことは目的にしない。

対象外は完全自動送信、既存メールの無断一括AI処理、モデルのファインチューニング、外部CRM同期、外部CLIの自動起動、アプリ終了中の常駐ジョブ、メールアカウントや本番設定の変更、実メール送信テスト。

既存のAI有効化・本文共有許可・登録プロジェクトの手動操作を維持する。AI無効でも手動メールと下書き保存は使える。自動下書きは明示オプトインで移行時false。文体学習は常時ON/OFFではなく本人が100件/200件を選んで開始する都度同意のjobとする。既存メールは自動下書き対象にしないが、選択accountの送信済みメールは文体学習ボタンからだけ一時解析できる。

## 2. 旧仕様からの変更

1. 顧客未確定は生成全面停止ではない。**現在の受信メール＋明示承認された共通資料**のみで案を作り、情報不足を表示する。他スレッドの顧客メールは禁止。
2. スレッド一致は関連付けの証拠であり、資料の開示権限ではない。To/Cc/Bcc・送信名義変更は別途検知する。
3. Sampleフォルダや登録ルートを自動で共通公開しない。資料の利用範囲は明示登録する。
4. 読めた出典をconfirmedにしない。既定はunknown。版が変われば確認を失効する。
5. 新しい未編集のAI下書きは編集欄に表示してよいが、AI起因で送信しない。人の文章の自動上書きは禁止。再生成は別候補として保持する。
6. 文体学習は送信済み直近100件/200件から抽象プロファイルを作る任意実行方式。生メールを追加コーパスとして保存せず、モデル自体が自動的に学習・記憶するという説明はしない。
7. 評価・回帰テストを最初から実装する。旧仕様の最終Phaseまで後回しにしない。

## 3. データの境界

| 情報 | 境界 | 既定 |
|---|---|---|
| 顧客・案件の事実 | account_id + workspace_id | 他workspaceへ持ち出さない |
| 案件内の会話 | 内部thread_id + workspace許可 | 受信ヘッダだけで許可を拡大しない |
| 共通製品資料 | アカウント所有者が共通利用を明示許可 | 全登録資料を自動許可しない |
| 本人の定型文体 | 送信account_id | 顧客の価格・固有名詞を保存しない |
| 相手との言葉遣い | account_id + workspace_id | 他workspaceへ持ち出さない |

P0のworkspaceは「顧客の一つの案件」を表せる名前を持たせる。同じ顧客の別案件も必要なら別workspaceとし、初期の案件横断共有を禁止する。customer_idは独立採番する。既存ai_projectsはファイルルートであり顧客・案件そのものではない。

既存messages.idをローカル参照として使い、既存message_id TEXTはRFC Message-IDのまま維持する。UIDVALIDITY変更やキャッシュ削除で元メールが消えた場合は、別メールへ再関連付けせず下書きをstaleにする。独立した内部threadsと保存済みメッセージの識別情報を持ち、整数IDの再利用で取り違えない。accountsを跨いだ照合を禁止する。

スレッド候補は内部ID、プロバイダーID（取得済みの場合のみ）、In-Reply-To、Referencesの順に解決する。件名・アドレスだけで統合しない。矛盾はconflict。新しい参加者・別組織への返信は既存の開示確認を無効化し、確認されるまで顧客固有の過去メール・資料を渡さない。転送は未設定から開始する。

宛先変更後の再許可は明示操作「この宛先で顧客情報を使う」とし、保存済みdraftのaccount/To/Cc/Bccのハッシュとworkspaceに結び付け、確認時revisionも監査情報として記録する。本文だけの編集や生成によるrevision増加では許可を失効させず、To/Cc/Bccまたはworkspaceが変われば必ず失効する。単なる送信警告への同意、同じworkspaceの再選択、LLM判定で代替しない。未確認でも共通情報だけの下書きは作れる。

## 4. 下書き・生成・送信の状態

### 4.1 保存と再開

既存draftsを本文の正本として使う。session_id（クライアントUUID）をアカウント内で一意にし、初回保存の再実行で重複を作らない。保存はexpected_revisionによる楽観的排他。保存成功後にrevisionを増やす。本文、送信名義、To/Cc/Bcc、件名、元メール、workspace、intent、reply_runの対応を永続化する。

フロントの保存は直列化する。保存中の追加編集を保持し、応答のsession_idと現在の編集セッションが違えばUIを変更しない。画面遷移前に保存をflushし、失敗時は「未保存」を明示して下書きを維持する。作成画面とローカル下書き一覧を接続する。IMAPのDrafts一覧だけで代替しない。

表示状態: editing / queued / generating / unreviewed / needs_review / stale / sending / sent / failed / unknown。discardedは復元可能な論理破棄とし、通常一覧から除外する。本文の取り消し・以前の生成候補への復帰を可能にする。

### 4.2 生成の競合

生成要求はdraft_id + expected_revisionに固定し、元メール・宛先・情報源の版・文体の版を記録する。応答時に再照合し、変化があればstaleとして扱い、現在の編集欄を上書きしない。再生成中も既存本文を保持する。

空の未編集下書きを含め、生成本文は常に候補として別保存する。利用者が「確認して本文に反映」を押したときだけ、expected_revision・スナップショット・情報源の現在版を再検証して適用する。生成完了だけで本文を変更せず、単純な先頭への追加も既定操作にしない。

### 4.3 人による送信と重複防止

通常の送信ボタン・Cmd/Ctrl+Enterは同じ関数・同じin-flightガードを使う。クリック時点の保存を完了してからprepareを呼ぶ。prepareは本文・件名・account・宛先・draft revision・元メール・context version・source hashes・reply_runを固定した有効期限付きトークンを返す。生成完了・保存完了・ジョブ完了からsendを呼ばない。

警告がなければ、その明示送信操作をレビュー確定として扱う。警告があれば具体的な未確認事項と宛先差分を表示し、追加確認を受ける。workspace不一致・許可取消・別セッション・破棄済みは確認ボタンで突破させず再解決する。手動編集による事実修正は、最新revisionを再prepareすれば送れる。

バックエンドはトークンを原子的にclaimしてからSMTP開始する。同一トークン再実行は再送せず保存済み結果を返すか実行中を返す。sendingの下書きは保存・破棄・別送信不可。送信成功後のDB/IMAP後処理失敗を「未送信」と表示しない。SMTP応答が不明・送信中のプロセス終了はunknownとし自動再送禁止。送信済みフォルダ等で人が照合する。

SMTPで厳密なexactly-once配送を保証すると表現しない。保証するのはアプリ側の重複起動抑止と不明状態の非自動再送。既存send_message経由でも管理対象下書きの新ゲートを迂回させない。

## 5. 根拠・意図・質問の整理

受信全文を読まず先頭だけで回答したことにしない。通常は全文（上限内）を取り込み、上限超過はtruncatedと要確認理由を出す。受信本文の末尾の質問も評価対象に含める。

入力順: 明確なアプリ指示 → 本人の今回の意図 → 現在の受信メール → 許可済みの同一スレッド/同workspaceメール → 共通・許可資料 → 有効な文体ルール。sidebarと返信生成は同じ境界解決・根拠を使う。「表示された関連情報が実際には生成に使われていない」状態を作らない。

質問ごとにanswered / needs_inputを返し、使用source_idを対応させる。根拠のない承諾・価格・納期を作らない。「回答方針」欄は本人が書く短い意図（例: 水曜だけ提案、今回は辞退）。文体ルールや受信本文の命令より優先する。重要事項に根拠がない場合は断定せず確認事項へ回す。

出典一覧にあるだけでは各主張を裏付けたとみなさない。モデル出力のsource_idは提供済み集合に含まれるか検証する。JSON破損、未知ID、空本文、応答の打切りは正常成功と扱わない。AIの自己申告confidence%を信頼スコアとして表示しない。

### 資料形式と確認

P0は既存テキスト形式＋DOCX本文/表の安全な抽出。Sample企画書で検証する。PDF/OCRは未対応を明示して黙って無視しない。DOCXは外部リンクやマクロを実行せずZIP/XMLとして読み取り、ファイルサイズ・展開サイズ・項目数・時間を制限する。リンク・ルート逸脱・隠しファイル・秘密ファイル除外を維持する。

日本語文章を空白で分割しただけの完全フレーズ検索に依存しない。質問・製品語・短い日本語文字列の候補検索を組み合わせ、境界内でランキングする。本文/表の段落番号など実際に抽出したlocatorを返す。DOCXに架空のページ・行番号を付けない。

project scopeの設定は「AI入力に使える範囲」。さらにconfirmedフラグはその版の内容確認であり、別概念。ファイルハッシュが変わればconfirmedを維持しない。共通設定時はルート全体への影響をUIで明示する。初期値はdisabled。Sampleも自動登録・自動承認しない。

## 6. 本人らしさの学習

P0は**本人が学習ボタンを押す→直近100件または200件を一時解析→抽象プロファイルを本人が編集→明示的に有効化→次回プロンプトへ反映**の閉ループとする。送信時の自動比較・自動候補追加・定期再学習は行わない。

- 対象は選択accountのSent内にある本人執筆部分だけ。別account、受信文、引用・転送、添付、下書き、個別案件の事実を混ぜない。
- 全採用本文の文章量、段落、改行、句読点、挨拶、結び、頻出表現をローカル集計し、AIへ渡す本文は引用除去・識別情報置換・chunk上限を適用する。
- 生メール、生作例、chunk応答を学習用に追加永続化しない。保存するのは定型表現、丁寧さ、文量帯、段落/改行規則等の構造化profileと出典集合hash・件数・期間だけ。
- 作成直後はdraft profile。本人が内容を編集・プレビューして有効化したactive版だけ生成に使う。版管理、rollback、無効化、削除を可能にする。
- profileに料金、承諾、顧客名、メール、URL、電話、日付、注文番号等を残さない。文体は根拠と今回の意図を上書きできない。
- account共通を基本とし、workspace限定overrideを将来追加しても他workspace・他accountへfallbackしない。

具体的な抽出、redaction、初期予算、schema、IPC、UI、受入条件は追加仕様の第2〜5節と第9〜10節を正とする。既存`style_rules`は移行互換の手動補正として残せるが、常時学習の正本にはしない。

## 7. 自動下書き・利用量

アプリ起動中のみ、通常同期で新しく入ったinboxの返信候補を処理する。オプトイン時点のhigh-watermarkと時刻を保存し、既存キャッシュ/過去メールの初回同期を対象にしない。通知・宣伝・自分の送信・自動応答/ループ・明らかな一斉配信を除外する。判別不能なものは手動生成で扱う。

1アプリ1生成ジョブ、初期1日20件（設定可能1〜100）の自動生成上限。手動生成も合計の実行回数・使用量を記録する。日次自動上限は永続化したUTC日付で判定しUIに基準を表示する。上限超過は待機理由を表示し、無断で別プロバイダーへ切替えない。

同じaccount/sourceの識別子/本文ハッシュ/context version/profile version/intent/recipientsの組合せをdedupe keyにする。新着で既存下書きが古くなればstaleにし、人の編集中下書きを置き換えない。中断ジョブはfailed/staleで回復し、再起動だけで再生成・再課金しない。自動再試行はP0では0回、手動再試行可能。

予算初期値: 受信本文8000文字、過去メール最大5通/計4000文字、資料最大8件/計6000文字、文体1200文字、指示/意図を含む総入力20000文字上限。モデルごとの正確なトークン計数が未対応なら文字数予算と推定値として明示し、厳密なトークン上限とは呼ばない。APIの実測usageは取れる場合に別保存。出力上限は通常1500/長文2400tokens、timeout45秒を初期値とし、打切りを検出する。元本文/重要情報を削ったら必ずneeds_review。期限・上限はローカル時計を使う。

## 8. IPC契約（UIとRustの共通仕様）

新IPCは`assistant_*`とし、**すべてのコマンドは引数を`{ req: ... }`で包む**。DTOのキーはsnake_case。既存APIは互換維持するが安全境界を迂回させない。数値IDはi64/number、run_id/session_id/tokenはUUID等の文字列、日時はUnix秒。

下記の`DraftInput`は既存の型（id/account_id/to/cc/bcc/subject/body_text/body_html/in_reply_to/references）。任意キーは欠損を許容し、戻り値のnullableキーはnullを返す。

```ts
type DraftStatus = 'editing'|'queued'|'generating'|'unreviewed'|'needs_review'|
  'stale'|'sending'|'sent'|'failed'|'unknown'|'discarded';
type ContextStatus = 'unassigned'|'confirmed'|'common_only'|'conflict'|'needs_review';
type FactStatus = 'confirmed'|'unknown'|'stale';
interface Workspace {
  id:number; account_id:number; customer_id:number; customer_name:string;
  name:string; context_version:number;
}
interface ReplyContext {
  source_message_id:number; thread_id:number; workspace_id:number|null;
  context_version:number; status:ContextStatus; review_reasons:string[];
}
interface SourceCard {
  id:string; kind:'message'|'project_file'; label:string;
  relative_path:string|null; locator:string|null; snippet:string; version:string;
  workspace_id:number|null; fact_status:FactStatus;
}
interface ReplyQuestion {
  id:string; text:string; status:'answered'|'needs_input'; source_ids:string[];
}
interface ReplyRun {
  id:string; draft_id:number; source_message_id:number|null; workspace_id:number|null;
  context_version:number; profile_version:number; snapshot_hash:string;
  generated_text:string; questions:ReplyQuestion[]; sources:SourceCard[];
  review_reasons:string[]; status:string; truncated:boolean;
  provider:string; model:string; input_chars:number;
  estimated_input_tokens:number; actual_input_tokens:number|null;
  actual_output_tokens:number|null; created_at:number;
}
interface DraftRecord {
  draft:DraftInput & {id:number}; session_id:string; revision:number;
  source_message_id:number|null; workspace_id:number|null; intent:string;
  reply_run_id:string|null; reply:ReplyRun|null; status:DraftStatus;
  review_reasons:string[]; updated_at:number;
}
interface AssistantSettings {
  account_id:number; auto_draft_enabled:boolean; active_style_profile_id:number|null;
  daily_generation_limit:number; auto_generated_today:number; quota_day:string;
}
interface ProjectScope {
  account_id:number; project_id:number; workspace_id:number|null;
  scope:'disabled'|'common'|'workspace'; confirmed:boolean;
}
interface StyleRule {
  id:number; account_id:number; workspace_id:number|null;
  from_phrase:string; to_phrase:string; status:'pending'|'active'|'rejected';
  created_at:number;
}
```

文体学習の新しい`StyleProfile` / `StyleProfileJob` DTOとIPCは [MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md](MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md#5-ipc契約) を正とする。上記`StyleRule` APIは既存データ互換用で、新規UIの主経路にしない。

| コマンド | req | 戻り値 |
|---|---|---|
| assistant_list_workspaces | account_id | Workspace[] |
| assistant_create_workspace | account_id, customer_name, name | Workspace |
| assistant_get_context | source_message_id | ReplyContext |
| assistant_bind_context | source_message_id, workspace_id:null可 | ReplyContext |
| assistant_list_project_scopes | account_id | ProjectScope[] |
| assistant_set_project_scope | ProjectScopeの全フィールド | ProjectScope |
| assistant_list_drafts | account_id, include_discarded?:boolean | DraftRecord[]（新しい順、最大100） |
| assistant_get_draft | draft_id | DraftRecord |
| assistant_save_draft | draft:DraftInput, session_id, expected_revision:number|null, source_message_id:number|null, workspace_id:number|null, intent:string, reply_run_id:string|null | DraftRecord |
| assistant_confirm_recipients | draft_id, expected_revision | DraftRecord（現在の宛先範囲の開示許可を明示確定） |
| assistant_discard_draft | draft_id, expected_revision | DraftRecord |
| assistant_restore_draft | draft_id, expected_revision | DraftRecord |
| assistant_generate_reply | draft_id, expected_revision, force?:boolean | {draft:DraftRecord,run:ReplyRun,applied:boolean} |
| assistant_apply_reply | draft_id, run_id, expected_revision | DraftRecord |
| assistant_prepare_send | draft_id, expected_revision | {token:string,review_reasons:string[],needs_confirmation:boolean} |
| assistant_send_draft | token:string, confirm_warnings:boolean | {draft_id:number,state:'sent'|'unknown',message_id:string|null,warnings:string[]} |
| assistant_get_settings | account_id | AssistantSettings |
| assistant_update_settings | account_id, auto_draft_enabled, daily_generation_limit | AssistantSettings |
| assistant_process_queue | account_id | {processed:number,pending:number,reason:string|null} |
| assistant_list_style_rules | account_id | StyleRule[] |
| assistant_save_style_rule | account_id, id?:number, workspace_id:number|null, from_phrase, to_phrase, status | StyleRule |
| assistant_delete_style_rule | account_id, id:number | void |

get_context/bind_context/save_draft/prepare/sendではaccountとworkspaceの所有関係をDBで再検証する。req内のsource_message_idを信頼して他アカウントの内容を取らない。IPCエラーは既存AppErrorを使い、Conflict相当はBadInput等でもよいが人が理解できる日本語理由を返す。UIだけの境界チェックは禁止。

元メールのない新規作成は手動編集・保存・送信を利用できる。P0の返信生成はsource_message_id必須とし、nullの場合に過去メールを推測検索しない。

## 9. 永続化・移行の要件

既存マイグレーションを破壊せず次版を追加する。具体的DDLはRust担当が既存schemaと照合するが、最低限以下を保存する。

- customers / workspaces: account所有、案件名、context_version、updated_at。
- threads / message_thread_links: account、元識別情報、workspace、status、participant fingerprint。
- project_scopes / source_confirmations: account、root ID、workspace/common/disabled、確認したファイル版。
- draft_meta: draft_id、unique session_id、revision、source identity、workspace、intent、run_id、状態、human_edited。
- reply_runs / sources: 第8節DTOに加え、生成時のdraft revision・recipients hash・source/body hash・context version。
- send_attempts: token、draft revision、snapshot hash、payload、expires、status、message_id、安全なerror kind。
- style_profiles / style_profile_jobs: account境界、抽象プロファイル、draft/active/archive、版、同意、件数/期間/使用量。生メールID一覧や本文は保存しない。既存style_rulesは互換用。
- assistant_settings / generation_jobs: オプトイン、high-watermark、dedupe key、日時、回数、ジョブ状態。

原子性: 保存/トークンclaim/生成ジョブclaimはDBトランザクション。ネットワーク待ち中にDB Mutexを保持しない。foreign keyによる顧客混在を防ぐか同等のトランザクション検証をテストする。削除済みsourceはstaleとし、キャッシュ同期をRESTRICT FKで壊さない。既存draftは読み出せるようmetadataを遅延補完し、AI由来と偽らない。

監査ログはID・版・遷移・時刻・エラー種別・使用量だけ。APIキー/認証情報/本文/CLI全出力を記録しない。標準メールデータ以上の生本文の無期限コピーを作らない。引用返信・他人の文面を学習候補に使わず、署名は本文特徴と分離して本人が確認できるprofile項目にする。

## 10. UI/UX

1. ローカル下書き一覧を常に到達可能にする。未確認/要確認/編集中/失敗が区別でき、再開・破棄・復元できる。
2. 作成欄に送信名義、To/Cc/Bcc、workspace、回答方針を表示。既定文体は「いつもの自分」。従来トーン/長さは上書き指定として残す。
3. 返信案を主役、右側は「要確認事項」「質問への回答状況」「根拠」の順。元メールを折畳み/分割表示し、画面遷移で編集中本文を失わない。
4. CLIは「詳細調査」に折り畳み、既定で閉じる。モデル名/生ID/トークン詳細も詳細欄へ。
5. 資料未対応・未確認・古い・本文省略・予算上限を明示。読み取れていないのに「参照済み」と表示しない。
6. 文体設定に「直近100件/200件から学習」、対象確認、進捗、draft profileの編集/プレビュー/有効化、旧版/rollback/削除の導線。何を一時送信し何を保存するかを分けて表示する。
7. 自動下書きのオンオフと日次件数を表示し、初期falseをUIで勝手にtrueへ変更しない。文体学習はオンオフではなく本人操作時だけ走るjobとして表示する。
8. 固定の「最終同期2分前」を除去し、取得できる実値か「未確認」を表示する。キーボード操作・ラベル・狭幅でも送信と確認事項が見えること。
9. ブラウザデモは架空のデータだけ。デモ中のSMTP/外部AI呼出しを発生させない。デモ保存と本番保存の表示を区別する。

## 11. 受け入れテスト

全て架空データ、インメモリ/一時DB、偽transportで実施。liveのメールDB・Keychain・APIキーを使わない。

| ID | 合格条件 |
|---|---|
| D01 | 初回save再実行が同じdraftになり、古いrevisionの保存は拒否される |
| D02 | 保存後の再読込で本文/宛先/元メール/方針/生成状態が一致する |
| D03 | 保存失敗を表示し、画面遷移/関連メール閲覧で本文が消えない |
| D04 | 生成中に別draft/宛先/本文へ変更しても旧応答で上書きされない |
| S01 | 二重クリック/ショートカット/IPC再送でSMTP mock呼出しは1回 |
| S02 | prepare後のrevision/宛先/workspace/source版変更で旧tokenは拒否される |
| S03 | SMTP後処理失敗は送信失敗に偽装せず、unknownは自動再送されない |
| C01 | 同じ製品名の顧客A/Bでも取得本文・資料・文体が交差しない |
| C02 | 他account/source/workspaceの組合せ、許可外project、失効許可が拒否される |
| C03 | 未確定/conflictでも共通資料のみで案を作れ、顧客資料は渡らない |
| C04 | CC追加/転送/References矛盾で開示確認が失効する |
| R01 | DOCX本文と表が抽出され、Sampleの事実を検索できる |
| R02 | 日本語長文の末尾質問が欠落しない、切詰めは明示される |
| R03 | 未知source_id/壊れたJSON/打切りを正常な根拠付き回答とみなさない |
| R04 | 料金・日程の根拠なし回答はneeds_input。出典と回答項目が対応する |
| L01 | 学習ボタンを押さない限りjobは作られず、100/200件以外を拒否する |
| L02 | 本人が編集してactiveにしたprofileだけ次回入力に反映し、rollback/削除後は指定版へ収束する |
| L03 | 価格/日付/顧客固有情報/引用文/相手文/他account/他workspaceがprofileに混ざらず、生本文を追加永続化しない |
| A01 | 初期/既存メールは自動生成されず、オプトイン後の対象新着だけ処理される |
| A02 | 重複同期・再起動で再生成せず、同時実行1/日次上限を守る |
| A03 | 自動生成の全経路に送信/外部CLI起動が存在しない |
| U01 | UIで一覧→再開→意図→案→根拠→送信確認に到達できる |
| U02 | 1280px前後の画面で主操作がCLIに押し出されない |

P0リリース条件は安全・保存テスト全件通過、型check/build、既存Rustテストの非退行。文章品質は別に本人評価を残し、実測前に精度や時短効果の達成を主張しない。初期評価は架空の問い合わせ・日程・資料確認・障害対応と顧客混在ケース。記録する指標は事実修正、回答漏れ、文体修正、本人の好み、確認時間、入力/出力使用量。比較データは同一スレッドの学習/評価混入を避ける。

初期の架空評価ケースとオフライン評価器は [evals/README.md](evals/README.md) を参照。評価器のテスト成功と、実際のAI返信品質の合格を区別する。

## 12. Terraへの実装分担と統合

- **Terra backend**: `src-tauri/`のみ。migration、下書き/送信、境界、DOCX、質問/根拠、学習、queue、IPC、Rustテストを担当。既存プロバイダー設定は変更しない。
- **Terra frontend**: `src/`とフロント検証設定のみ。第8節契約に接続するUI/store、デモ、状態競合対策、設定/作業一覧、フロントテストを担当。Rustを編集しない。
- **親エージェント**: 本仕様と旧仕様の案内、decision log、架空の評価fixture、契約整合・統合レビュー・テスト。担当作業完了前は共有のソースhunkを書き換えない。

新branch/worktree/commit/push/deployは作業に含めない。依存が必要なときは最小の一般配布ライブラリを選び、lockfile変更は担当側で直列化する。実データ上のmigrationとメール/API実機試験は別の明示操作とする。

完了報告は「実装したもの」「実行したテスト」「未実装」「実機未確認」を分ける。全P0が未完了なら部分完了として報告し、仕様の必須事項を黙って削らない。

## 13. 参考

- 現在の実装: `src/components/compose/ComposeWindow.tsx`, `src/components/ai/ReplyDraftButton.tsx`, `src-tauri/src/commands/ai_gen_cmd.rs`, `src-tauri/src/ai_projects.rs`。
- [OpenAI公式 Prompt engineering](https://developers.openai.com/api/docs/guides/prompt-engineering): 指示・文脈・作例の分離。採用するモデル/価格を変更する根拠としては使わない。
- [RFC 5322 §3.6.4](https://www.rfc-editor.org/rfc/rfc5322.html#section-3.6.4): Message-ID/In-Reply-To/Referencesはメッセージ関係の識別。A-mailの開示許可は別に管理する。
- [MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md](MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md): オンデマンド文体プロファイル、Gmail OAuth、利用者サブスクリプションAI、起動中限定の実装境界。
