# ProjectPilot workspace

このフォルダはProjectPilotが、工程、レシピ、実行、レビュー、承認履歴を管理するために使用します。

- 認証情報やAPIキーは保存しません。
- AI出力はまず `drafts/` へ保存されます。
- プロジェクトカバーの正本は `assets/project-cover/cover.json` です。
- `state/events.jsonl` は追記履歴です。
- 既存成果物への反映とステージ移行にはオーナー確認が必要です。
