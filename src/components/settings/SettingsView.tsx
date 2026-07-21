import { useUiStore } from "@/store/uiStore";
import { AccountSettings } from "./AccountSettings";
import { AiSettings } from "./AiSettings";

/**
 * 設定ビュー(閲覧/作成エリアに全面表示)。アカウント設定と AI 設定を縦に並べる。
 * ⌘, または タイトルバーの「⚙︎ 設定」から開き、「閉じる」で閲覧へ戻る。
 */
export function SettingsView() {
  const setMode = useUiStore((s) => s.setMode);

  return (
    <section className="reader" aria-label="設定">
      <div className="cmp-toolbar">
        <span className="chip">⚙︎ 設定</span>
        <div className="grow" />
        <button className="iconbtn" onClick={() => setMode("read")}>
          閉じる
        </button>
      </div>
      <div className="settings-scroll">
        <AccountSettings />
        <AiSettings />
      </div>
    </section>
  );
}
