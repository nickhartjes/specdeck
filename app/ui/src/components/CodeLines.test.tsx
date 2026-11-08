import { render, screen } from '@testing-library/react';
import axe from 'axe-core';
import { describe, expect, it } from 'vitest';
import { tokenizeLines } from '../model/highlight';
import { CodeLines } from './CodeLines';

describe('code lines', () => {
  it('numbers the lines and colours the code', () => {
    const { container } = render(<CodeLines lines={tokenizeLines('fn a() {}\n// note', 'rust')} label="Whole file" />);
    const lines = container.querySelectorAll('.ln');
    expect(lines).toHaveLength(2);
    expect(lines[0]!.querySelector('.n')!.textContent).toBe('1');
    expect(lines[0]!.querySelector('.tok-kw')!.textContent).toBe('fn');
    expect(lines[1]!.querySelector('.tok-com')!.textContent).toBe('// note');
    expect(screen.getByRole('group', { name: 'Whole file' })).toBeInTheDocument();
  });

  it('marks changed lines with a sign and words, not colour alone', () => {
    const { container } = render(
      <CodeLines lines={tokenizeLines('a\nb\nc', null)} marks={[null, 'added', 'changed']} removed={[0, 0, 0, 2]} label="Whole file" />,
    );
    const lines = container.querySelectorAll('.ln');
    expect(lines[0]!.className).toBe('ln');
    expect(lines[1]!.className).toBe('ln mark-added');
    expect(lines[1]!.textContent).toBe('2+Added in this session: b');
    expect(lines[2]!.textContent).toContain('Changed in this session');
    expect(screen.getByText('2 lines removed')).toBeInTheDocument();
  });

  it('shows markup in a file as text, and runs nothing', () => {
    const text = '<script>window.ran = true</script>\n<img src=x onerror="window.ran = true">';
    const { container } = render(<CodeLines lines={tokenizeLines(text, 'markup')} label="Whole file" />);
    expect(container.querySelector('script, img')).toBeNull();
    expect(container.textContent).toContain('<script>window.ran = true</script>');
    expect((window as { ran?: boolean }).ran).toBeUndefined();
  });

  it('passes axe', async () => {
    render(
      <main>
        <CodeLines lines={tokenizeLines('a\nb', 'rust')} marks={['added', null]} removed={[0, 1, 0]} label="Whole file" />
      </main>,
    );
    const results = await axe.run(document.body, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
