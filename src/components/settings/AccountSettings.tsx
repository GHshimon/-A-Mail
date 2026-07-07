import { useState } from "react";
import { useAccountStore } from "@/store/accountStore";
import { isTauri, IpcError } from "@/ipc/client";
import type { Provider } from "@/ipc/types";

/**
 * アカウント設定: 登録済み一覧 + 追加/削除フォーム。
 *
 * メール + アプリパスワードで登録すると、接続設定はプロバイダのプリセットから
 * 解決され、アプリパスワードは Rust 側で Keychain に保存される(DB には残さない)。
 * 実際のログイン検証(IMAP 接続テスト)は後続で `test_connection` に配線する。
 */

const PRESET_HINT: Record<Provider, string> = {
  gmail: "IMAP imap.gmail.com:993 / SMTP smtp.gmail.com:465",
  icloud: "IMAP imap.mail.me.com:993 / SMTP smtp.mail.me.com:587",
};

export function AccountSettings() {
  const accounts = useAccountStore((s) => s.accounts);
  const addAccount = useAccountStore((s) => s.addAccount);
  const removeAccount = useAccountStore((s) => s.removeAccount);

  const [email, setEmail] = useState("");
  const [appPassword, setAppPassword] = useState("");
  const [provider, setProvider] = useState<Provider>("gmail");
  const [displayName, setDisplayName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSubmit =
    email.includes("@") && appPassword.trim().length > 0 && !busy;

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
      setError(err instanceof IpcError ? err.message : "追加に失敗しました");
    } finally {
      setBusy(false);
    }
  };

  const onRemove = async (id: number, label: string) => {
    if (!confirm(`「${label}」を削除しますか?`)) return;
    setError(null);
    try {
      await removeAccount(id);
    } catch (err) {
      setError(err instanceof IpcError ? err.message : "削除に失敗しました");
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
            <li key={a.id}>
              <span>
                {a.display_name ? `${a.display_name} ` : ""}
                &lt;{a.email}&gt; · {a.provider}
              </span>
              <button
                type="button"
                className="link-danger"
                onClick={() => onRemove(a.id, a.display_name || a.email)}
              >
                削除
              </button>
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
