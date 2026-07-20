import type { Folder, FolderRole } from "@/ipc/types";
import { useUiStore } from "@/store/uiStore";

const ROLE_ICON: Record<FolderRole, string> = {
  inbox: "📥",
  starred: "⭐️",
  sent: "📤",
  drafts: "📝",
  archive: "🗄",
  trash: "🗑",
  junk: "🚫",
  custom: "📁",
};

function iconFor(role: FolderRole | null): string {
  return role ? ROLE_ICON[role] : ROLE_ICON.custom;
}

interface FolderTreeProps {
  folders: Folder[];
}

/** 1 アカウント分のフォルダ一覧。 */
export function FolderTree({ folders }: FolderTreeProps) {
  const selectedFolderId = useUiStore((s) => s.selectedFolderId);
  const selectFolder = useUiStore((s) => s.selectFolder);
  const selectAccount = useUiStore((s) => s.selectAccount);
  const setMode = useUiStore((s) => s.setMode);

  return (
    <ul className="nav">
      {folders.map((f) => (
        <li
          key={f.id}
          className={f.id === selectedFolderId ? "active" : undefined}
          tabIndex={0}
          onClick={() => {
            selectAccount(f.account_id);
            selectFolder(f.id);
            setMode("read");
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              selectAccount(f.account_id);
              selectFolder(f.id);
              setMode("read");
            }
          }}
        >
          <span className="ic">{iconFor(f.role)}</span>
          <span className="lbl">{f.name}</span>
          {f.unread > 0 && <span className="count">{f.unread}</span>}
        </li>
      ))}
    </ul>
  );
}
