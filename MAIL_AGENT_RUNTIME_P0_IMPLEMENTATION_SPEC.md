# A-mail メールエージェント基盤 P0 実装仕様書

- 版: 1.1 / 2026-09-10
- 状態: オーナー承認済み要件を実装可能な境界へ整理した追加仕様。外部認証・実機受入済みを意味しない。
- 優先関係: 本書は `DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md` の文体学習、AI認証、Gmail認証、実行ライフサイクルを置き換える。顧客・案件境界、下書き候補、送信ゲートは同書を継承する。
- 製品原則: **メールを受け取ったA-mailが必要な文脈だけを組み立て、AIは返信候補だけを返し、本人が確認して送る。**

## 1. P0で固定する要件

1. 文体学習は利用者が設定画面の「文体を学習」を押した時だけ開始する。自動・定期・送信ごとの再学習はしない。
2. 学習対象は選択中の送信アカウントの送信済みメールから、本人が100件または200件を選ぶ。別account、受信文、下書き、添付を混ぜない。
3. 生メールの学習用複製やファインチューニングは行わない。既存メールから一時的に特徴を抽出し、保存するのは本人が確認・編集できる抽象プロファイルだけとする。
4. Gmailの主接続動線は「Googleで接続」。システムブラウザを使うOAuth 2.0 Authorization Code + PKCEとし、パスワード入力を求めない。
5. AIの主接続動線は「ChatGPTで接続」。A-mail共通APIキーやサービス負担のAPI課金は使わず、対応する利用者自身のChatGPT/Codex利用枠を使う。
6. AIにSMTP、IMAP書込、送信IPC、外部CLI、shell、ファイル編集の権限を渡さない。AIの出力は常に候補で、本文への反映と送信は別の本人操作とする。
7. 同期、受信トリガー、生成、学習はA-mailプロセス起動中だけ動く。常駐デーモン、LaunchAgent、ログイン項目、終了後の予約実行は作らない。
8. Gmail、AI、文体学習はそれぞれ独立して接続・同意・解除できる。暗黙の有効化や代替プロバイダーへの自動切替えをしない。

## 2. 利用者フロー

### 2.1 初回セットアップ

1. 「Googleで接続」を押す。
2. A-mailはランダムなloopbackポートを開き、stateとPKCE S256を生成してシステムブラウザを開く。WebView内ログインは禁止する。
3. Googleの同意画面でメールの読み書き権限と利用目的を表示する。A-mailはcallbackのstate、redirect URI、期限を検証してからcodeを交換する。
4. 取得したrefresh tokenはmacOS Keychain、短命access tokenはプロセスメモリだけに保持する。接続確認に成功してからaccountを有効化する。
5. 「ChatGPTで接続」を押す。A-mailが起動したCodex App Serverのaccount APIでChatGPT loginを開始し、表示された公式URLをシステムブラウザで本人が開いて認証する。返信生成は同じApp Server threadではなく、後述の非永続processを使う。
6. 接続後はプラン種別、接続状態、取得可能な範囲の利用上限だけを表示する。A-mailがChatGPTのパスワードやtokenファイルを直接読む画面は作らない。
7. 自動下書きを使うかを説明付きで選ぶ。初期値はOFF。ONでも生成するのは未送信候補だけである。

Google OAuthを利用できないiCloud等は既存のアプリパスワード方式を残す。ただしGmailの新規登録画面では「詳細設定」に退避し、主動線にしない。

### 2.2 受信メールからの下書き

1. 起動中の通常同期で新しい受信メールを検出する。
2. 自動下書きがONで対象条件を満たす場合だけ、`account_id / customer_id / workspace_id / internal_thread_id / internal_message_id`を解決する。
3. A-mailが現在の受信本文、同一境界内の必要な過去メール、許可済み資料、active文体プロファイルを予算内で組み立てる。
4. AI providerには固定した入力だけを渡す。providerがメールストアを探索することは禁止する。
5. 生成結果を`reply_run`と候補本文として保存し、作業一覧に「未確認」で表示する。作成本文へ自動反映しない。
6. 本人が根拠・要確認事項・宛先・本文を確認し、「本文に反映」を押して編集する。
7. 本人が送信ボタンを押した時だけ既存のprepare/confirm/sendゲートを通る。生成完了イベントからsendを呼ぶコードパスは作らない。

