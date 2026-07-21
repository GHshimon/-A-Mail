import { useUiStore } from "@/store/uiStore";

/**
 * タイトルバー(macOS Overlay スタイル前提)。
 *
 * traffic light は OS が描画するため、左に `padding-left` の余白のみ確保する
 * (CSS 側)。`data-tauri-drag-region` を付与しウィンドウドラッグを可能にする。
 * 非 macOS ではフォールバックの疑似 traffic light を表示する。
 */
export function TitleBar() {
  const mode = useUiStore((s) => s.mode);
  const setMode = useUiStore((s) => s.setMode);
  const toggleAi = useUiStore((s) => s.toggleAi);
  const aiOpen = useUiStore((s) => s.aiOpen);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="traffic" aria-hidden="true">
        <span className="light r" />
        <span className="light y" />
        <span className="light g" />
      </div>
      <span className="tb-title">受信トレイ</span>
      <div className="tb-spacer" data-tauri-drag-region />
      <div className="search" role="search">
        🔍 メールを検索
      </div>
      <button
        className={`iconbtn${aiOpen ? " on" : ""}`}
        title="AI アシストを開閉"
        onClick={() => toggleAi()}
      >
        ✦ AI
      </button>
      <button
        className={`iconbtn${mode === "settings" ? " on" : ""}`}
        title="設定 (⌘,)"
        onClick={() => setMode(mode === "settings" ? "read" : "settings")}
      >
        ⚙︎
      </button>
      <button
        className="primary"
        title="新規作成"
        onClick={() => setMode(mode === "compose" ? "read" : "compose")}
      >
        ✏️ 作成
      </button>
    </div>
  );
}
