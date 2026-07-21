import { invoke } from "./client";
import type {
  DraftHeader,
  DraftInput,
  Folder,
  MessageFull,
  MessageHeader,
  OutgoingMessage,
  SentInfo,
} from "./types";

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

// ---- M2: 送信 / 下書き ----

/** メールを SMTP 送信し、Sent への保存・\Answered など送信後処理まで行う。 */
export function sendMessage(message: OutgoingMessage): Promise<SentInfo> {
  return invoke<SentInfo>("send_message", { message });
}

/** 下書きを保存(id があれば更新)。draft_id を返す。 */
export function saveDraft(draft: DraftInput): Promise<number> {
  return invoke<number>("save_draft", { draft });
}

/** アカウントの下書き一覧(更新の新しい順)。 */
export function listDrafts(accountId: number): Promise<DraftHeader[]> {
  return invoke<DraftHeader[]>("list_drafts", { accountId });
}

/** 下書きを削除。 */
export function deleteDraft(draftId: number): Promise<void> {
  return invoke<void>("delete_draft", { draftId });
}
