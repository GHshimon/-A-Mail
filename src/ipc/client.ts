import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { AppError, AppErrorKind } from "./types";

/**
 * Tauri コマンド(IPC)呼び出しの薄いラッパ。
 *
 * - 全アクセスはこの層経由に統一し、コンポーネントからの `invoke` 直呼びは禁止。
 * - Rust の `AppResult<T>` の reject を {kind, message} 形へ正規化する。
 * - WebView が Tauri 外(通常の Vite dev / ブラウザ)で動くケースを検出し、
 *   分かりやすいエラーに変換する(M0 のフロント単体確認用)。
 */

const KNOWN_KINDS: readonly AppErrorKind[] = [
  "Auth",
  "Network",
  "Imap",
  "Smtp",
  "Db",
  "Keychain",
  "Ai",
  "BadInput",
  "NotFound",
];

/** Tauri ランタイム上で動いているか(= Rust バックエンドが利用可能か)。 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function normalizeError(err: unknown): AppError {
  // Rust から来た {kind, message} 形はそのまま採用。
  if (
    err &&
    typeof err === "object" &&
    "kind" in err &&
    KNOWN_KINDS.includes((err as { kind: AppErrorKind }).kind)
  ) {
    const e = err as { kind: AppErrorKind; message?: unknown };
    return {
      kind: e.kind,
      message: typeof e.message === "string" ? e.message : "",
    };
  }
  if (typeof err === "string") {
    return { kind: "Network", message: err };
  }
  if (err instanceof Error) {
    return { kind: "Network", message: err.message };
  }
  return { kind: "Network", message: "不明なエラーが発生しました" };
}

export class IpcError extends Error {
  readonly kind: AppErrorKind;
  constructor(appError: AppError) {
    super(appError.message || appError.kind);
    this.name = "IpcError";
    this.kind = appError.kind;
  }
}

export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) {
    // フロント単体(ブラウザ / Vite dev)では Rust バックエンドが無い。
    throw new IpcError({
      kind: "Network",
      message: `Tauri ランタイム外のため '${command}' を呼び出せません`,
    });
  }
  try {
    return await tauriInvoke<T>(command, args);
  } catch (err) {
    throw new IpcError(normalizeError(err));
  }
}
