import DOMPurify from "dompurify";

/**
 * HTML メール表示前のサニタイズ(DOMPurify ラッパ)。
 *
 * iframe sandbox + CSP と二重防御する前提。ここでは script/iframe/object 等を
 * 除去し、イベント属性を落とす。実際の描画は HtmlSandbox が iframe srcdoc で行う。
 * 外部画像のブロック/表示切替(CSP 差し替え)は HtmlSandbox 側の責務。
 */
export function sanitizeMailHtml(dirty: string): string {
  return DOMPurify.sanitize(dirty, {
    FORBID_TAGS: ["script", "iframe", "object", "embed", "form", "style", "link"],
    FORBID_ATTR: ["srcset"],
    ALLOW_DATA_ATTR: false,
    // on* イベント属性は DOMPurify が既定で除去する。
    ADD_ATTR: ["target"],
  });
}
