import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { BridgeContext } from '../api/bridge';
import type { SoFar } from '../model/catchUp';
import type { LinkItem, Turn } from '../model/timeline';
import { fakeBridge } from '../test/helpers';
import { CatchUpCard } from './CatchUpCard';

const soFar: SoFar = {
  ranMs: 40 * 60_000,
  prompts: 6,
  plans: [{ change: 'viewer-heartbeat', documents: [], tasks: { done: 3, total: 6 } }],
  documents: ['openspec/changes/viewer-heartbeat/proposal.md', 'openspec/changes/viewer-heartbeat/design.md'],
  shared: [
    { file: 1, name: 'wireframe.png' },
    { file: 2, name: 'old.log' },
  ],
  links: [1, 2],
  codeFiles: 12,
};

const link = (linkId: number, url: string | null, title: string): LinkItem => ({ kind: 'link', id: `link-${linkId}`, linkId, url, title, by: 'Nick', sessionOnly: false, at: 0, outputSeq: 0 });
const turns: Turn[] = [1, 2].map((n) => ({ promptId: `p${n}`, prompt: `Prompt ${n}`, at: 0, outputSeq: n, items: [], response: null, respondedAt: null, tokens: null, revision: 0 }));

function card(overrides: Partial<Parameters<typeof CatchUpCard>[0]> = {}) {
  const bridge = fakeBridge('viewer');
  const props = {
    soFar,
    turns,
    sharedNow: new Set([1]),
    links: [link(1, 'https://excalidraw.com/', 'Excalidraw room'), link(2, null, 'Old board')],
    onJump: vi.fn(),
    onLatest: vi.fn(),
    onClose: vi.fn(),
    onReadPlan: vi.fn(),
    onOpenDocument: vi.fn(),
    onOpenShared: vi.fn(),
    ...overrides,
  };
  render(
    <BridgeContext.Provider value={bridge}>
      <CatchUpCard {...props} />
    </BridgeContext.Provider>,
  );
  return { ...props, bridge, region: screen.getByRole('region', { name: 'So far' }) };
}

describe('the so far card', () => {
  it('sums up what happened before joining', () => {
    const { region } = card();
    expect(region).toHaveTextContent('You joined 40 minutes in. So far: 6 prompts.');
    expect(region).toHaveTextContent('viewer-heartbeat · 3 of 6 tasks');
    expect(region).toHaveTextContent('12 code files changed');
    // A file and a link removed since are left out.
    expect(within(region).getByRole('button', { name: 'wireframe.png' })).toBeInTheDocument();
    expect(within(region).queryByText('old.log')).not.toBeInTheDocument();
    expect(within(region).getByRole('button', { name: 'Excalidraw room' })).toBeInTheDocument();
    expect(within(region).queryByText('Old board')).not.toBeInTheDocument();
  });

  it('opens plans, documents, shared files and links', async () => {
    const { region, onReadPlan, onOpenDocument, onOpenShared, bridge } = card();
    await userEvent.click(within(region).getByRole('button', { name: 'Read the plan' }));
    expect(onReadPlan).toHaveBeenCalledWith('viewer-heartbeat', []);
    await userEvent.click(within(region).getByRole('button', { name: 'design.md' }));
    expect(onOpenDocument).toHaveBeenCalledWith('openspec/changes/viewer-heartbeat/design.md');
    await userEvent.click(within(region).getByRole('button', { name: 'wireframe.png' }));
    expect(onOpenShared).toHaveBeenCalledWith(1, 'wireframe.png');
    await userEvent.click(within(region).getByRole('button', { name: 'Excalidraw room' }));
    expect(bridge.openLink).toHaveBeenCalledWith(1);
  });

  it('shows five of a long list and how many more', () => {
    const documents = Array.from({ length: 9 }, (_, i) => `doc${i}.md`);
    const { region } = card({ soFar: { ...soFar, documents } });
    expect(within(region).getByRole('button', { name: 'doc4.md' })).toBeInTheDocument();
    expect(within(region).queryByRole('button', { name: 'doc5.md' })).not.toBeInTheDocument();
    expect(region).toHaveTextContent('and 4 more');
  });

  it('jumps, goes to the latest prompt and closes', async () => {
    const { region, onJump, onLatest, onClose } = card();
    await userEvent.selectOptions(within(region).getByLabelText('Jump to a prompt'), 'p2');
    expect(onJump).toHaveBeenCalledWith('p2');
    await userEvent.click(within(region).getByRole('button', { name: 'Go to the latest prompt' }));
    expect(onLatest).toHaveBeenCalled();
    await userEvent.click(within(region).getByRole('button', { name: 'Close the overview' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('says when the viewer joined right after the start', () => {
    const { region } = card({ soFar: { ...soFar, ranMs: 20_000, prompts: 1 } });
    expect(region).toHaveTextContent('You joined just after the start. So far: 1 prompt.');
  });
});
