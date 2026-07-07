import { useAccountStore } from "@/store/accountStore";
import { FolderTree } from "./FolderTree";

/**
 * サイドバー本体。登録アカウントごとに、メールアドレス見出し + フォルダツリーを
 * 並べる。M0 ではモックデータ、M1 以降は IPC 取得に切り替わる。
 */
export function AccountList() {
  const accounts = useAccountStore((s) => s.accounts);
  const foldersByAccount = useAccountStore((s) => s.foldersByAccount);

  return (
    <nav className="sidebar" aria-label="アカウントとフォルダ">
      {accounts.map((a) => (
        <div key={a.id}>
          <div className="acct" title={a.email}>
            {a.email}
          </div>
          <FolderTree folders={foldersByAccount[a.id] ?? []} />
        </div>
      ))}
    </nav>
  );
}