自動下書きOFFでも「AIで下書き」は手動で使える。顧客境界が未確定なら現在の受信メールと明示承認済み共通資料だけを使い、過去の顧客メールは渡さない。

### 2.3 文体学習

1. 設定の「文体を学習」を押す。
2. 「直近100件」「直近200件」を選ぶ。取得可能件数が少なければ実件数を表示し、黙って範囲を広げない。
3. 対象account、件数、期間、除外件数、AIへ一時送信する情報、保存される情報を確認画面に表示する。
4. 本人が開始を押した後だけ解析する。キャンセル時は未完成プロファイルをactiveにしない。
5. 解析結果を新しいdraft版として表示し、本人が各項目を編集・無効化できるようにする。
6. 架空の短い問い合わせに適用したプレビューを表示し、「この文体を使う」を押した版だけactiveにする。
7. 旧版はrollback用に保持する。本人はいつでも編集、無効化、削除、再学習できる。再学習は必ず再度ボタン操作から始める。

## 3. 文体プロファイルの作り方

### 3.1 対象抽出

対象は選択accountのSentフォルダにあり、Fromが当該accountまたは登録済み本人aliasと一致するメールである。新しい順に候補を走査し、次を除外する。

- 下書き、送信失敗、不明状態、受信メール、添付本文、カレンダー招待、配信システムによる自動メール。
- 引用返信、転送元、相手が書いた文、長い法定注記。署名は本文から分離し、署名候補としてのみ集計する。
- 本人の文章が極端に短く、挨拶・署名を除くと特徴を持たないメール。除外理由と件数は画面に出す。
- 同一本文の重複。テンプレートは重複数を頻度として数えるが、同じ生本文を何度もAI入力へ送らない。

件数は「採用できた本人本文」の数とする。候補の走査上限は初期値400件。100/200件に届かなければ不足を表示し、別accountや受信箱で補わない。

### 3.2 ローカル前処理とトークン管理

選んだ全メールを一度にモデルへ送らない。次の二段階で処理する。

1. **ローカル集計**: 全採用本文から文字数分布、文数、段落数、1段落の行数、空行位置、句読点、箇条書き、挨拶・結び・署名、頻出n-gram候補を決定論的に集計する。
2. **限定AI抽象化**: 宛先、アドレス、URL、電話、金額、日付、注文番号、固有IDをplaceholderへ置換し、引用を除いた本人本文だけを小分けにする。各chunkから文体候補JSONを得て、最後に候補とローカル統計を統合する。

初期予算は1chunk最大12,000文字、最大10chunk、統合入力最大12,000文字、出力最大2,000tokensとする。200件すべての構造統計と頻度には反映するが、モデルへ渡す作例は頻度帯と長さ帯を層化抽出して上限内に収める。切詰め、除外、送信数は完了画面に表示する。正確なtoken数をproviderから取得できない場合は文字数と推定値を分けて表示する。

一時本文、chunk応答、生作例は追加テーブルや監査ログへ保存しない。解析中のプロセスメモリだけで扱い、完了・失敗・キャンセル・アプリ終了時に破棄する。既存の通常メールキャッシュ自体の保持方針は別であり、本機能のための複製を作らない。

### 3.3 保存する抽象プロファイル

保存対象は次の構造化データに限定する。

