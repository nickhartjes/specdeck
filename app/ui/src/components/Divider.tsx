import { useRef, useState } from 'react';
import { MAX_PANEL, MIN_PANEL } from '../model/layout';

interface Props {
  /** What it resizes, for screen readers: "files" or "timeline". */
  name: string;
  width: number;
  /** Which way dragging to the right moves the width: +1 for a panel on the left, -1 on the right. */
  side: 1 | -1;
  onWidth: (width: number) => void;
  /** Back to the preset's usual width. */
  onReset: () => void;
  /** A drag starts or ends; the host's terminal is resized only at the end. */
  onDragging?: (dragging: boolean) => void;
}

/** The divider beside a side panel: drag it, or focus it and use the arrow keys. */
export function Divider({ name, width, side, onWidth, onReset, onDragging }: Props) {
  const [dragging, setDragging] = useState(false);
  const start = useRef({ x: 0, width });
  const end = () => {
    setDragging(false);
    onDragging?.(false);
  };
  return (
    <div
      className={`divider${dragging ? ' dragging' : ''}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize the ${name} panel`}
      aria-valuenow={width}
      aria-valuemin={MIN_PANEL}
      aria-valuemax={MAX_PANEL}
      tabIndex={0}
      onKeyDown={(event) => {
        const step = event.shiftKey ? 64 : 16;
        if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
          event.preventDefault();
          onWidth(width + side * (event.key === 'ArrowRight' ? step : -step));
        }
      }}
      onDoubleClick={onReset}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture?.(event.pointerId);
        start.current = { x: event.clientX, width };
        setDragging(true);
        onDragging?.(true);
      }}
      onPointerMove={(event) => dragging && onWidth(start.current.width + side * (event.clientX - start.current.x))}
      onPointerUp={end}
      onPointerCancel={end}
    />
  );
}
