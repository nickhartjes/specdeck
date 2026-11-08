import { useCallback, useEffect, useRef } from 'react';
import type { SharedFileInfo } from '../api/types';

/**
 * Fetches each shared file once for this window and keeps its bytes while it is shared, so
 * opening it again or saving a copy does not fetch it again. A failed fetch is tried again next
 * time; a removed file's bytes are let go.
 */
export function useSharedBytes(files: SharedFileInfo[], fetch: (file: number) => Promise<ArrayBuffer>): (file: number) => Promise<ArrayBuffer> {
  const cache = useRef(new Map<number, Promise<ArrayBuffer>>());
  useEffect(() => {
    const shared = new Set(files.map((file) => file.file));
    for (const file of cache.current.keys()) if (!shared.has(file)) cache.current.delete(file);
  }, [files]);
  return useCallback(
    (file: number) => {
      const cached = cache.current.get(file);
      if (cached) return cached;
      const fetched = fetch(file);
      cache.current.set(file, fetched);
      fetched.catch(() => cache.current.delete(file));
      return fetched;
    },
    [fetch],
  );
}
