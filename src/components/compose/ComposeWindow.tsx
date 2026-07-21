import { useCallback, useEffect, useRef, useState } from "react";
import {
  hasContent,
  toDraftInput,
  toOutgoing,
  useComposeStore,
  type Length,
  type Tone,
} from "@/store/composeStore";
import { useUiStore } from "@/store/uiStore";
import { sendMessage, saveDraft } from "@/ipc/mail";
import { IpcError } from "@/ipc/client";
import { GhostTextEditor } from "./GhostTextEditor";

const TONES: { v: Tone; label: string }[] = [
  { v: "casual", label: "カジュアル" },
  { v: "standard", label: "標準" },
  { v: "formal", label: "丁寧" },
];
const LENGTHS: { v: Length; label: string }[] = [
  { v: "short", label: "短" },
  { v: "mid", label: "標準" },
  { v: "long", label: "長" },
];

/** カンマ/改行/セミコロン区切りの文字列をアドレス配列へ。 */
function parseAddrs(raw: string): string[] {
  return raw
    .split(/[,\n;]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

const AUTOSAVE_MS = 2000;

/** メール作成エリア(compose モード)。宛先/件名/本文の編集・送信・下書きオートセーブ。 */
export function ComposeWindow() {
  const draft = useComposeStore((s) => s.draft);
  const tone = useComposeStore((s) => s.tone);
  const length = useComposeStore((s) => s.length);
  const status = useComposeStore((s) => s.status);
  const setTone = useComposeStore((s) => s.setTone);
  const setLength = useComposeStore((s) => s.setLength);
  const setSubject = useComposeStore((s) => s.setSubject);
  const setAccountId = useComposeStore((s) => s.setAccountId);
  const setDraftId = useComposeStore((s) => s.setDraftId);
  const setStatus = useComposeStore((s) => s.setStatus);
  const resetDraft = useComposeStore((s) => s.resetDraft);

  const aiOpen = useUiStore((s) => s.aiOpen);
  const toggleAi = useUiStore((s) => s.toggleAi);
  const selectedAccountId = useUiStore((s) => s.selectedAccountId);

  const kind = useComposeStore((s) => s.kind);

  // 宛先/Cc は入力の途中状態を保つためローカルの生文字列で保持。
  const [toRaw, setToRaw] = useState("");
  const [ccRaw, setCcRaw] = useState("");
  const [showCc, setShowCc] = useState(false);

  // 返信/転送などで store 側の宛先が差し替わったら生文字列へ反映(タイプ中は store を触らないのでループしない)。
  useEffect(() => setToRaw(draft.to.join(", ")), [draft.to]);
  useEffect(() => {
    setCcRaw(draft.cc.join(", "));
    if (draft.cc.length > 0) setShowCc(true);
  }, [draft.cc]);

  // account 未設定なら現在選択中のアカウントを引き継ぐ。
  useEffect(() => {
    if (draft.accountId == null && selectedAccountId != null) {
      setAccountId(selectedAccountId);
    }
  }, [draft.accountId, selectedAccountId, setAccountId]);

  const effAccountId = draft.accountId ?? selectedAccountId;

  // 現在の入力から送信/保存ペイロードを組む(宛先は生文字列をパース)。
  const currentDraft = useCallback(
    () => ({
      ...draft,
      accountId: effAccountId,
      to: parseAddrs(toRaw),
      cc: parseAddrs(ccRaw),
    }),
    [draft, effAccountId, toRaw, ccRaw],
  );

  // ---- 下書きオートセーブ(2秒デバウンス) ----
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    const d = currentDraft();
    if (effAccountId == null || !hasContent(d)) return;
    if (status.phase === "sending") return;

    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      const input = toDraftInput(d);
      if (!input) return;
      try {
        setStatus({ phase: "saving" });
        const id = await saveDraft(input);
        setDraftId(id);
        setStatus({ phase: "saved", at: Date.now() });
      } catch {
        // 下書き保存の失敗は致命でない(ブラウザ単体実行/一時的な失敗)。静かに戻す。
        setStatus({ phase: "idle" });
      }
    }, AUTOSAVE_MS);

    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
    // 本文/件名/宛先の変化で再スケジュール。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toRaw, ccRaw, draft.subject, draft.body, effAccountId]);

  // ---- 送信 ----
  const send = useCallback(async () => {
    const d = currentDraft();
    if (effAccountId == null) {
      setStatus({ phase: "error", message: "アカウントを選択してください" });
      return;
    }
    if (d.to.length === 0) {
      setStatus({ phase: "error", message: "宛先を入力してください" });
      return;
    }
    const payload = toOutgoing(d);
    if (!payload) return;

    setStatus({ phase: "sending" });
    try {
      await sendMessage(payload);
      setStatus({ phase: "sent" });
      // 送信済み下書きは Rust 側で削除済み。フォームは新規状態へ。
      resetDraft({ accountId: effAccountId });
    } catch (err) {
      const message =
        err instanceof IpcError ? err.message || err.kind : "送信に失敗しました";
      setStatus({ phase: "error", message });
    }
  }, [currentDraft, effAccountId, resetDraft, setStatus]);

  const canSend =
    effAccountId != null &&
    parseAddrs(toRaw).length > 0 &&
    status.phase !== "sending";

  const kindLabel =
    kind === "reply" || kind === "reply-all"
      ? "↩︎ 返信を作成中"
      : kind === "forward"
        ? "➡︎ 転送を作成中"
        : "✎ 新規メール";

  return (
    <section
      className="reader"
      aria-label="メール作成"
      onKeyDown={(e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
          e.preventDefault();
          void send();
        }
      }}
    >
      <div className="cmp-toolbar">
        <span className="chip">{kindLabel}</span>
        <span className="seg-label">文体</span>
        <div className="tone-seg" role="group" aria-label="文体トーン">
          {TONES.map((t) => (
            <button key={t.v} aria-pressed={tone === t.v} onClick={() => setTone(t.v)}>
              {t.label}
            </button>
          ))}
        </div>
        <span className="seg-label">長さ</span>
        <div className="tone-seg" role="group" aria-label="長さ">
          {LENGTHS.map((l) => (
            <button key={l.v} aria-pressed={length === l.v} onClick={() => setLength(l.v)}>
              {l.label}
            </button>
          ))}
        </div>
        <div className="grow" />
        <StatusText />
        <button
          className={`iconbtn${aiOpen ? " on" : ""}`}
          title="AIアシストを開閉"
          onClick={() => toggleAi()}
        >
          ✦ AI
        </button>
        <button className="primary" title="送信" disabled={!canSend} onClick={() => void send()}>
          {status.phase === "sending" ? "送信中…" : "送信"}{" "}
          <span className="kbd on-accent">⌘↩</span>
        </button>
      </div>

      <div className="fields">
        <div className="field">
          <span className="lbl">宛先</span>
          <input
            className="field-input"
            value={toRaw}
            placeholder="メールアドレス(カンマ区切り)"
            aria-label="宛先"
            onChange={(e) => setToRaw(e.target.value)}
            spellCheck={false}
          />
          {!showCc && (
            <button
              className="iconbtn"
              title="Cc を追加"
              onClick={() => setShowCc(true)}
            >
              Cc
            </button>
          )}
        </div>
        {showCc && (
          <div className="field">
            <span className="lbl">Cc</span>
            <input
              className="field-input"
              value={ccRaw}
              placeholder="Cc(カンマ区切り)"
              aria-label="Cc"
              onChange={(e) => setCcRaw(e.target.value)}
              spellCheck={false}
            />
          </div>
        )}
        <div className="field">
          <span className="lbl">件名</span>
          <input
            className="field-input"
            value={draft.subject}
            placeholder="件名"
            aria-label="件名"
            onChange={(e) => setSubject(e.target.value)}
            spellCheck={false}
          />
        </div>
      </div>

      <GhostTextEditor />

      <div className="ghost-hint">
        グレーの文字が予測入力です。<span className="kbd">Tab</span> で確定 /{" "}
        <span className="kbd">Esc</span> で破棄。
      </div>
    </section>
  );
}

/** ツールバー右の送信/保存ステータス。 */
function StatusText() {
  const status = useComposeStore((s) => s.status);
  switch (status.phase) {
    case "saving":
      return <span className="cmp-status">保存中…</span>;
    case "saved":
      return <span className="cmp-status ok">下書き保存済み</span>;
    case "sent":
      return <span className="cmp-status ok">✓ 送信しました</span>;
    case "error":
      return <span className="cmp-status err">{status.message}</span>;
    default:
      return null;
  }
}
