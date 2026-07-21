import type { CSSProperties } from "react";
import { useUiStore } from "@/store/uiStore";
import { TitleBar } from "./TitleBar";
import { ResizablePane } from "./ResizablePane";
import { AccountList } from "@/components/sidebar/AccountList";
import { MessageList } from "@/components/list/MessageList";
import { MessageView } from "@/components/reader/MessageView";
import { ComposeWindow } from "@/components/compose/ComposeWindow";
import { SettingsView } from "@/components/settings/SettingsView";
import { AiSidebar } from "@/components/ai/AiSidebar";

/**
 * アプリ全体のガワ。
 *  titlebar / [ サイドバー | 一覧 | 本文・作成(+ AI ドロワー) ]
 *
 * 列幅は uiStore の paneWidths を CSS 変数として流し込み、ResizablePane の
 * ドラッグで更新する。AI ドロワーは aiOpen かつ compose モード時に列として出現。
 */
export function AppShell() {
  const { nav, list } = useUiStore((s) => s.paneWidths);
  const setPaneWidth = useUiStore((s) => s.setPaneWidth);
  const mode = useUiStore((s) => s.mode);
  const aiOpen = useUiStore((s) => s.aiOpen);

  // AI ドロワーは作成モードで意味を持つ(関連情報は作成中に提示)。
  const showAi = aiOpen && mode === "compose";

  const shellStyle = {
    "--col-nav": `${nav}px`,
    "--col-list": `${list}px`,
    "--col-ai": "320px",
  } as CSSProperties;

  return (
    <div
      className={`app-shell${showAi ? " ai-open" : ""}`}
      style={shellStyle}
    >
      <TitleBar />
      <div className="shell-body">
        <ResizablePane
          width={nav}
          onResize={(w) => setPaneWidth("nav", w)}
        >
          <AccountList />
        </ResizablePane>

        <ResizablePane
          width={list}
          onResize={(w) => setPaneWidth("list", w)}
        >
          <MessageList />
        </ResizablePane>

        {mode === "compose" ? (
          <ComposeWindow />
        ) : mode === "settings" ? (
          <SettingsView />
        ) : (
          <MessageView />
        )}

        {showAi && <AiSidebar />}
      </div>
    </div>
  );
}
