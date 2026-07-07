import { useMemo, useState } from "react";
import { sanitizeMailHtml } from "@/lib/sanitizeHtml";

interface HtmlSandboxProps {
  html: string;
}

/**
 * HTML メールの安全表示。
 *
 * 二重防御:
 *  1. DOMPurify でサニタイズ(script/iframe/イベント属性除去)。
 *  2. iframe sandbox(allow-scripts は付けない)+ CSP で外部リソース遮断。
 *
 * 外部画像は既定ブロック。ユーザーが「画像を表示」を押すと img-src を緩めた
 * srcdoc に差し替える。
 */
export function HtmlSandbox({ html }: HtmlSandboxProps) {
  const [showImages, setShowImages] = useState(false);

  const srcdoc = useMemo(() => {
    const clean = sanitizeMailHtml(html);
    const imgSrc = showImages ? "https: data:" : "data:";
    const csp = [
      "default-src 'none'",
      `img-src ${imgSrc}`,
      "style-src 'unsafe-inline'",
      "font-src data:",
    ].join("; ");
    return `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<style>body{font-family:-apple-system,system-ui,sans-serif;color:#1c1e22;margin:12px;line-height:1.6;}
img{max-width:100%;height:auto;} a{color:#0a6cff;}</style>
</head><body>${clean}</body></html>`;
  }, [html, showImages]);

  return (
    <div style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0 }}>
      {!showImages && (
        <div
          style={{
            padding: "6px 22px",
            fontSize: 11.5,
            color: "var(--ink-3)",
            borderBottom: "1px solid var(--sep)",
            display: "flex",
            alignItems: "center",
            gap: 10,
          }}
        >
          <span>🔒 外部画像はブロックされています</span>
          <button className="iconbtn" onClick={() => setShowImages(true)}>
            画像を表示
          </button>
        </div>
      )}
      <iframe
        className="html-sandbox"
        title="メール本文"
        sandbox="allow-popups-to-escape-sandbox"
        srcDoc={srcdoc}
      />
    </div>
  );
}
