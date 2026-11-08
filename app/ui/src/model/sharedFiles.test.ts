import { describe, expect, it } from 'vitest';
import type { TimelineEvent } from '../api/types';
import { dataUrl, formatSize, refusal, sharedBy, shareSteps, textOf } from './sharedFiles';

const file = (name: string, size = 10, type = '') => ({ name, size, type });

describe('shared files in the window', () => {
  it('refuses before sending what the host would refuse anyway', () => {
    expect(refusal(file('wireframe.png', 240_000, 'image/png'))).toBeNull();
    expect(refusal(file('ci-failure.log'))).toBeNull();
    expect(refusal(file('notes', 5))).toBeNull();
    expect(refusal(file('big.log', 10 * 1024 * 1024 + 1))).toMatch('larger than 10 MB');
    expect(refusal(file('empty.txt', 0))).toMatch('empty');
    expect(refusal(file('logo.svg', 10, 'image/svg+xml'))).toMatch("can't be shared");
    expect(refusal(file('photo.heic', 10, 'image/heic'))).toMatch("can't be shared");
    expect(refusal(file('spec.pdf', 10, 'application/pdf'))).toMatch("can't be shared");
    expect(refusal(file('tool.exe'))).toMatch("can't be shared");
  });

  it('says sizes as people read them', () => {
    expect(formatSize(900)).toBe('900 bytes');
    expect(formatSize(18 * 1024)).toBe('18 KB');
    expect(formatSize(2.4 * 1024 * 1024)).toBe('2.4 MB');
    expect(formatSize(10 * 1024 * 1024)).toBe('10 MB');
  });

  it('makes a data URL of any size, and reads text as UTF-8', () => {
    const big = new Uint8Array(200_000).fill(65);
    expect(dataUrl('image/png', big.buffer)).toMatch(/^data:image\/png;base64,QUFB/);
    expect(textOf(new TextEncoder().encode('Ünïcødé').buffer)).toBe('Ünïcødé');
  });

  it('names who shared a file', () => {
    expect(sharedBy({ by: 2, byLabel: 'Sanne' }, 2)).toBe('You');
    expect(sharedBy({ by: 2, byLabel: 'Sanne' }, 0)).toBe('Sanne');
    expect(sharedBy({ by: 0, byLabel: '' }, 3)).toBe('The host');
  });

  it('takes the shares from the timeline', () => {
    const events: TimelineEvent[] = [
      { outputSeq: 0, at: 1, kind: 'prompt', promptId: 'p', text: 'go' },
      { outputSeq: 0, at: 2, kind: 'file-shared', file: 4, name: 'a.md', fileKind: 'markdown', mediaType: 'text/markdown', size: 3, sha256: '', by: 2, byLabel: 'Sanne' },
    ];
    expect(shareSteps(events)).toEqual([{ file: 4, name: 'a.md', by: 2, byLabel: 'Sanne', at: 2 }]);
  });
});
