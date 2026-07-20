import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";

interface ResizablePaneProps {
  /** 現在の幅 px。 */
  width: number;
  /** ドラッグ確定時に新しい幅を通知。 */
  onResize: (width: number) => void;
  /** ハンドルを左右どちらに置くか。 */
  side?: "right" | "left";
  className?: string;
  children: ReactNode;
}

/**
 * ペイン幅をドラッグでリサイズできるラッパ。
 * ハンドルは絶対配置の細い掴み代。実幅は親グリッドの列テンプレートが持ち、
 * ここでは onResize で通知するだけ(uiStore が clamp して保持)。
 */
export function ResizablePane({
  width,
  onResize,
  side = "right",
  className,
  children,
}: ResizablePaneProps) {
  const [dragging, setDragging] = useState(false);
  const startX = useRef(0);
  const startW = useRef(0);

  const onPointerDown = useCallback(
    (e: ReactPointerEvent) => {
      e.preventDefault();
      startX.current = e.clientX;
      startW.current = width;
      setDragging(true);
    },
    [width],
  );

  useEffect(() => {
    if (!dragging) return;
    const onMove = (e: PointerEvent) => {
      const delta = e.clientX - startX.current;
      const next = side === "right" ? startW.current + delta : startW.current - delta;
      onResize(next);
    };
    const onUp = () => setDragging(false);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [dragging, onResize, side]);

  return (
    <div className={`pane-cell${className ? ` ${className}` : ""}`}>
      {children}
      <div
        className={`resizer${dragging ? " dragging" : ""}`}
        style={side === "right" ? { right: 0 } : { left: 0 }}
        onPointerDown={onPointerDown}
        role="separator"
        aria-orientation="vertical"
      />
    </div>
  );
}
