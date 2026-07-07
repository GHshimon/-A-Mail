import { invoke } from "./client";
import type { Folder, MessageFull, MessageHeader } from "./types";

// フォルダ/メッセージ/同期/送信/下書き系 Tauri コマンドの型付きラッパ。
// 実装本体は Rust 側(commands/mail_cmd.rs)で M1〜M2 に用意する。

export function listFolders(accountId: number): Promise<Folder[]> {
  return invoke<Folder[]>("list_folders", { accountId });
}

/** IMAP へ接続・ログインできるかを検証(フォルダ操作はしない)。 */
export function testConnection(accountId: number): Promise<void> {
  return invoke<void>("test_connection", { accountId });
}

/** IMAP LIST でフォルダを取得し DB に反映、最新の一覧を返す。 */
export function syncFolders(accountId: number): Promise<Folder[]> {
  return invoke<Folder[]>("sync_folders", { accountId });
}

export function listMessages(
  folderId: number,
  offset: number,
  limit: number,
): Promise<MessageHeader[]> {
  return invoke<MessageHeader[]>("list_messages", { folderId, offset, limit });
}

/** フォルダを IMAP から同期し、直近のヘッダ一覧を返す。 */
export function syncFolder(
  folderId: number,
  limit?: number,
): Promise<MessageHeader[]> {
  return invoke<MessageHeader[]>("sync_folder", { folderId, limit });
}

export function getMessage(messageId: number): Promise<MessageFull> {
  return invoke<MessageFull>("get_message", { messageId });
}

export function markRead(messageId: number, seen: boolean): Promise<void> {
  return invoke<void>("mark_read", { messageId, seen });
}