```ts
interface StyleProfileContent {
  greetings: Array<{ pattern: string; usage: string }>;
  closings: Array<{ pattern: string; usage: string }>;
  signature: { enabled: boolean; text: string };
  honorific_level: 'plain'|'polite'|'very_polite';
  tone_notes: string[];
  sentence_length: { median_chars: number; preferred_range: [number, number] };
  message_length: { median_chars: number; preferred_range: [number, number] };
  paragraphs: { median_count: number; blank_line_between: boolean; notes: string[] };
  line_break_rules: string[];
  punctuation_rules: string[];
  bullet_rules: string[];
  phrase_groups: {
    request: string[]; acknowledgement: string[]; apology: string[];
    decline: string[]; scheduling: string[]; follow_up: string[];
  };
  avoid_phrases: string[];
  freeform_instructions: string;
}
```

プロファイルに顧客名、相手の固有名詞、メールアドレス、URL、電話、価格、納期、日時、注文番号、個別案件の事実を保存しない。短い定型句にもそれらが含まれれば候補から除外する。文体は事実、根拠、本人の今回の回答方針を上書きできない。

account共通profileを基本とし、P0では自動的なcustomer/workspace別学習を行わない。将来のworkspace overrideは本人が明示作成した場合だけ許可し、別workspaceへfallbackしない。

## 4. 永続化

次の概念を既存migrationへ冪等に追加する。整数migration番号だけで既存系列を判定せず、`schema_migrations`と実構造検査で収束させる。

### 4.1 `style_profiles`

| 列 | 内容 |
|---|---|
| id | ローカル内部ID |
| account_id | 必須。accounts FK |
| workspace_id | P0はNULL。将来override用 |
| version | account/workspace内の単調増加 |
| status | `draft / active / archived / deleted` |
| source_count | 採用本人本文数 |
| source_candidate_count | 走査候補数 |
| source_range_start / source_range_end | 対象期間。メールID列は保存しない |
| source_set_hash | message内部IDと内容hashから作る不可逆集合hash |
| content_json | `StyleProfileContent`。schema version付き |
| provider / model | 生成元。手動のみなら`local/manual` |
| input_chars / estimated_input_tokens / actual_input_tokens / actual_output_tokens | 使用量。取得不能はNULL |
| consent_version | 確認画面の版 |
| created_at / updated_at / activated_at | Unix秒 |

同一account/workspaceでactiveは最大1件。active切替えと旧版archiveは同一transactionにする。hard delete時は内容を消し、`reply_runs`には使用したprofile id/versionとhashだけ残す。

### 4.2 `style_profile_jobs`

| 列 | 内容 |
|---|---|
| id | UUID |
| account_id | 必須 |
| requested_count | 100または200 |
| status | `queued / collecting / analyzing / awaiting_review / completed / failed / cancelled / interrupted` |
| progress_current / progress_total | UI進捗 |
| error_kind | 秘密・本文を含まない分類値 |
| result_profile_id | draft profile。未作成ならNULL |
| created_at / started_at / finished_at | Unix秒 |

ジョブはプロセス内queueだけで実行する。起動時に`queued/collecting/analyzing`を`interrupted`へ収束させ、自動再開・自動再課金しない。「もう一度実行」は本人の新操作と新job idを必要とする。

## 5. IPC契約

既存方針どおり引数は`{ req: ... }`、DTOはsnake_caseとする。

