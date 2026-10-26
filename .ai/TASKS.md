# 作業リスト

`.ai/tasks.jsonl` から生成。直接編集しても次の render で消える。
更新は `scripts/aitask.py`(共有コピー: `.ai/aitask.py`)から行う。

最終更新: 2026-09-12T10:54

## 未着手

| ID | 担当 | 作業 | 範囲 |
|---|---|---|---|
| t0014 | - | 実機での目視確認: 参考ファイル UI / 分類精度 | src/components/ai/ContextFiles.tsx<br>src-tauri/src/ai/prompts.rs |
| t0015 | - | NAS フォルダ連携(参考ファイルの次段階) | src-tauri/src/ai |
| t0016 | - | SPEC 4.2 の残り: 通知 / IMAP IDLE / スレッド表示 / 送信添付 | src-tauri/src<br>src |
| t0020 | - | 【要注意】main ビルドを実 DB へ向けない(統合まで) | src-tauri/tauri.conf.json |
| t0021 | - | Claudeブランチの必須保持機能をmainへ段階統合 | src-tauri/src/imap<br>src-tauri/src/util<br>src-tauri/src/state.rs<br>src-tauri/src/store<br>src<br>src-tauri/Cargo.toml |
| t0030 | - | 【不具合/codex領域】要点が壊れる・下書きが空(応答の切り詰め + bodyLen 再取得) | src-tauri/src/ai/gemini.rs<br>src-tauri/src/commands/ai_gen_cmd.rs<br>src/components/ai/AiSidebar.tsx |

## 完了

| ID | 担当 | 作業 | 範囲 |
|---|---|---|---|
| t0002 | claude | M2 送信・下書き・連絡先(SMTP) | src-tauri/src/smtp<br>src-tauri/src/util/reply.rs<br>src-tauri/src/commands/compose_cmd.rs |
| t0003 | claude | M3 Gemini 補助 A/B/C | src-tauri/src/ai<br>src-tauri/src/commands/ai_cmd.rs<br>src/store/aiStore.ts |
| t0004 | claude | フロント全面: モック除去・UI/UX 改良・Vitest 導入 | src<br>src/styles |
| t0005 | claude | 添付ファイルの保存 | src-tauri/src/commands/mail_cmd.rs<br>src-tauri/src/imap/fetch.rs |
| t0006 | claude | 文字化け修正(full_encoding / UTF-7 / 生 ISO-2022-JP) | src-tauri/src/util/utf7.rs<br>src-tauri/src/util/jis.rs<br>src-tauri/src/imap/fetch.rs<br>src-tauri/src/imap/sync.rs |
| t0007 | claude | IMAP 接続再利用・差分同期・本文先読み | src-tauri/src/imap/pool.rs<br>src-tauri/src/commands/mail_cmd.rs<br>src-tauri/src/state.rs |
| t0008 | claude | 新着メールの AI 分類(タグ付け) | src-tauri/src/ai/prompts.rs<br>src-tauri/src/store/migrations.rs<br>src/lib/category.ts<br>src/components/list |
| t0009 | claude | 作成時の参考ファイル(AI コンテキスト) | src-tauri/src/ai/mod.rs<br>src/lib/contextFile.ts<br>src/components/ai/ContextFiles.tsx |
| t0010 | codex | 下書きアシスタント P0: バックエンド | src-tauri/src<br>src-tauri/Cargo.toml |
| t0011 | codex | 下書きアシスタント P0: フロント | src<br>package.json |
| t0012 | codex | 顧客コンテキスト境界の設計・仕様 | CONTEXT_REPLY_IMPLEMENTATION_SPEC.md<br>DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md |
| t0013 | codex | 【要判断】claude ブランチと main の統合(migrations の版番号が衝突) | src-tauri/src/store/migrations.rs |
| t0017 | claude | 作業リストの仕組み(aitask.py / AGENTS.md) | scripts/aitask.py<br>AGENTS.md<br>.ai |
| t0018 | claude | 統合監査(読み取り専用): claude ブランチ x main 未コミット | .ai/CLAUDE_INTEGRATION_AUDIT.md |
| t0019 | codex | 【要判断】移行番号の解決: 構造検査付き互換移行の採否 | src-tauri/src/store/migrations.rs |
| t0022 | codex | オンデマンド文体プロファイル・OAuth・サブスクAI仕様更新 | .ai/decisions.jsonl<br>DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md<br>SPEC.md<br>MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md |
| t0023 | codex | オンデマンド文体プロファイル Phase A | src-tauri/src/style_profile.rs<br>src-tauri/src/store/migrations.rs<br>src-tauri/src/store/models.rs<br>src-tauri/src/commands/assistant_cmd.rs<br>src-tauri/src/assistant_automation.rs<br>src-tauri/src/lib.rs<br>src/ipc/types.ts<br>src/ipc/assistant.ts<br>src/components/ai/AssistantSettingsPanel.tsx<br>src/styles/global.css |
| t0024 | codex | 有効文体プロファイルの返信候補反映 | src-tauri/src/assistant.rs<br>src-tauri/src/assistant_quality.rs |
| t0025 | codex | Phase B ChatGPTサブスクAI接続 | src-tauri/src/codex_subscription.rs<br>src-tauri/src/commands/subscription_cmd.rs<br>src-tauri/src/commands/assistant_cmd.rs<br>src-tauri/src/commands/mod.rs<br>src-tauri/src/state.rs<br>src-tauri/src/lib.rs<br>src-tauri/src/store/migrations.rs<br>src-tauri/src/store/models.rs<br>src-tauri/src/assistant.rs<br>src/ipc/types.ts<br>src/ipc/assistant.ts<br>src/components/ai/AssistantSettingsPanel.tsx<br>src/styles/global.css<br>MAIL_AGENT_RUNTIME_P0_IMPLEMENTATION_SPEC.md<br>DRAFT_ASSISTANT_P0_IMPLEMENTATION_SPEC.md |
| t0026 | codex | AIサイドバーHooks順序クラッシュ修正 | src/components/ai/AssistantDraftList.tsx |
| t0027 | codex | スマホ向けPhase Bデモ動画・静止画の作成 | Sample/Demo |
| t0028 | codex | 実画面を使った機能説明資料の作成 | Sample/FeatureGuide |
| t0029 | codex | A-mail最新共有checkoutのmacOS再ビルド | dist<br>src-tauri/target/release/bundle |

## 取りやめ

| ID | 担当 | 作業 | 範囲 |
|---|---|---|---|
| t0001 | - | 動作確認用 | src-tauri/src/imap |

