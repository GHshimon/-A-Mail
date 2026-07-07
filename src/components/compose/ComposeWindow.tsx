import { useComposeStore, type Length, type Tone } from "@/store/composeStore";
import { useUiStore } from "@/store/uiStore";
import { AddressField } from "./AddressField";
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

/** メール作成エリア(compose モード)。ツールバー + 宛先/件名 + 予測入力エディタ。 */
export function ComposeWindow() {
  const draft = useComposeStore((s) => s.draft);
  const tone = useComposeStore((s) => s.tone);
  const length = useComposeStore((s) => s.length);
  const setTone = useComposeStore((s) => s.setTone);
  const setLength = useComposeStore((s) => s.setLength);

  const aiOpen = useUiStore((s) => s.aiOpen);
  const toggleAi = useUiStore((s) => s.toggleAi);

  return (
    <section className="reader" aria-label="メール作成">
      <div className="cmp-toolbar">
        <span className="chip">↩︎ 返信を作成中</span>
        <span className="seg-label">文体</span>
        <div className="tone-seg" role="group" aria-label="文体トーン">
          {TONES.map((t) => (
            <button
              key={t.v}
              aria-pressed={tone === t.v}
              onClick={() => setTone(t.v)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <span className="seg-label">長さ</span>
        <div className="tone-seg" role="group" aria-label="長さ">
          {LENGTHS.map((l) => (
            <button
              key={l.v}
              aria-pressed={length === l.v}
              onClick={() => setLength(l.v)}
            >
              {l.label}
            </button>
          ))}
        </div>
        <div className="grow" />
        <button
          className={`iconbtn${aiOpen ? " on" : ""}`}
          title="AIアシストを開閉"
          onClick={() => toggleAi()}
        >
          ✦ AI
        </button>
        <button className="primary" title="送信">
          送信 <span className="kbd on-accent">⌘↩</span>
        </button>
      </div>

      <div className="fields">
        <AddressField label="宛先" addresses={draft.to} />
        <div className="field">
          <span className="lbl">件名</span>
          <span className="val">{draft.subject}</span>
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
