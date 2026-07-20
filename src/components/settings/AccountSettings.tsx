import { useState } from "react";
import { useAccountStore } from "@/store/accountStore";
import { isTauri, IpcError } from "@/ipc/client";
import { testConnection } from "@/ipc/mail";
import type { Provider } from "@/ipc/types";

/**
 * アカウント設定: 登録済み一覧 + 追加/削除 + 接続テスト/フォルダ同期。
 *
 * メール + アプリパスワードで登録すると、接続設定はプロバイダのプリセットから
 * 解決され、アプリパスワードは Rust 側で Keychain に保存される(DB には残さない)。
 * 登録時に IMAP へ接続してフォルダ一覧も取得する(失敗時は「同期」で再試行)。
 */

const PRESET_HINT: Record<Provider, string> = {
  gmail: "IMAP imap.gmail.com:993 / SMTP smtp.gmail.com:465",
  icloud: "IMAP imap.mail.me.com:993 / SMTP smtp.mail.me.com:587",
};

function msgOf(err: unknown, fallback: string): string {
  return err instanceof IpcError ? err.message : fallback;
}

export function AccountSettings() {
  const accounts = useAccountStore((s) => s.accounts);
  const addAccount = useAccountStore((s) => s.addAccount);
  const removeAccount = useAccountStore((s) => s.removeAccount);
  const refreshFolders = useAccountStore((s) => s.refreshFolders);

  const [email, setEmail] = useState("");
  const [appPassword, setAppPassword] = useState("");
  const [provider, setProvider] = useState<Provider>("gmail");
  const [displayName, setDisplayName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rowStatus, setRowStatus] = useState<Record<number, string>>({});

  const canSubmit =
    email.includes("@") && appPassword.trim().length > 0 && !busy;

  const setStatus = (id: number, text: string) =>
    setRowStatus((s) => ({ ...s, [id]: text }));

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!isTauri()) {
      setError("アカウント登録はアプリ(Tauri)上でのみ動作します。");
      return;
    }
    setBusy(true);
    try {
      await addAccount({ email, appPassword, provider, displayName });
      setEmail("");
      setAppPassword("");
      setDisplayName("");
    } catch (err) {
      setError(msgOf(err, "追加に失敗しました"));
    } finally {
      setBusy(false);
    }
  };

  const onTest = async (id: number) => {
    setStatus(id, "接続中…");
    try {
      await testConnection(id);
      setStatus(id, "接続OK ✓");
    } catch (err) {
      setStatus(id, msgOf(err, "接続に失敗しました"));
    }
  };

  const onSync = async (id: number) => {
    setStatus(id, "同期中…");
    try {
      await refreshFolders(id);
      setStatus(id, "フォルダを同期しました ✓");
    } catch (err) {
      setStatus(id, msgOf(err, "同期に失敗しました"));
    }
  };

  const onRemove = async (id: number, label: string) => {
    if (!confirm(`「${label}」を削除しますか?`)) return;
    setError(null);
    try {
      await removeAccount(id);
    } catch (err) {
      setError(msgOf(err, "削除に失敗しました"));
    }
  };

  return (
    <div className="settings-pane">
      <h2>アカウント</h2>

      {accounts.length === 0 ? (
        <p className="muted">登録済みのアカウントはありません。</p>
      ) : (
        <ul className="account-list">
          {accounts.map((a) => (
            <li key={a.id} className="account-row">
              <div className="account-row-main">
                <span>
                  {a.display_name ? `${a.display_name} ` : ""}
                  &lt;{a.email}&gt; · {a.provider}
                </span>
                <div className="account-actions">
                  <button type="button" onClick={() => onTest(a.id)}>
                    接続テスト
                  </button>
                  <button type="button" onClick={() => onSync(a.id)}>
                    同期
                  </button>
                  <button
                    type="button"
                    className="link-danger"
                    onClick={() => onRemove(a.id, a.display_name || a.email)}
                  >
                    削除
                  </button>
                </div>
              </div>
              {rowStatus[a.id] && (
                <p className="muted account-status">{rowStatus[a.id]}</p>
              )}
            </li>
          ))}
        </ul>
      )}

      <form className="account-form" onSubmit={onSubmit}>
        <h3>アカウントを追加</h3>

        <label>
          メールアドレス
          <input
            type="email"
            value={email}
            autoComplete="username"
            placeholder="you@example.com"
            onChange={(e) => setEmail(e.target.value)}
          />
        </label>

        <label>
          アプリパスワード
          <input
            type="password"
            value={appPassword}
            autoComplete="off"
            placeholder="プロバイダで発行したアプリ用パスワード"
            onChange={(e) => setAppPassword(e.target.value)}
          />
        </label>

        <label>
          プロバイダ
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as Provider)}
          >
            <option value="gmail">Gmail</option>
            <option value="icloud">iCloud</option>
          </select>
        </label>
        <p className="muted preset-hint">{PRESET_HINT[provider]}</p>

        <label>
          表示名(任意)
          <input
            type="text"
            value={displayName}
            placeholder="仕事用 など"
            onChange={(e) => setDisplayName(e.target.value)}
          />
        </label>

        {error && <p className="form-error" role="alert">{error}</p>}

        <button type="submit" className="primary" disabled={!canSubmit}>
          {busy ? "追加中…" : "＋ アカウントを追加"}
        </button>
      </form>
    </div>
  );
}
