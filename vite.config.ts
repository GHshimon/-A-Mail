import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// @tauri-apps/cli が `TAURI_ENV_*` を注入する。dev 中はモバイル実機からも
// 参照できるよう host を公開する。
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Tauri は `VITE_` 接頭辞の環境変数のみをフロントへ露出する。
  // 秘密情報(パスワード/APIキー)は Rust 側 + Keychain に置き、ここには載せない。
  envPrefix: ["VITE_", "TAURI_ENV_"],

  // Tauri CLI のエラー出力を潰さないよう Vite のクリア表示を無効化。
  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Rust 側の変更で Vite を再起動させない。
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    // macOS の WebView(WKWebView)は新しめの ES をサポートするので target を上げる。
    target: "es2021",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
