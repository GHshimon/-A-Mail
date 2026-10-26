import fs from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const sharp = require("sharp");

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const framesDir = path.join(scriptDir, "frames");

const W = 1080;
const H = 1920;
const palette = {
  night: "#111827",
  night2: "#172554",
  ink: "#172033",
  ink2: "#4c566a",
  muted: "#6b7280",
  panel: "#ffffff",
  canvas: "#f4f6fa",
  line: "#dfe4ec",
  blue: "#0a6cff",
  blueSoft: "#e7f0ff",
  purple: "#7b5cff",
  purpleSoft: "#f0ecff",
  green: "#218a4b",
  greenSoft: "#e7f7ed",
  amber: "#b86500",
  amberSoft: "#fff4dc",
  red: "#d13c45",
  redSoft: "#fff0f1",
};

const font = "-apple-system, BlinkMacSystemFont, 'Hiragino Kaku Gothic ProN', 'Hiragino Sans', 'Yu Gothic', sans-serif";

function esc(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function rect(x, y, width, height, fill, radius = 28, extra = "") {
  return `<rect x="${x}" y="${y}" width="${width}" height="${height}" rx="${radius}" fill="${fill}" ${extra}/>`;
}

function line(x1, y1, x2, y2, stroke = palette.line, width = 2, dash = "") {
  return `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${stroke}" stroke-width="${width}" ${dash ? `stroke-dasharray="${dash}"` : ""}/>`;
}

function textLines(x, y, lines, options = {}) {
  const {
    size = 34,
    lineHeight = Math.round(size * 1.45),
    weight = 500,
    fill = palette.ink,
    anchor = "start",
    letterSpacing = 0,
    opacity = 1,
  } = options;
  const body = lines
    .map(
      (item, index) =>
        `<tspan x="${x}" dy="${index === 0 ? 0 : lineHeight}">${esc(item)}</tspan>`,
    )
    .join("");
  return `<text x="${x}" y="${y}" text-anchor="${anchor}" font-family="${font}" font-size="${size}" font-weight="${weight}" letter-spacing="${letterSpacing}" fill="${fill}" opacity="${opacity}">${body}</text>`;
}

function pill(x, y, label, options = {}) {
  const {
    width = Math.max(136, label.length * 25 + 44),
    fill = palette.blueSoft,
    color = palette.blue,
    height = 48,
    size = 23,
  } = options;
  return `${rect(x, y, width, height, fill, height / 2)}${textLines(x + width / 2, y + height / 2 + 8, [label], {
    size,
    weight: 700,
    fill: color,
    anchor: "middle",
  })}`;
}

function button(x, y, label, options = {}) {
  const {
    width = 300,
    fill = palette.blue,
    color = "#ffffff",
    stroke = "none",
    height = 70,
  } = options;
  return `${rect(x, y, width, height, fill, 18, `stroke="${stroke}" stroke-width="2"`)}${textLines(x + width / 2, y + height / 2 + 10, [label], {
    size: 28,
    weight: 700,
    fill: color,
    anchor: "middle",
  })}`;
}

function toggle(x, y, on) {
  const fill = on ? palette.blue : "#cbd2dd";
  const knobX = on ? x + 48 : x + 8;
  return `${rect(x, y, 88, 48, fill, 24)}<circle cx="${knobX + 16}" cy="${y + 24}" r="16" fill="#ffffff"/>`;
}

function checkRow(x, y, label, options = {}) {
  const { color = palette.green, detail = "" } = options;
  return `
    <circle cx="${x + 18}" cy="${y - 9}" r="18" fill="${color}"/>
    <path d="M ${x + 9} ${y - 9} l 7 7 l 13 -15" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
    ${textLines(x + 52, y, [label], { size: 28, weight: 700 })}
    ${detail ? textLines(x + 52, y + 38, [detail], { size: 23, fill: palette.muted }) : ""}
  `;
}

function appChrome(x, y, width, height, title = "A-mail") {
  return `
    <g filter="url(#shadow)">
      ${rect(x, y, width, height, palette.panel, 30)}
      ${rect(x, y, width, 74, "#edf1f7", 30)}
      <rect x="${x}" y="${y + 44}" width="${width}" height="30" fill="#edf1f7"/>
      <circle cx="${x + 32}" cy="${y + 36}" r="9" fill="#ff5f57"/>
      <circle cx="${x + 61}" cy="${y + 36}" r="9" fill="#febc2e"/>
      <circle cx="${x + 90}" cy="${y + 36}" r="9" fill="#28c840"/>
      ${textLines(x + 122, y + 45, [title], { size: 23, weight: 700, fill: palette.ink2 })}
    </g>
  `;
}

function progress(current, dark, total = 8) {
  let result = "";
  const start = W - 72 - total * 24;
  for (let index = 0; index < total; index += 1) {
    const fill = dark
      ? (index < current ? "#ffffff" : "rgba(255,255,255,.28)")
      : (index < current ? palette.blue : "rgba(23,32,51,.16)");
    result += `<circle cx="${start + index * 27}" cy="94" r="7" fill="${fill}"/>`;
  }
  return result;
}

function base({ index, eyebrow, title, subtitle, body, theme = "light" }) {
  const dark = theme === "dark";
  const titleFill = dark ? "#ffffff" : palette.ink;
  const subtitleFill = dark ? "#cbd5e1" : palette.ink2;
  const background = dark
    ? `<rect width="${W}" height="${H}" fill="url(#nightGrad)"/>`
    : `<rect width="${W}" height="${H}" fill="url(#lightGrad)"/>`;
  return `
    <svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
      <defs>
        <linearGradient id="nightGrad" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#0f172a"/><stop offset=".52" stop-color="#172554"/><stop offset="1" stop-color="#312e81"/></linearGradient>
        <linearGradient id="lightGrad" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#eef5ff"/><stop offset=".55" stop-color="#f6f4ff"/><stop offset="1" stop-color="#edf7f4"/></linearGradient>
        <linearGradient id="aiGrad" x1="0" y1="0" x2="1" y2="0"><stop stop-color="${palette.blue}"/><stop offset="1" stop-color="${palette.purple}"/></linearGradient>
        <filter id="shadow" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="0" dy="20" stdDeviation="26" flood-color="#1e293b" flood-opacity=".18"/></filter>
      </defs>
      ${background}
      ${dark ? `<circle cx="910" cy="310" r="270" fill="#7c3aed" opacity=".12"/><circle cx="140" cy="1600" r="340" fill="#0ea5e9" opacity=".10"/>` : ""}
      ${pill(72, 70, eyebrow, {
        fill: dark ? "rgba(255,255,255,.13)" : "rgba(10,108,255,.11)",
        color: dark ? "#ffffff" : palette.blue,
      })}
      ${progress(index, dark)}
      ${textLines(72, 210, title, { size: 68, lineHeight: 82, weight: 800, fill: titleFill, letterSpacing: -2 })}
      ${textLines(76, 380, subtitle, { size: 31, lineHeight: 46, weight: 500, fill: subtitleFill })}
      ${body}
      ${line(72, 1812, 1008, 1812, dark ? "rgba(255,255,255,.18)" : "#d7deea", 2)}
      ${textLines(72, 1860, ["架空デモ  |  実メール・実認証・実送信なし"], { size: 22, weight: 700, fill: dark ? "#cbd5e1" : palette.muted })}
      ${textLines(1008, 1860, [`${index} / 8`], { size: 22, weight: 700, fill: dark ? "#cbd5e1" : palette.muted, anchor: "end" })}
    </svg>
  `;
}

const slides = [
  {
    name: "01-title",
    svg: base({
      index: 1,
      eyebrow: "A-mail / Phase B",
      title: ["返信候補が", "できるまで"],
      subtitle: ["AIは提案する。", "確認して送るのは本人。"],
      theme: "dark",
      body: `
        ${rect(72, 650, 936, 700, "rgba(255,255,255,.08)", 42, 'stroke="rgba(255,255,255,.15)" stroke-width="2"')}
        ${textLines(126, 735, ["受信"], { size: 34, weight: 800, fill: "#fff" })}
        ${textLines(126, 788, ["テストメール"], { size: 26, fill: "#cbd5e1" })}
        ${line(126, 844, 944, 844, "rgba(255,255,255,.18)", 2)}
        ${textLines(126, 930, ["文脈境界"], { size: 34, weight: 800, fill: "#fff" })}
        ${textLines(126, 983, ["customer / workspace / thread / message ID"], { size: 24, fill: "#cbd5e1" })}
        ${line(126, 1039, 944, 1039, "rgba(255,255,255,.18)", 2)}
        ${textLines(126, 1125, ["AI候補"], { size: 34, weight: 800, fill: "#fff" })}
        ${textLines(126, 1178, ["本人の文体プロファイルを参照"], { size: 26, fill: "#cbd5e1" })}
        ${line(126, 1234, 944, 1234, "rgba(255,255,255,.18)", 2)}
        ${textLines(126, 1320, ["本人確認  →  通常の送信操作"], { size: 34, weight: 800, fill: "#fff" })}
        ${rect(72, 1440, 936, 188, "#ffffff", 34)}
        ${textLines(120, 1516, ["この動画で分かること"], { size: 28, weight: 800, fill: palette.blue })}
        ${textLines(120, 1570, ["接続・生成・確認・送信の安全な境界"], { size: 30, weight: 700, fill: palette.ink })}
      `,
    }),
  },
  {
    name: "02-context-boundary",
    svg: base({
      index: 2,
      eyebrow: "STEP 1",
      title: ["テストメールを開く"],
      subtitle: ["メールアドレスだけに頼らず、内部IDで", "顧客と案件の境界を固定します。"],
      body: `
        ${appChrome(72, 560, 936, 930, "A-mail  |  受信トレイ")}
        ${pill(112, 675, "テストメール", { width: 190, fill: palette.amberSoft, color: palette.amber })}
        ${textLines(112, 780, ["導入日程の確認"], { size: 42, weight: 800 })}
        ${textLines(112, 842, ["サンプル株式会社 田中（架空）"], { size: 25, weight: 700, fill: palette.ink2 })}
        ${textLines(112, 907, ["A-mail サポートご担当者様", "来月の導入候補日と、事前準備について", "教えていただけますでしょうか。"], { size: 27, lineHeight: 48, fill: palette.ink2 })}
        ${rect(112, 1085, 856, 326, palette.purpleSoft, 24)}
        ${textLines(150, 1143, ["返信コンテキスト境界"], { size: 27, weight: 800, fill: palette.purple })}
        ${textLines(150, 1202, ["customer_id"], { size: 22, weight: 700, fill: palette.muted })}
        ${textLines(390, 1202, ["CUST-DEMO-024"], { size: 24, weight: 700 })}
        ${textLines(150, 1254, ["workspace_id"], { size: 22, weight: 700, fill: palette.muted })}
        ${textLines(390, 1254, ["SAMPLE-SERVICE"], { size: 24, weight: 700 })}
        ${textLines(150, 1306, ["thread_id"], { size: 22, weight: 700, fill: palette.muted })}
        ${textLines(390, 1306, ["THR-DEMO-001"], { size: 24, weight: 700 })}
        ${textLines(150, 1358, ["境界状態"], { size: 22, weight: 700, fill: palette.muted })}
        ${pill(390, 1322, "confirmed", { width: 170, height: 44, size: 22, fill: palette.greenSoft, color: palette.green })}
        ${rect(72, 1548, 936, 150, palette.panel, 28)}
        ${textLines(112, 1612, ["同じ製品名でも、別顧客の情報は混ぜません。"], { size: 29, weight: 800 })}
        ${textLines(112, 1660, ["件名一致・アドレス一致だけで自動統合しない設計です。"], { size: 23, fill: palette.muted })}
      `,
    }),
  },
  {
    name: "03-connect",
    svg: base({
      index: 3,
      eyebrow: "STEP 2",
      title: ["ChatGPTに接続"],
      subtitle: ["APIキーをA-mailへ登録せず、", "利用者自身の対応サブスク枠を使います。"],
      body: `
        ${appChrome(72, 560, 936, 892, "A-mail  |  AI設定")}
        ${textLines(120, 684, ["AI接続"], { size: 34, weight: 800 })}
        ${pill(790, 646, "未接続", { width: 160, fill: palette.redSoft, color: palette.red })}
        ${rect(112, 752, 856, 230, palette.canvas, 24)}
        ${textLines(148, 815, ["ChatGPT / Codex 対応サブスクリプション"], { size: 27, weight: 800 })}
        ${textLines(148, 872, ["APIキーはA-mailに保存しません。"], { size: 25, fill: palette.ink2 })}
        ${textLines(148, 918, ["接続しただけではメール本文を送りません。"], { size: 25, fill: palette.ink2 })}
        ${button(112, 1035, "ChatGPTで接続", { width: 430 })}
        ${rect(112, 1145, 856, 214, palette.blueSoft, 24)}
        ${textLines(150, 1207, ["本人が行うこと"], { size: 27, weight: 800, fill: palette.blue })}
        ${textLines(150, 1262, ["1. 接続ボタンを押す", "2. ブラウザで認証する", "3. 返信下書きへの利用をONにする"], { size: 25, lineHeight: 42, fill: palette.ink })}
        ${rect(72, 1510, 936, 170, palette.amberSoft, 28)}
        ${textLines(112, 1575, ["現在の配布ビルドでは release gate はOFF"], { size: 28, weight: 800, fill: palette.amber })}
        ${textLines(112, 1625, ["この場面は接続動線を説明するUI再現です。"], { size: 24, fill: palette.ink2 })}
      `,
    }),
  },
  {
    name: "04-browser-auth",
    svg: base({
      index: 4,
      eyebrow: "STEP 3",
      title: ["ブラウザで本人認証"],
      subtitle: ["パスワードや2段階認証は本人が入力。", "A-mailが認証情報を聞き取ることはありません。"],
      body: `
        ${rect(72, 585, 936, 700, palette.panel, 38, 'filter="url(#shadow)"')}
        ${pill(120, 642, "認証フロー", { width: 180, fill: palette.purpleSoft, color: palette.purple })}
        ${rect(120, 742, 252, 240, palette.canvas, 26)}
        ${textLines(246, 820, ["A-mail"], { size: 34, weight: 800, anchor: "middle" })}
        ${textLines(246, 875, ["接続を開始"], { size: 25, fill: palette.muted, anchor: "middle" })}
        ${line(390, 862, 504, 862, palette.purple, 7)}
        <path d="M 500 846 L 528 862 L 500 878 Z" fill="${palette.purple}"/>
        ${rect(548, 742, 372, 240, palette.night, 26)}
        ${textLines(734, 810, ["ChatGPT"], { size: 34, weight: 800, fill: "#ffffff", anchor: "middle" })}
        ${textLines(734, 865, ["本人がログイン"], { size: 25, fill: "#cbd5e1", anchor: "middle" })}
        ${textLines(734, 912, ["パスワード / 2FA"], { size: 22, fill: "#94a3b8", anchor: "middle" })}
        ${line(734, 1017, 734, 1070, palette.green, 7)}
        <path d="M 718 1064 L 734 1092 L 750 1064 Z" fill="${palette.green}"/>
        ${pill(594, 1110, "接続済み", { width: 280, height: 72, size: 30, fill: palette.greenSoft, color: palette.green })}
        ${rect(72, 1360, 936, 320, palette.panel, 30)}
        ${checkRow(118, 1438, "A-mailにAPIキーを入力しない", { detail: "秘密鍵の発行・貼り付け・保管が不要" })}
        ${checkRow(118, 1542, "認証はブラウザ上で本人が完了", { detail: "認証画面の操作はこの架空デモでは省略" })}
        ${checkRow(118, 1646, "接続解除の影響を事前表示", { detail: "同じMacのCodex CLIにも影響する可能性を案内" })}
      `,
    }),
  },
  {
    name: "05-opt-in",
    svg: base({
      index: 5,
      eyebrow: "STEP 4",
      title: ["利用条件を本人がON"],
      subtitle: ["接続済みでも自動では使いません。", "アカウントごとに明示的に有効化します。"],
      body: `
        ${appChrome(72, 555, 936, 970, "A-mail  |  AI接続")}
        ${textLines(120, 677, ["AI接続"], { size: 34, weight: 800 })}
        ${pill(786, 641, "接続済み", { width: 170, fill: palette.greenSoft, color: palette.green })}
        ${rect(112, 744, 856, 280, palette.canvas, 24)}
        ${textLines(148, 805, ["プラン"], { size: 22, weight: 700, fill: palette.muted })}
        ${textLines(416, 805, ["対応プラン（表示例）"], { size: 25, weight: 700 })}
        ${textLines(148, 862, ["短期枠"], { size: 22, weight: 700, fill: palette.muted })}
        ${textLines(416, 862, ["18% 使用"], { size: 25, weight: 700 })}
        ${rect(586, 840, 322, 24, "#d9e1ed", 12)}${rect(586, 840, 58, 24, palette.blue, 12)}
        ${textLines(148, 919, ["長期枠"], { size: 22, weight: 700, fill: palette.muted })}
        ${textLines(416, 919, ["42% 使用"], { size: 25, weight: 700 })}
        ${rect(586, 897, 322, 24, "#d9e1ed", 12)}${rect(586, 897, 135, 24, palette.purple, 12)}
        ${textLines(148, 976, ["※ 数値はすべて架空の表示例"], { size: 20, fill: palette.muted })}
        ${rect(112, 1067, 856, 134, palette.blueSoft, 24)}
        ${toggle(148, 1110, true)}
        ${textLines(266, 1135, ["返信下書きにこのChatGPT接続を使う"], { size: 26, weight: 800 })}
        ${textLines(266, 1171, ["失敗時のAPI自動切替なし"], { size: 21, fill: palette.muted })}
        ${rect(112, 1240, 856, 204, palette.greenSoft, 24)}
        ${textLines(148, 1305, ["実行範囲"], { size: 27, weight: 800, fill: palette.green })}
        ${textLines(148, 1360, ["A-mail起動中だけ  /  1日の生成上限あり", "終了後の常駐処理・自動送信なし"], { size: 25, lineHeight: 42, fill: palette.ink })}
        ${rect(72, 1580, 936, 118, palette.panel, 28)}
        ${textLines(112, 1650, ["接続  ≠  本文共有  ≠  送信"], { size: 34, weight: 800, fill: palette.purple })}
      `,
    }),
  },
  {
    name: "06-generate-candidate",
    svg: base({
      index: 6,
      eyebrow: "STEP 5",
      title: ["返信候補を生成"],
      subtitle: ["必要な文脈だけを隔離して渡し、", "AIの出力は本文とは別の「候補」にします。"],
      body: `
        ${appChrome(72, 555, 936, 1015, "A-mail  |  返信作成")}
        ${rect(102, 659, 390, 820, "#fbfcfd", 22, `stroke="${palette.line}" stroke-width="2"`)}
        ${pill(134, 707, "テストメール", { width: 176, height: 42, size: 20, fill: palette.amberSoft, color: palette.amber })}
        ${textLines(134, 804, ["導入日程の確認"], { size: 31, weight: 800 })}
        ${textLines(134, 856, ["田中様（架空）"], { size: 23, weight: 700, fill: palette.ink2 })}
        ${textLines(134, 925, ["来月の候補日と", "事前準備を確認したい"], { size: 25, lineHeight: 43, fill: palette.ink2 })}
        ${line(134, 1031, 454, 1031, palette.line, 2)}
        ${textLines(134, 1082, ["参照範囲"], { size: 22, weight: 800, fill: palette.muted })}
        ${textLines(134, 1132, ["・同一スレッド", "・サンプル企画書", "・文体プロファイル v2"], { size: 23, lineHeight: 42, fill: palette.ink })}
        ${pill(134, 1305, "境界 confirmed", { width: 242, height: 44, size: 21, fill: palette.greenSoft, color: palette.green })}
        ${rect(520, 659, 458, 820, palette.purpleSoft, 22)}
        ${pill(552, 707, "AI候補", { width: 146, height: 42, size: 20, fill: "#ffffff", color: palette.purple })}
        ${textLines(552, 810, ["田中様"], { size: 25, weight: 700 })}
        ${textLines(552, 865, ["お問い合わせありがとう", "ございます。導入候補日は", "10月7日・9日・14日です。"], { size: 24, lineHeight: 42, fill: palette.ink })}
        ${textLines(552, 1015, ["事前に対象人数とご利用", "環境をご共有ください。"], { size: 24, lineHeight: 42, fill: palette.ink })}
        ${textLines(552, 1130, ["どうぞよろしくお願いいたします。"], { size: 24, fill: palette.ink })}
        ${line(552, 1190, 940, 1190, "#d7cffc", 2)}
        ${textLines(552, 1242, ["文体: 丁寧 / 3段落 / 空行あり"], { size: 21, weight: 700, fill: palette.purple })}
        ${textLines(552, 1290, ["出典: 同一スレッド + 企画書"], { size: 21, fill: palette.ink2 })}
        ${pill(552, 1342, "本文は未変更", { width: 236, height: 46, size: 21, fill: palette.redSoft, color: palette.red })}
        ${rect(72, 1620, 936, 100, palette.panel, 26)}
        ${textLines(112, 1682, ["候補ができても、作成画面の本文は勝手に変わりません。"], { size: 27, weight: 800 })}
      `,
    }),
  },
  {
    name: "07-human-review",
    svg: base({
      index: 7,
      eyebrow: "STEP 6",
      title: ["本人が確認・反映"],
      subtitle: ["宛先・根拠・言い回しを確認してから、", "候補を本文へ反映します。"],
      body: `
        ${rect(72, 565, 936, 900, palette.panel, 34, 'filter="url(#shadow)"')}
        ${textLines(120, 655, ["送信前チェック"], { size: 34, weight: 800 })}
        ${checkRow(120, 744, "宛先", { detail: "田中様 / サンプル株式会社（架空）" })}
        ${checkRow(120, 862, "根拠", { detail: "同一スレッド・サンプル企画書のみ" })}
        ${checkRow(120, 980, "言い回し", { detail: "本人の文体プロファイル v2 を反映" })}
        ${rect(112, 1080, 856, 116, palette.amberSoft, 22)}
        ${textLines(150, 1152, ["日程の確定情報は本人が最終確認"], { size: 28, weight: 800, fill: palette.amber })}
        ${button(112, 1245, "本文に反映", { width: 350 })}
        ${button(486, 1245, "修正", { width: 210, fill: "#ffffff", color: palette.ink, stroke: palette.line })}
        ${button(720, 1245, "破棄", { width: 210, fill: "#ffffff", color: palette.red, stroke: "#efb5ba" })}
        ${rect(72, 1530, 936, 190, palette.redSoft, 30)}
        ${textLines(116, 1600, ["AIにメール送信権限はありません"], { size: 34, weight: 800, fill: palette.red })}
        ${textLines(116, 1654, ["本文反映後も、通常の「送信」は本人が押します。"], { size: 26, weight: 600, fill: palette.ink })}
      `,
    }),
  },
  {
    name: "08-status",
    svg: base({
      index: 8,
      eyebrow: "CURRENT STATUS",
      title: ["ここまで実装済み", "ここから実機確認"],
      subtitle: ["デモで見せられる設計と、", "まだ証明していないことを分けます。"],
      theme: "dark",
      body: `
        ${rect(72, 585, 936, 454, "rgba(255,255,255,.10)", 34, 'stroke="rgba(255,255,255,.18)" stroke-width="2"')}
        ${pill(112, 629, "実装済み", { width: 168, fill: palette.greenSoft, color: palette.green })}
        ${textLines(112, 723, ["・ChatGPT接続UI / 利用枠表示", "・アカウント単位の明示ON", "・隔離した返信候補生成", "・本文反映と送信の本人確認ゲート"], { size: 28, lineHeight: 65, weight: 650, fill: "#ffffff" })}
        ${rect(72, 1085, 936, 430, "rgba(255,255,255,.10)", 34, 'stroke="rgba(255,255,255,.18)" stroke-width="2"')}
        ${pill(112, 1129, "実機未確認", { width: 210, fill: palette.amberSoft, color: palette.amber })}
        ${textLines(112, 1223, ["・配布アプリでの実ChatGPT認証", "・実サブスク枠での返信候補生成", "・実Gmailアカウントでの送信", "・終了後に動作しないことの受入試験"], { size: 28, lineHeight: 65, weight: 650, fill: "#ffffff" })}
        ${rect(72, 1580, 936, 130, "#ffffff", 28)}
        ${textLines(112, 1634, ["release gate は受入試験まで OFF"], { size: 31, weight: 800, fill: palette.purple })}
        ${textLines(112, 1676, ["この動画は操作設計の説明であり、本番接続の証跡ではありません。"], { size: 22, fill: palette.ink2 })}
      `,
    }),
  },
];

function overviewSvg() {
  const step = (number, y, title, detail, color, fill) => `
    ${rect(72, y, 936, 86, fill, 22, `stroke="${color}" stroke-width="1.5" stroke-opacity=".28"`)}
    <circle cx="126" cy="${y + 42}" r="42" fill="${color}"/>
    ${textLines(126, y + 54, [String(number)], { size: 30, weight: 800, fill: "#ffffff", anchor: "middle" })}
    ${textLines(196, y + 25, [title], { size: 30, weight: 800 })}
    ${textLines(196, y + 69, [detail], { size: 23, fill: palette.ink2 })}
  `;
  return `
    <svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
      <defs>
        <linearGradient id="overviewBg" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#edf5ff"/><stop offset=".55" stop-color="#f7f3ff"/><stop offset="1" stop-color="#edf9f3"/></linearGradient>
        <filter id="shadow" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="0" dy="16" stdDeviation="22" flood-color="#1e293b" flood-opacity=".15"/></filter>
      </defs>
      <rect width="${W}" height="${H}" fill="url(#overviewBg)"/>
      ${pill(72, 70, "A-mail / 架空デモ", { width: 246 })}
      ${textLines(72, 186, ["AI返信候補の流れ"], { size: 62, weight: 800, letterSpacing: -1.5 })}
      ${textLines(76, 244, ["AIは提案。確認して送るのは本人。"], { size: 28, fill: palette.ink2 })}
      ${rect(54, 316, 972, 820, palette.panel, 36, 'filter="url(#shadow)"')}
      ${step(1, 360, "テストメールを開く", "顧客・案件・スレッド・メッセージIDで境界固定", palette.blue, palette.blueSoft)}
      ${step(2, 484, "ChatGPTへ接続", "APIキーをA-mailへ登録せず、本人がブラウザ認証", palette.purple, palette.purpleSoft)}
      ${step(3, 608, "アカウントごとにON", "接続だけでは本文共有も候補生成もしない", palette.green, palette.greenSoft)}
      ${step(4, 732, "返信候補を生成", "必要な文脈と編集可能な文体プロファイルだけを参照", palette.purple, palette.purpleSoft)}
      ${step(5, 856, "本人が確認・本文反映", "宛先・根拠・言い回しをチェック", palette.amber, palette.amberSoft)}
      ${step(6, 980, "本人が通常送信", "AIには送信権限なし。アプリ終了後は動作なし", palette.red, palette.redSoft)}
      ${rect(54, 1190, 972, 270, palette.night, 34)}
      ${textLines(98, 1260, ["安全境界"], { size: 31, weight: 800, fill: "#ffffff" })}
      ${textLines(98, 1322, ["接続  ≠  本文共有  ≠  本文反映  ≠  送信"], { size: 29, weight: 800, fill: "#c4b5fd" })}
      ${textLines(98, 1380, ["同一製品でも別顧客の情報は混ぜない"], { size: 25, fill: "#cbd5e1" })}
      ${rect(54, 1505, 472, 220, palette.greenSoft, 28)}
      ${pill(88, 1538, "実装済み", { width: 154, height: 42, size: 20, fill: "#ffffff", color: palette.green })}
      ${textLines(88, 1620, ["接続UI / 明示ON", "候補分離 / 確認ゲート"], { size: 25, lineHeight: 44, weight: 700 })}
      ${rect(554, 1505, 472, 220, palette.amberSoft, 28)}
      ${pill(588, 1538, "実機未確認", { width: 188, height: 42, size: 20, fill: "#ffffff", color: palette.amber })}
      ${textLines(588, 1620, ["実認証 / 実AI生成", "実Gmail送信 / 終了試験"], { size: 25, lineHeight: 44, weight: 700 })}
      ${line(72, 1804, 1008, 1804, "#d4dbe7", 2)}
      ${textLines(72, 1852, ["架空デモ  |  release gate は受入試験まで OFF"], { size: 22, weight: 700, fill: palette.muted })}
    </svg>
  `;
}

await fs.mkdir(framesDir, { recursive: true });
for (const slide of slides) {
  await sharp(Buffer.from(slide.svg)).png().toFile(path.join(framesDir, `${slide.name}.png`));
}
await sharp(Buffer.from(overviewSvg()))
  .png()
  .toFile(path.join(scriptDir, "phase-b-mobile-demo-overview.png"));

const manifest = {
  title: "A-mail Phase B mobile demonstration",
  disclaimer: "架空デモ。実メール・実認証・実AI生成・実送信は行っていません。",
  frame_size: `${W}x${H}`,
  seconds_per_frame: 7.5,
  expected_duration_seconds: slides.length * 7.5,
  frames: slides.map((slide) => `frames/${slide.name}.png`),
};
await fs.writeFile(
  path.join(scriptDir, "render_manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8",
);

console.log(`Rendered ${slides.length} frames and overview to ${scriptDir}`);