| コマンド | 主なreq | 戻り値・制約 |
|---|---|---|
| `assistant_style_learning_preview` | `account_id, requested_count` | 候補/採用見込/期間/除外内訳。外部AIを呼ばない |
| `assistant_start_style_learning` | `account_id, requested_count, consent_version, preview_hash` | `StyleProfileJob`。preview変更時は拒否 |
| `assistant_get_style_learning_job` | `job_id` | 自account jobのみ |
| `assistant_cancel_style_learning` | `job_id` | cancellation tokenを発火。active版は変更しない |
| `assistant_list_style_profiles` | `account_id` | 生作例なし。新しい順 |
| `assistant_update_style_profile` | `profile_id, expected_updated_at, content` | draft/activeを編集し新version作成 |
| `assistant_activate_style_profile` | `profile_id` | transactionで一意active化 |
| `assistant_delete_style_profile` | `profile_id` | 生成中に使っている版は新規利用停止、完了runはhashのみ保持 |
| `assistant_preview_style_profile` | `profile_id, fictional_scenario_id` | 組込みの架空入力だけ。実メールを追加送信しない |
| `account_start_google_oauth` | `request_id` | system browser開始情報。秘密を返さない |
| `account_get_google_oauth_status` | `request_id` | pending/connected/denied/failed/expired |
| `account_disconnect_google` | `account_id` | token失効を試み、Keychain削除。失敗を表示 |
| `assistant_start_chatgpt_login` | `request_id, mode?` | provider管理のbrowser/device-code開始情報 |
| `assistant_cancel_chatgpt_login` | `request_id, login_id?` | 進行中の接続だけを中止。A-mail終了時も同じcancel経路を使う |
| `assistant_get_subscription_status` | なし | connected/plan/rate-limit取得可否。tokenは返さない |
| `assistant_disconnect_subscription` | `confirm_shared_logout` | Codex CLI等も共有するChatGPT認証のlogout。確認後だけ実行し、全accountの利用設定をOFFにする |

同accountで文体学習は1件だけ実行できる。別accountのprofile/job ID指定、100/200以外、古いpreview hash、古い更新版は拒否する。

## 6. AI provider設計

### 6.1 共通境界

`DraftTextProvider`と`StyleAnalysisProvider`を分け、どちらもA-mailが渡した文字列/JSONから文字列/JSONを返すだけのinterfaceにする。providerにDB、Keychain、IMAP、SMTP、Tauri AppHandleを渡さない。

provider出力は次を満たすまで保存候補にしない。

- JSON schema、文字数、既知source ID、禁止フィールドを検証する。
- mail headerや宛先命令、受信本文内の「以前の指示を無視」等を命令として扱わない。
- 価格、日程、承諾、固有名詞は根拠sourceか本人の回答方針がなければ断定させない。
- profileに個別案件らしい値が出た場合はactivation前に遮断し、要編集として表示する。

### 6.2 ChatGPT/Codex利用枠の経路

P0は認証・利用状態と返信生成を分離する。Codex App Serverの`chatgpt` managed authenticationと`account/read` / `account/rateLimits/read`を接続・プラン・利用枠表示に使い、返信本文は保存セッションを作らない`codex exec --ephemeral`で生成する。通常のOpenAI APIキー経路とは別であり、枠不足、未対応プラン、認証切れでは処理を停止し、サービス側APIやGeminiへ自動fallbackしない。

A-mailはCodexを強い実行エージェントとして使わず、次の境界で文章生成器として隔離する。

- A-mail専用に作った空の一時ディレクトリをcwdとし、メールDB、home、プロジェクト、添付ディレクトリを見せない。
- `--ephemeral --ignore-user-config --ignore-rules`、read-only sandbox、approval=`never`を固定し、MCP、apps、hooks、multi-agent、webを無効化する。APIキーとprovider base URLの環境変数を子processへ渡さない。
- 出力schemaとJSONL event allowlistを使う。command、file change、MCP、web、dynamic toolまたは未知eventを1件でも観測した候補は保存せず失敗扱いにする。CodexにSMTP/IMAP/DB/送信handleは渡さない。
- 下書き生成ごとに新しいephemeral processを作り、customer/accountを跨いで会話を再利用しない。A-mailが許可済みsourceだけを固定入力として渡す。
- 子processはA-mail起動後の必要時だけ開始し、終了時にgraceful shutdown後、期限内に止まらなければterminateする。
- Codex資格情報ファイルをA-mailがコピー・解析しない。App Serverのaccount/login APIとOS側の安全な資格情報保存に委ねる。

通常のApp Server `thread/start`は永続threadなので返信生成には使わない。公式の非対話モードが明記する`--ephemeral`を採用するが、**未確認ゲート**として、packaged appで履歴・ログへprompt/本文が残らないこと、禁止tool eventが破棄されること、終了後に子processが残らないことを実機確認するまでCargo feature `codex-subscription-reply`は既定OFFとする。CodexランタイムをA-mailへ同梱・再配布できる条件もrelease前に確認する。同梱できなければ「初回のみCodexを導入」が必要となり、「A-mailだけを入れる」要件は未達として表示する。

