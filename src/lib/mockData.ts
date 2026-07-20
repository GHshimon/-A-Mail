import type { Account, Folder, MessageHeader } from "@/ipc/types";

/**
 * M0 用のプレースホルダデータ。
 *
 * Rust バックエンドがまだ無い / Tauri 外(ブラウザや Linux でのフロント確認)
 * のとき、UI のガワを確認できるようダミーを供給する。実データが入る M1 以降は
 * ストアが IPC 経由の取得へ切り替わり、これらは使われなくなる。
 */

export const mockAccounts: Account[] = [
  {
    id: 1,
    email: "douchi.shimon@gmail.com",
    provider: "gmail",
    display_name: "ドウチ",
    imap_host: "imap.gmail.com",
    imap_port: 993,
    smtp_host: "smtp.gmail.com",
    smtp_port: 465,
    smtp_starttls: false,
  },
  {
    id: 2,
    email: "shimon@icloud.com",
    provider: "icloud",
    display_name: "ドウチ (iCloud)",
    imap_host: "imap.mail.me.com",
    imap_port: 993,
    smtp_host: "smtp.mail.me.com",
    smtp_port: 587,
    smtp_starttls: true,
  },
];

export const mockFolders: Record<number, Folder[]> = {
  1: [
    { id: 11, account_id: 1, name: "受信トレイ", imap_path: "INBOX", role: "inbox", unread: 12 },
    { id: 12, account_id: 1, name: "スター付き", imap_path: "[Gmail]/Starred", role: "starred", unread: 0 },
    { id: 13, account_id: 1, name: "送信済み", imap_path: "[Gmail]/Sent Mail", role: "sent", unread: 0 },
    { id: 14, account_id: 1, name: "下書き", imap_path: "[Gmail]/Drafts", role: "drafts", unread: 2 },
    { id: 15, account_id: 1, name: "アーカイブ", imap_path: "[Gmail]/All Mail", role: "archive", unread: 0 },
    { id: 16, account_id: 1, name: "ゴミ箱", imap_path: "[Gmail]/Trash", role: "trash", unread: 0 },
  ],
  2: [
    { id: 21, account_id: 2, name: "受信トレイ", imap_path: "INBOX", role: "inbox", unread: 3 },
    { id: 22, account_id: 2, name: "送信済み", imap_path: "Sent Messages", role: "sent", unread: 0 },
  ],
};

const now = Math.floor(Date.now() / 1000);
const min = 60;
const hour = 3600;
const day = 86400;

export const mockMessages: Record<number, MessageHeader[]> = {
  11: [
    {
      id: 101,
      uid: 5001,
      subject: "Re: 4月納品分の請求書について",
      from: "田中 亮 · 山手商事",
      date: now - 90 * min,
      snippet: "お世話になっております。先日ご相談した納期の件、社内で確認が取れました…",
      seen: false,
      flagged: false,
      has_attachments: true,
    },
    {
      id: 102,
      uid: 5000,
      subject: "[ghshimon/-a-mail] CI passed",
      from: "GitHub",
      date: now - 3 * hour,
      snippet: "All checks have passed for commit 4462899…",
      seen: false,
      flagged: false,
      has_attachments: false,
    },
    {
      id: 103,
      uid: 4998,
      subject: "来週の打ち合わせ日程の件",
      from: "佐藤 みなみ",
      date: now - 5 * hour,
      snippet: "火曜か水曜の午後で調整できればと思っております。ご都合いかがでしょうか。",
      seen: true,
      flagged: false,
      has_attachments: false,
    },
    {
      id: 104,
      uid: 4990,
      subject: "ご購入の領収書",
      from: "Apple",
      date: now - 1 * day,
      snippet: "iCloud+ 200GB のお支払いが完了しました。",
      seen: true,
      flagged: false,
      has_attachments: false,
    },
    {
      id: 105,
      uid: 4980,
      subject: "3月分お支払いのご連絡",
      from: "鈴木 建設 経理部",
      date: now - 5 * day,
      snippet: "下記の通りお振込みいたしましたのでご確認ください。",
      seen: true,
      flagged: true,
      has_attachments: false,
    },
  ],
};
