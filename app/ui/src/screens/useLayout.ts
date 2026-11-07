import { useEffect, useRef, useState } from 'react';
import type { Layout } from '../model/layout';

/** How long a layout must stay unchanged before it is saved, so dragging a divider saves once. */
const SAVE_AFTER_MS = 400;

/**
 * This window's layout: changes show at once, and are saved once they settle, and when the
 * session screen closes. Null until the person chose one.
 */
export function useLayout(stored: Layout | null, save: (layout: Layout) => void): [Layout | null, (layout: Layout) => void] {
  const [layout, setLayout] = useState(stored);
  const unsaved = useRef<Layout | null>(null);
  const saveRef = useRef(save);
  useEffect(() => {
    saveRef.current = save;
  });

  useEffect(() => {
    if (!unsaved.current) return;
    const timer = window.setTimeout(() => {
      if (unsaved.current) saveRef.current(unsaved.current);
      unsaved.current = null;
    }, SAVE_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, [layout]);
  useEffect(
    () => () => {
      if (unsaved.current) saveRef.current(unsaved.current);
    },
    [],
  );

  const change = (next: Layout) => {
    unsaved.current = next;
    setLayout(next);
  };
  return [layout, change];
}
