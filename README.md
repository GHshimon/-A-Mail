# -A-Mail

Mac 上で動作する、AI 入力補助(Gemini)つきの独立系メールクライアント。個人利用目的。

- 仕様: [SPEC.md](./SPEC.md)
- 実装設計: [IMPLEMENTATION.md](./IMPLEMENTATION.md)
- 技術: **Tauri v2 (Rust) + Vite + React + TypeScript**、状態管理は **Zustand**

## 現在の状態: M0(土台)

M0 で用意したもの:

- Tauri v2 + Vite + React + TS の雛形(IMPLEMENTATION.md §1 のディレクトリ構成に準拠)
- 3 ペイン + AI 補助ドロワーの UI ガワ(サイドバー / 一覧 / 本文・作成 / AI)
- ダーク / ライトのシステム追従(`useTheme`、`data-theme` 切替)
- ペイン幅のドラッグリサイズ、一覧の仮想スクロール
- macOS 向けウィンドウ設定(`titleBarStyle: Overlay` / `transparent` / CSP / `macOSPrivateApi`)、`window-vibrancy` によるサイドバー半透明
- Rust バックエンド土台: `AppError` / `AppState` / SQLite 初期化 + マイグレーション(FTS5 含む)/ Keychain ラッパ / 設定・Gemini キー登録コマンド
- 型付き IPC ラッパ(`src/ipc/*`)とエラー正規化

> M0 の UI はモックデータで表示を確認できる(Tauri 外・ブラウザでも動作)。実データ連携(IMAP/SMTP/Gemini)は M1 以降。

## 開発

### 必要ツール

- Node.js 20+ / npm
- Rust(stable, `rustup`)
- macOS + **Xcode Command Line Tools**(`.app` ビルド時)

### フロントエンド(どの OS でも可)

```bash
npm install
npm run build     # tsc 型チェック + vite build
npm run dev       # Vite 単体(Tauri 外)でガワを確認
```

### アプリ(macOS)

```bash
npm run tauri dev     # フロント HMR + Rust
npm run tauri build   # → src-tauri/target/release/bundle/macos/-A-Mail.app
```

個人利用のため署名 / 公証は不要。初回は `右クリック→開く`、または
`xattr -dr com.apple.quarantine <アプリ>` で Gatekeeper を回避。

> 注: Tauri のネイティブビルドには `webkit2gtk` 等の system ライブラリが必要で、
> `.app` ビルドは macOS 前提。Linux ではフロントの `npm run build` までを確認する。

## マイルストーン

M0 土台 → M1 閲覧 → M2 作成/送信 → M3 AI 補助 → M4 拡張(詳細は IMPLEMENTATION.md §12)。
