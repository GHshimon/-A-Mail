import { useEffect, useRef } from "react";

/**
 * A(予測入力)のデバウンス土台。
 *
 * 入力停止から `delay` ms 後にコールバックを 1 回だけ発火する。IME 変換中
 * (composing)はトリガしない、という制御は呼び出し側の GhostTextEditor で行う。
 *
 * M0 では骨組みのみ。実際の Gemini ストリーミング購読(Tauri Channel)は M3。
 */
export function useDebouncedCompletion(
  value: string,
  onTrigger: (value: string) => void,
  opts: { delay?: number; enabled?: boolean } = {},
): void {
  const { delay = 300, enabled = true } = opts;
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cb = useRef(onTrigger);
  cb.current = onTrigger;

  useEffect(() => {
    if (!enabled) return;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => cb.current(value), delay);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [value, delay, enabled]);
}