既存Gemini/OpenAI APIキー経路は開発・上級者用として残してよいが、初期設定や自動fallbackにしない。通常のOpenAI API課金をChatGPTサブスクリプション消費と説明してはならない。

## 7. Gmail OAuth設計

現行のIMAP/SMTPメールコアを維持し、認証だけをGoogle OAuth 2.0 + SASL XOAUTH2へ差し替える。GmailのIMAP/SMTP用scopeは`https://mail.google.com/`である。

- Desktop app用OAuth clientを使う。client secretを端末内の秘密として扱わず、PKCE S256でcode interceptionを防ぐ。
- `http://127.0.0.1:{random_port}`のloopback callbackを使用し、`localhost`固定やOOB copy/pasteを使わない。
- 毎回暗号学的randomなstate、code_verifier、request idを作り、1回・短時間だけ有効にする。
- 同意にはoffline accessを要求し、refresh tokenをKeychainへ保存する。access tokenは期限前に起動中だけrefreshする。
- IMAPは`AUTHENTICATE XOAUTH2`、SMTPは`AUTH XOAUTH2`を使う。access tokenをpassword欄、DB、ログ、フロントDTOへ流さない。
- 認証失効時は「再接続が必要」とし、アプリパスワードへ無断fallbackしない。
- 接続解除では実行中同期を止め、Google token revokeをbest effortで行い、Keychain tokenを削除する。ローカルメール削除は別の明示選択とする。

`https://mail.google.com/`は広いscopeであり、他者へ配布する公開アプリではGoogleのOAuth同意画面、プライバシーポリシー、ドメイン所有、必要な検証・審査が外部前提になる。これを完了するまでは開発用test userでのみ受入試験し、一般利用者が簡単に接続できると主張しない。より限定したscopeへ移る場合はGmail APIへのメールコア変更を別判断とする。

## 8. 起動中限定の実行管理

- Tauri processを唯一のscheduler ownerとする。OS schedulerや別helperの自動起動を使わない。
- ウィンドウを閉じてもプロセスを残す設定の場合は、UIに「A-mail実行中」を表示する。「終了」は全queueをcancelし、IMAP IDLE、OAuth listener、AI子processを止めてから終了する。
- 終了時の生成・学習は`interrupted`、SMTP開始後は既存仕様どおり`unknown`にする。次回起動で自動再開・自動再送しない。
- sleep復帰、network復帰、token refreshで自動下書きのdedupe keyを変えない。1通から複数候補を無断生成しない。
- 自動下書きはアプリ起動中の新規受信だけ。オプトイン以前と停止中に届いたメールは、再起動後に自動処理せず「手動で下書き可能」とする。

## 9. UI文言と状態

設定画面は接続を一つの「AI ON/OFF」にまとめず、次のカードに分ける。

1. **メール接続**: Google接続状態、対象アドレス、再接続、解除。
2. **AI接続**: ChatGPT接続状態、プラン/利用枠取得可否、接続、解除。「A-mailがメールを自動送信する権限はありません」を表示。
3. **自動下書き**: ON/OFF、1日上限、対象外条件、「A-mail起動中のみ」。
4. **自分の文体**: active版、最終学習日時、100/200件の学習ボタン、編集、プレビュー、旧版、削除。

学習確認画面の必須文言:

- 「選択した送信済みメールから、あなたが書いた部分だけを一時的に解析します」
- 「保存するのは言い回し・文章量・段落・改行などの文体プロファイルです。学習用のメール複製は保存しません」
- 「顧客名、メールアドレス、金額、日付などはプロファイル候補から除外します」
- 「AIへ送る件数・文字数」と「ローカル集計だけに使う件数」を別表示する

