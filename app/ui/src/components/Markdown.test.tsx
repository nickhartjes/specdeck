import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { describe, expect, it, vi } from 'vitest';
import { fakeBridge, renderWith } from '../test/helpers';
import { Markdown } from './Markdown';

describe('markdown', () => {
  it('leaves out HTML comments, such as the notes in OpenSpec templates', () => {
    const { container } = render(<Markdown text={'### New Capabilities\n\n<!-- None: documentation only. -->\n\nText <!-- inline --> after.'} />);
    expect(container).not.toHaveTextContent('<!--');
    expect(container).not.toHaveTextContent('documentation only');
    expect(screen.getByText(/Text\s+after\./)).toBeInTheDocument();
  });

  it('keeps other angle-bracket text the agent wrote', () => {
    const { container } = render(<Markdown text={'Try "write a test for <filepath>"'} />);
    expect(container).toHaveTextContent('write a test for <filepath>');
  });
});

describe('links in markdown', () => {
  function links(text: string, openContentLink = vi.fn(async () => {})) {
    const bridge = fakeBridge('host', { openContentLink });
    renderWith(bridge, <Markdown text={text} />);
    return openContentLink;
  }

  it('open through the app on a click, and the window stays', async () => {
    const open = links('Read [the docs](https://example.com/guide).');
    const link = screen.getByRole('link', { name: 'the docs' });
    // The address shows on hover and stays readable and copyable.
    expect(link).toHaveAttribute('href', 'https://example.com/guide');
    expect(link).toHaveAttribute('title', 'https://example.com/guide');
    const followed = vi.fn();
    document.addEventListener('click', (event) => followed(event.defaultPrevented), { once: true });
    await userEvent.click(link);
    expect(open).toHaveBeenCalledWith('https://example.com/guide');
    expect(followed).toHaveBeenCalledWith(true);
  });

  it('to a place in the page or a relative path open nothing', async () => {
    const open = links('See [the heading](#heading) and [the design](design.md).');
    const followed: boolean[] = [];
    const listen = (event: Event) => followed.push(event.defaultPrevented);
    document.addEventListener('click', listen);
    await userEvent.click(screen.getByRole('link', { name: 'the heading' }));
    await userEvent.click(screen.getByRole('link', { name: 'the design' }));
    document.removeEventListener('click', listen);
    expect(open).not.toHaveBeenCalled();
    expect(followed).toEqual([true, true]);
  });

  it('say so when the app refuses them, and keep their address', async () => {
    const open = links('Try [the app](http://localhost:3000/) or [this](file:///etc/passwd).', vi.fn(async () => Promise.reject("This link can't be opened.")));
    await userEvent.click(screen.getByRole('link', { name: 'the app' }));
    expect(await screen.findByRole('status')).toHaveTextContent("This link can't be opened.");
    expect(screen.getByRole('link', { name: 'this' })).toHaveAttribute('title', 'file:///etc/passwd');
    await userEvent.click(screen.getByRole('link', { name: 'this' }));
    expect(open).toHaveBeenCalledWith('file:///etc/passwd');
  });

  it('never carry an address that runs script', () => {
    links('[run](javascript:alert(1)) [page](data:text/html,x)');
    expect(screen.getByText('run').closest('a')).toHaveAttribute('href', '');
    expect(screen.getByText('page').closest('a')).toHaveAttribute('href', '');
  });

  it('pass the accessibility check, with a refusal shown', async () => {
    links('Read [the docs](https://example.com/guide).', vi.fn(async () => Promise.reject("This link can't be opened.")));
    await userEvent.click(screen.getByRole('link', { name: 'the docs' }));
    await screen.findByRole('status');
    const results = await axe.run(document.body, { rules: { 'color-contrast': { enabled: false }, region: { enabled: false } } });
    expect(results.violations).toEqual([]);
  });
});
