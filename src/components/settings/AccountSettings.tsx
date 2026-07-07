import { useAccountStore } from "@/store/accountStore";

/**
 * アカウント設定(一覧表示 + 追加/削除の導線)。
 *
 * M0 は登録済みアカウントの表示のみ。追加フロー(メール + アプリパスワード →
 * Keychain 保存 + 接続テスト)は M1 で `add_account` に配線する。
 */
export function AccountSettings() {
  const accounts = useAccountStore((s) => s.accounts);

  return (
    <div className="settings-pane">
      <h2>アカウント</h2>
      <ul>
        {accounts.map((a) => (
          <li key={a.id}>
            {a.display_name} &lt;{a.email}&gt; · {a.provider}
          </li>
        ))}
      </ul>
      <button className="primary" title="アカウントを追加(M1)">
        ＋ アカウントを追加
      </button>
    </div>
  );
}
