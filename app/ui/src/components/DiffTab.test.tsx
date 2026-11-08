import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { DiffHunk } from '../api/types';
import type { FileChange, TouchedFile } from '../model/timeline';
import { DiffTab } from './DiffTab';

const v1 = 'fn a() {}\nfn B() {}\nfn c() {}';
const v2 = 'fn a() {}\nfn B() {}\nfn c() {}\nfn d() {}';
const edit1: DiffHunk[] = [{ oldStart: 1, oldLines: 3, newStart: 1, newLines: 3, lines: [' fn a() {}', '-fn b() {}', '+fn B() {}', ' fn c() {}'] }];
const edit2: DiffHunk[] = [{ oldStart: 3, oldLines: 1, newStart: 3, newLines: 2, lines: [' fn c() {}', '+fn d() {}'] }];

const change = (toolUseId: string, hunks: DiffHunk[], kind: FileChange['change'] = 'edited'): FileChange => ({
  path: 'src/session.rs',
  change: kind,
  hunks,
  promptId: 'p',
  toolUseId,
  at: 0,
  outputSeq: 7,
});

const file = (...changes: FileChange[]): TouchedFile => ({ path: 'src/session.rs', readOnly: false, changes });
const whole = () => screen.getByRole('button', { name: 'Whole file, changes marked' });
const changes = () => screen.getByRole('button', { name: 'Changes' });

describe('diff tab', () => {
  it('opens on the changes, and switches to the whole file with the changes marked', async () => {
    const onView = vi.fn();
    const { container } = render(
      <DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={new Map([['t1', v1]])} wholeFiles onView={onView} />,
    );
    expect(changes()).toHaveAttribute('aria-pressed', 'true');
    expect(container.querySelectorAll('.diff')).toHaveLength(1);
    await userEvent.click(whole());
    expect(onView).toHaveBeenCalledWith('whole');
    expect(whole()).toHaveAttribute('aria-pressed', 'true');
    const lines = [...container.querySelectorAll('.code-lines .ln')];
    expect(lines.map((line) => line.className)).toEqual(['ln', 'ln mark-changed', 'ln']);
    expect(lines[1]!.querySelector('.tok-kw')!.textContent).toBe('fn');
  });

  it('follows the file as it changes', () => {
    const texts = new Map([['t1', v1]]);
    const { container, rerender } = render(<DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={texts} wholeFiles view="whole" />);
    expect(container.querySelectorAll('.code-lines .ln')).toHaveLength(3);
    rerender(<DiffTab file={file(change('t1', edit1), change('t2', edit2))} onJump={() => {}} texts={new Map([...texts, ['t2', v2]])} wholeFiles view="whole" />);
    const marks = [...container.querySelectorAll('.code-lines .ln')].map((line) => line.className);
    expect(marks).toEqual(['ln', 'ln mark-changed', 'ln', 'ln mark-added']);
  });

  it('says that the host shares only the changes', async () => {
    render(<DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={new Map()} wholeFiles={false} />);
    expect(screen.getByText('The host shares only the changes.')).toBeInTheDocument();
    expect(whole()).toHaveAttribute('aria-disabled', 'true');
    expect(whole()).toHaveAccessibleDescription('The host shares only the changes.');
    await userEvent.click(whole());
    expect(changes()).toHaveAttribute('aria-pressed', 'true');
  });

  it('tells the host where to change it', () => {
    render(<DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={new Map()} wholeFiles={false} host />);
    expect(screen.getByText(/You share only the changes/)).toBeInTheDocument();
  });

  it('says why a file is not whole while whole files are shared', () => {
    const { rerender } = render(<DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={new Map()} wholeFiles />);
    expect(screen.getByText(/changed while only the changes were shared, or it is private/)).toBeInTheDocument();
    rerender(<DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={new Map([['t1', null]])} wholeFiles />);
    expect(screen.getByText('Too large to show whole.')).toBeInTheDocument();
  });

  it('says when earlier changes are not marked', () => {
    render(<DiffTab file={file(change('t1', edit1), change('t2', edit2))} onJump={() => {}} texts={new Map([['t2', v2]])} wholeFiles view="whole" />);
    expect(screen.getByText(/The first change is not marked/)).toBeInTheDocument();
  });

  it('shows the change a step points to, whatever the file was left on', () => {
    const { container, rerender } = render(<DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={new Map([['t1', v1]])} wholeFiles view="whole" />);
    expect(container.querySelector('.code-lines')).not.toBeNull();
    rerender(<DiffTab file={file(change('t1', edit1))} onJump={() => {}} texts={new Map([['t1', v1]])} wholeFiles view="whole" focus={{ toolUseId: 't1', n: 1 }} />);
    expect(container.querySelector('.code-lines')).toBeNull();
    expect(container.querySelector('[data-tool="t1"]')).not.toBeNull();
  });
});
