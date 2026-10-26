import { useEffect, useRef } from "react";
import { useUiStore } from "@/store/uiStore";
import { useMailStore } from "@/store/mailStore";
import { useComposeStore } from "@/store/composeStore";
import { useAccountStore } from "@/store/accountStore";

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
  const accountId = useUiStore((s) => s.selectedAccountId);
  const selectMessage = useUiStore((s) => s.selectMessage);
  const accounts = useAccountStore((s) => s.accounts);
  const searchQuery = useMailStore((s) => s.searchQuery);
  const search = useMailStore((s) => s.search);
  const setSearchQuery = useMailStore((s) => s.setSearchQuery);
  const clearSearch = useMailStore((s) => s.clearSearch);
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (searchTimer.current) clearTimeout(searchTimer.current);
  }, []);

  // アカウント切替時に、旧アカウント向けの遅延検索を発火させない。
  useEffect(() => {
    if (searchTimer.current) {
      clearTimeout(searchTimer.current);
      searchTimer.current = null;
    }
  }, [accountId]);

  const cancelAndClearSearch = () => {
    if (searchTimer.current) {
      clearTimeout(searchTimer.current);
      searchTimer.current = null;
    }
    clearSearch();
  };

  const updateSearch = (value: string) => {
    if (searchTimer.current) clearTimeout(searchTimer.current);
    if (!value) {
      cancelAndClearSearch();
      return;
    }
    // 検索結果と、直前に開いていたメール本文が食い違わないようにする。
    selectMessage(null);
    // 入力値を即時表示しつつ、FTS 呼び出しは短くデバウンスする。
    setSearchQuery(value);
    // 起動直後は selectedAccountId の反映が検索入力より遅れることがあるため、
    // 利用可能なら先頭アカウントをフォールバックに使う。
    const searchAccountId = accountId ?? accounts[0]?.id ?? null;
    searchTimer.current = setTimeout(() => void search(searchAccountId, value), 250);
  };

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
        <span aria-hidden="true">⌕</span>
        <input
          value={searchQuery}
          placeholder="メールを検索"
          aria-label="メールを検索"
          onChange={(e) => updateSearch(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              cancelAndClearSearch();
              e.currentTarget.blur();
            }
          }}
        />
        {searchQuery && (
          <button aria-label="検索をクリア" onClick={cancelAndClearSearch}>×</button>
        )}
      </div>
      {mode === "compose" && (
        <button
          className={`iconbtn${aiOpen ? " on" : ""}`}
          title="AI アシストを開閉"
          onClick={() => toggleAi()}
        >
          ✦ AI
        </button>
      )}
      <button
        className={`iconbtn${mode === "settings" ? " on" : ""}`}
        title="設定 (⌘,)"
        onClick={() => setMode(mode === "settings" ? "read" : "settings")}
      >
        ⚙︎
      </button>
      <button
        className="iconbtn"
        title="ローカル下書き"
        onClick={() => {
          setMode("compose");
          toggleAi(true);
        }}
      >
        下書き
      </button>
      <button
        className="primary"
        title="新規作成"
        onClick={() => {
          if (mode === "compose") {
            setMode("read");
          } else {
            const selectedAccountId = useUiStore.getState().selectedAccountId;
            useComposeStore.getState().resetDraft({ accountId: selectedAccountId });
            useComposeStore.getState().setKind("new");
            setMode("compose");
          }
        }}
      >
        ✏️ 作成
      </button>
    </div>
  );
}