「学習済み」ではなく、本人がactivationした時に「文体プロファイル使用中」と表示する。精度を保証する表現は避け、修正可能であることを主に伝える。

## 10. 受入テスト

全自動テストは架空メール、一時DB、偽IMAP/SMTP/AI、隔離Keychain service名で行う。実メール・実tokenは専用test accountでの明示操作に分ける。

| ID | 合格条件 |
|---|---|
| SP01 | ボタンを押さない限り、同期・送信・起動でstyle jobが作られない |
| SP02 | 100/200以外を拒否し、選択accountのSent本人本文だけが採用される |
| SP03 | 引用、転送、相手文、別account、添付、重複がprofile入力に混ざらない |
| SP04 | 全採用本文が構造統計に反映され、AI入力はchunk/総文字予算を超えない |
| SP05 | profileにメール、URL、電話、金額、日付、顧客固有語が残ればactivationを拒否する |
| SP06 | job完了/失敗/cancel/終了後に生本文・chunk応答の追加永続化がない |
| SP07 | draft profileは生成に使われず、本人が編集してactiveにした版だけ使われる |
| SP08 | active版の編集・rollback・削除が次回生成へ反映され、既存runの監査hashは維持される |
| GO01 | OAuthのstate不一致、PKCE不一致、期限切れ、callback再送を拒否する |
| GO02 | refresh/access tokenがDB・ログ・フロントDTOに出ず、Keychain/メモリ境界を守る |
| GO03 | IMAP/SMTP mockはXOAUTH2で認証し、期限切れ時に起動中だけ1回refreshして再接続する |
| GO04 | 解除で通信停止・Keychain削除し、ローカルメールは別確認なしに消さない |
| AI01 | ChatGPT未接続・枠不足・認証切れで生成せず、別providerへfallbackしない |
| AI02 | providerにSMTP/IMAP/DB/送信handleがなく、file/shell/MCP/web/tool eventを検出した候補は破棄される |
| AI03 | 生成ごとに新規ephemeral processとなり、以前の会話内容なしで同じ入力境界を再現できる |
| AI04 | prompt/本文がCodex履歴・ログに残らないことを隔離環境で確認する。未達なら実メール機能をreleaseしない |
| SD01 | AI完了からsend IPCへ到達する経路がなく、生成候補は本人の反映操作まで本文を変えない |
| SD02 | 送信は本人クリック/ショートカットからだけ始まり、宛先・workspace変更で再確認になる |
| LC01 | アプリ終了後にA-mail/AI/OAuth listener/同期helper processが残らない |
| LC02 | 終了中の新着を再起動時に自動生成せず、中断jobを自動再開・再課金しない |

実機受入は次を別々に記録する。

- Google開発用test accountでOAuth接続、1通受信、1通手動送信、token refresh、revoke。
- ChatGPT test accountでlogin、利用枠表示、架空メール下書き、隔離・履歴・ログ確認、終了後process確認。
- 100件と200件の**架空送信メール集合**でprofile生成、編集、activation、再生成差分。
- 本人の実メールを使う試験は上記が通った後、対象件数/期間/送信範囲を本人が確認した1回だけ行う。

## 11. 実装順序とrelease gate

### 2026-09-10 時点の実装状況

Phase Aのうち、既存ローカルキャッシュに本文がある送信済みメールを対象にした100/200件プレビュー、引用・短文・重複等の除外、決定論的な文体抽象化、版管理、本人編集、架空メールプレビュー、active版だけを返信プロンプトへ反映する経路を実装済みとする。送信成功時の旧式な自動学習は無効化し、既存設定値もmigration時にOFFへ戻す。

Phase Bのコードとして、App Server account login/status/rate-limit、login cancel/logout、account別のサブスク返信設定、`codex exec --ephemeral`返信adapter、固定schema/JSONL検証、利用状態UI、別providerへのfallback禁止、終了/cancel経路を実装した。通常buildではCargo featureがOFFのため実認証・実生成を開始できない。架空ブラウザmockとunit test、ローカルApp Serverのinitialize probeだけを確認済みで、実ChatGPT login、実モデル呼出し、実メール、実DB、packaged appの`AI04/LC01`は未実施である。

