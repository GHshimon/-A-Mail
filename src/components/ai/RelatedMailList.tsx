import { useState } from "react";
import type { MessageHeader } from "@/ipc/types";
import { formatListDate } from "@/lib/formatDate";
import { useUiStore } from "@/store/uiStore";
import {
  hasContent,
  toDraftInput,
  useComposeStore,
} from "@/store/composeStore";
import { saveDraft } from "@/ipc/mail";
import { IpcError } from "@/ipc/client";

interface RelatedMailListProps {
  mails: MessageHeader[];
}

/** B: 関連する過去メール一覧(FTS5 結果を rank 順で併記)。 */
export function RelatedMailList({ mails }: RelatedMailListProps) {
  const selectMessage = useUiStore((s) => s.selectMessage);
  const setMode = useUiStore((s) => s.setMode);
  const [openingId, setOpeningId] = useState<number | null>(null);

  const open = async (id: number) => {
    if (openingId != null) return;
    setOpeningId(id);
    const compose = useComposeStore.getState();
    try {
      if (hasContent(compose.draft)) {
        const input = toDraftInput(compose.draft);
        if (!input) {
          compose.setStatus({ phase: "error", message: "下書き保存にはアカウントの選択が必要です" });
          return;
        }
        compose.setStatus({ phase: "saving" });
        const draftId = await saveDraft(input);
        compose.setDraftId(draftId);
        compose.setStatus({ phase: "saved", at: Date.now() });
      }
      selectMessage(id);
      setMode("read");
    } catch (err) {
      compose.setStatus({
        phase: "error",
        message: err instanceof IpcError ? err.message || err.kind : "下書きを保存できませんでした",
      });
    } finally {
      setOpeningId(null);
    }
  };
  if (mails.length === 0) {
    return (
      <p className="muted" style={{ fontSize: 12 }}>
        関連する過去メールは見つかりませんでした。
      </p>
    );
  }
  return (
    <div className="related">
      {mails.map((m) => (
        <div
          className="rel"
          key={m.id}
          tabIndex={0}
          role="button"
          aria-disabled={openingId != null}
          onClick={() => void open(m.id)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              void open(m.id);
            }
          }}
        >
          <div className="rt">
            <span>{m.subject || "(件名なし)"}</span>
            <span className="d">{formatListDate(m.date)}</span>
          </div>
          <div className="rs">{m.snippet || m.from}</div>
          {openingId === m.id && <div className="score">下書きを保存しています…</div>}
        </div>
      ))}
    </div>
  );
}
