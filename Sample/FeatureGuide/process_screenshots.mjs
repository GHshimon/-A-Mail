import fs from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const sharp = require("sharp");

const here = path.dirname(fileURLToPath(import.meta.url));
const rawDir = "/tmp/a-mail-featureguide-raw";
const outputDir = path.join(here, "screens");

const W = 868;
const H = 813;
const uiFont = "-apple-system, BlinkMacSystemFont, 'Hiragino Sans', 'Yu Gothic', sans-serif";

const xml = (value) => String(value)
  .replaceAll("&", "&amp;")
  .replaceAll("<", "&lt;")
  .replaceAll(">", "&gt;")
  .replaceAll('"', "&quot;");

function overlaySvg(items) {
  const body = items.map((item) => {
    if (item.kind === "rect") {
      return `<rect x="${item.x}" y="${item.y}" width="${item.width}" height="${item.height}" rx="${item.radius ?? 4}" fill="${item.fill ?? "#1d1f22"}"/>`;
    }
    return `<text x="${item.x}" y="${item.y}" font-family="${uiFont}" font-size="${item.size ?? 10}" font-weight="${item.weight ?? 600}" fill="${item.fill ?? "#8b909b"}" letter-spacing="${item.letterSpacing ?? 0}">${xml(item.text)}</text>`;
  }).join("");
  return Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">${body}</svg>`);
}

const sidebarRedactions = [
  { kind: "rect", x: 2, y: 54, width: 208, height: 27, fill: "#1d1f22" },
  { kind: "text", x: 9, y: 72, text: "DEMO@EXAMPLE.TEST", size: 10, fill: "#777d89", letterSpacing: 0.5 },
  { kind: "rect", x: 2, y: 331, width: 208, height: 29, fill: "#1d1f22" },
  { kind: "text", x: 40, y: 350, text: "SAMPLE@EXAMPLE.TEST", size: 10, fill: "#777d89", letterSpacing: 0.5 },
];

const githubRedaction = [
  { kind: "rect", x: 220, y: 221, width: 300, height: 35, fill: "#1d1f22" },
  { kind: "text", x: 252, y: 243, text: "[demo/a-mail] CI passed", size: 12, fill: "#e6e8ec", weight: 700 },
];

const composeFromRedaction = [
  { kind: "rect", x: 55, y: 163, width: 300, height: 33, fill: "#1d1f22" },
  { kind: "text", x: 69, y: 184, text: "デモ <demo@example.test>", size: 12, fill: "#c8ccd3", weight: 600 },
];

const composeMailboxRedaction = [
  { kind: "rect", x: 64, y: 204, width: 315, height: 32, fill: "#1d1f22" },
  { kind: "text", x: 69, y: 226, text: "デモ <demo@example.test>", size: 12, fill: "#c8ccd3", weight: 600 },
];

const settingsAccountRedactions = [
  { kind: "rect", x: 539, y: 153, width: 190, height: 69, fill: "#1d1f22" },
  { kind: "text", x: 550, y: 178, text: "デモユーザー", size: 12, fill: "#e6e8ec", weight: 700 },
  { kind: "text", x: 550, y: 198, text: "<demo@example.test>", size: 11, fill: "#c8ccd3" },
  { kind: "text", x: 550, y: 216, text: "· gmail", size: 10, fill: "#9aa0aa" },
  { kind: "rect", x: 539, y: 235, width: 160, height: 69, fill: "#1d1f22" },
  { kind: "text", x: 550, y: 259, text: "サンプルユーザー", size: 12, fill: "#e6e8ec", weight: 700 },
  { kind: "text", x: 550, y: 279, text: "sample@example.test", size: 9, fill: "#c8ccd3" },
  { kind: "text", x: 550, y: 297, text: "· icloud", size: 10, fill: "#9aa0aa" },
];

async function redact(sourceName, outputName, items) {
  const source = path.join(rawDir, sourceName);
  await sharp(source)
    .composite([{ input: overlaySvg(items), left: 0, top: 0 }])
    .png()
    .toFile(path.join(outputDir, outputName));
}

async function crop(sourceName, outputName, extract) {
  const source = path.join(rawDir, sourceName);
  await sharp(source)
    .extract(extract)
    .resize({ width: extract.width * 2, kernel: sharp.kernel.lanczos3 })
    .png()
    .toFile(path.join(outputDir, outputName));
}

await fs.mkdir(outputDir, { recursive: true });

await redact("01-inbox.png", "01-inbox.png", [...sidebarRedactions, ...githubRedaction]);
await redact("02-message-reader.png", "02-message-reader.png", [...sidebarRedactions, ...githubRedaction]);
await redact("18-mail-search.png", "03-mail-search.png", sidebarRedactions);
await redact("04-context-confirmed.png", "04-context-confirmed.png", composeFromRedaction);
await crop("04-context-confirmed.png", "15-context-confirmed-panel.png", { left: 568, top: 38, width: 300, height: 745 });
await crop("06-reply-candidate-detail.png", "05-reply-candidate-panel.png", { left: 568, top: 38, width: 300, height: 745 });
await redact("07-applied-draft.png", "06-applied-draft.png", [...composeFromRedaction, ...composeMailboxRedaction]);
await crop("12-chatgpt-connected-optin.png", "07-chatgpt-connected-panel.png", { left: 568, top: 38, width: 300, height: 745 });
await crop("13-style-learning-preview.png", "08-style-learning-preview-panel.png", { left: 568, top: 70, width: 300, height: 700 });
await crop("14-style-profile-editor.png", "09-style-profile-editor-panel.png", { left: 568, top: 38, width: 300, height: 745 });
await crop("15-style-profile-preview.png", "10-style-profile-preview-panel.png", { left: 568, top: 38, width: 300, height: 745 });
await crop("17-codex-cli-ready.png", "11-codex-cli-panel.png", { left: 568, top: 38, width: 300, height: 745 });
await crop("19-new-compose-project-context.png", "12-new-compose-context-panel.png", { left: 568, top: 38, width: 300, height: 745 });
await redact("20-account-settings.png", "13-account-settings.png", [
  ...sidebarRedactions,
  ...githubRedaction,
  ...settingsAccountRedactions,
]);
await crop("08-ai-provider-settings.png", "14-legacy-ai-provider-panel.png", { left: 520, top: 38, width: 348, height: 745 });

const manifest = {
  captured_from: "http://127.0.0.1:1421/",
  capture_type: "A-mail React UI running in the browser demo provider",
  source_data: "fictional browser demo data; no real mail, AI provider, SMTP, or authentication",
  privacy_processing: [
    "Replaced personal-looking account labels and From addresses with example.test values",
    "Replaced a personal-looking GitHub handle with demo/a-mail",
    "Cropped AI panels so unrelated account data is outside the delivered image",
  ],
  files: [
    "01-inbox.png",
    "02-message-reader.png",
    "03-mail-search.png",
    "04-context-confirmed.png",
    "05-reply-candidate-panel.png",
    "06-applied-draft.png",
    "07-chatgpt-connected-panel.png",
    "08-style-learning-preview-panel.png",
    "09-style-profile-editor-panel.png",
    "10-style-profile-preview-panel.png",
    "11-codex-cli-panel.png",
    "12-new-compose-context-panel.png",
    "13-account-settings.png",
    "14-legacy-ai-provider-panel.png",
    "15-context-confirmed-panel.png"
  ],
};

await fs.writeFile(
  path.join(here, "screen_manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8",
);

console.log(`Processed ${manifest.files.length} screenshots into ${outputDir}`);