文体学習のPhase Aは引き続き外部AIへ本文を送らず、画面にも `0件 / 0文字` と表示する。ローカルキャッシュに本文がない送信済みメールをIMAPから必要数だけ取得する処理、限定AI抽象化、Gmail OAuth/XOAUTH2は未実装であり、Phase Cのrelease gateを満たした扱いにはしない。

架空ブラウザデモでは、対象確認、明示同意、v1生成、本人による微修正、v2保存、active版切替までを確認済みとする。これはUI導線の確認であり、外部接続や実データでの精度保証ではない。

### Phase A: ローカル完結

- schema/DTO/IPC、style profile editor、100/200 preview、架空データによる抽出・redaction・構造統計。
- providerはfake。生本文非永続とaccount境界を先にテストする。

### Phase B: AI接続

- **実装済み・release待ち:** App Server account認証、利用状態UI、ephemeral返信adapter、tool/cwd/config/event隔離、account別opt-in。
- unit/browser mockは架空データで通す。packaged appの`AI04/LC01`未達なので`codex-subscription-reply`は既定OFFのままにする。

### Phase C: Gmail OAuth

- Google Cloud側のDesktop client/test userを準備し、PKCE/loopback/Keychain/XOAUTH2を実装する。
- 公開配布前にOAuth検証、プライバシー表示、サポート連絡先を完了する。

### Phase D: 統合

- 新着trigger→候補→本人反映→既存送信ゲートを架空transportで通す。
- 専用test accountの実機受入後にのみ既定UIへ昇格する。

P0 releaseの必須gateは、顧客/account混在なし、生本文の追加永続化なし、AIの実行権限なし、本人操作以外の送信なし、終了後processなし、OAuth/AI認証の実機成功である。モデル文章品質は本人評価を別に残し、合格前に「本人そのもの」「完全自動」と宣伝しない。

## 12. 外部前提と未確認事項

| 項目 | 現在の扱い |
|---|---|
| Google OAuth client ID | アプリ提供者による作成が必要。repositoryへsecretを置かない |
| Googleの公開アプリ検証 | `https://mail.google.com/`利用時の一般配布前提。完了までtest user限定 |
| Codex対応ChatGPTプラン | account statusから確認し、未対応なら接続不可と表示 |
| Codex runtimeの同梱・再配布 | **未確認**。release前にライセンス/更新/署名を確認 |
| ephemeral processのprompt非永続化 | source実装済み、packaged実機は**未確認**。`AI04/LC01`で証明できなければ実メール投入不可 |
| 正確なtoken usage | providerが返す場合だけ実測。その他は推定と明記 |
| Claude等の別サブスクAI | provider adapterの将来候補。公式の安全な組込み認証が確認できるまでP0対象外 |

## 13. 公式資料

- [OpenAI: Codex authentication](https://learn.chatgpt.com/docs/auth) — ChatGPT sign-inとAPI keyは別の認証・課金経路。
- [OpenAI: Codex App Server](https://learn.chatgpt.com/docs/app-server) — 製品組込み、account/login、利用状態、thread/approvalイベント。
- [OpenAI: Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode) — `codex exec --ephemeral`、sandbox、JSONL、output schema。
- [OpenAI: ChatGPTとAPIの課金](https://help.openai.com/en/articles/9039756) — ChatGPT契約とAPI課金は別管理。
- [Google: OAuth 2.0 for Desktop Apps](https://developers.google.com/identity/protocols/oauth2/native-app) — system browser、PKCE、loopback callback。
- [Google: Gmail IMAP/SMTP OAuth](https://developers.google.com/workspace/gmail/imap/imap-smtp) — IMAP/SMTPのOAuth 2.0およびSASL XOAUTH2。
- [Google: XOAUTH2 protocol](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol) — `https://mail.google.com/` scopeとIMAP/SMTP交換形式。
