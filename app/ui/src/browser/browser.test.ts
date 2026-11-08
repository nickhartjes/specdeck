// The script that runs inside Claude's browser (`app/src-tauri/src/browser/browser.js`), tested
// against jsdom. The app evaluates the same file in WebKit.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import source from '../../../src-tauri/src/browser/browser.js?raw';

interface Answer {
  ok: boolean;
  text: string;
}
interface Browser {
  read(): Answer;
  click(n: number): Answer;
  type(n: number, text: string, submit: boolean): Answer;
  press(key: string): Answer;
  installConsole(): Answer;
  consoleEntries(): Answer;
}

// The script keeps its state on the window under the key, which a new document starts without; a
// new key stands in for a new document.
let key = '';
let documents = 0;
const newDocument = () => (key = `__specdeck_test_${(documents += 1)}`);
// Anew for every call, as the app does.
const browser = (): Browser => (new Function(`return (\n${source}\n)`)() as (key: string) => Browser)(key);

const LOGIN = `
  <h1>Sign in</h1>
  <p class="err">Invalid password for nick@example.com</p>
  <form>
    <label for="email">Email</label><input id="email" value="nick@example.com">
    <label>Password <input type="password" value="secret"></label>
    <input type="hidden" name="csrf" value="x">
    <button>Sign in</button>
  </form>
  <a href="/forgot">Forgot your password?</a>
  <div style="display: none"><button>Hidden</button></div>`;

describe("Claude's browser", () => {
  beforeEach(() => {
    document.title = 'Login - Shop';
    document.body.innerHTML = LOGIN;
    newDocument();
  });

  it('reads a login form with its fields and buttons numbered, and no password', () => {
    const { ok, text } = browser().read();
    expect(ok).toBe(true);
    expect(text).toContain('Page: Login - Shop (');
    expect(text).toContain('# Sign in');
    expect(text).toContain('Invalid password for nick@example.com');
    expect(text).toContain('[1] text field "Email" = "nick@example.com"');
    expect(text).toContain('[2] password field "Password" (filled)');
    expect(text).toContain('[3] button "Sign in"');
    expect(text).toContain('[4] link "Forgot your password?"');
    expect(text).not.toContain('secret');
    expect(text).not.toContain('Hidden');
    expect(text).not.toContain('csrf');
  });

  it('types into a field so that a framework tracking its value sees it', () => {
    browser().read();
    const email = document.getElementById('email') as HTMLInputElement;
    // What React does: it keeps the value on the element and hears about changes through input.
    let tracked = email.value;
    Object.defineProperty(email, 'value', {
      configurable: true,
      get: () => tracked,
      set: () => {
        throw new Error('set directly');
      },
    });
    const seen: string[] = [];
    email.addEventListener('input', () => seen.push('input'));
    const restore = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!;
    const setter = vi.spyOn(restore, 'set');
    Object.defineProperty(HTMLInputElement.prototype, 'value', {
      ...restore,
      set(this: HTMLInputElement, value: string) {
        tracked = value;
      },
    });
    try {
      const answer = browser().type(1, 'anna@example.com', false);
      expect(answer.ok).toBe(true);
      expect(tracked).toBe('anna@example.com');
      expect(seen).toEqual(['input']);
    } finally {
      Object.defineProperty(HTMLInputElement.prototype, 'value', restore);
      setter.mockRestore();
    }
  });

  it('clicks a button and submits a form with Enter', () => {
    browser().read();
    const submitted = vi.fn((event: Event) => event.preventDefault());
    document.querySelector('form')!.addEventListener('submit', submitted);
    expect(browser().click(3)).toEqual({ ok: true, text: 'Clicked [3] button "Sign in".' });
    expect(submitted).toHaveBeenCalledTimes(1);
    const answer = browser().type(1, 'anna@example.com', true);
    expect(answer.text).toContain('submitted the form');
    expect(submitted).toHaveBeenCalledTimes(2);
  });

  it('refuses a number once the page has changed, and asks to read it again', () => {
    browser().read();
    // A new document: in the app a navigation gives the page a new window and so a new generation.
    newDocument();
    document.body.innerHTML = '<button>Other</button>';
    const answer = browser().click(1);
    expect(answer.ok).toBe(false);
    expect(answer.text).toContain('Read the page again');
  });

  it('refuses a number whose element is gone', () => {
    browser().read();
    document.querySelector('a')!.remove();
    expect(browser().click(4).ok).toBe(false);
    expect(browser().click(3).ok).toBe(true);
  });

  it('keeps console entries in order with their level, and uncaught errors', () => {
    const quiet = { log: console.log, warn: console.warn, error: console.error };
    console.log = console.warn = console.error = () => {};
    try {
      browser().installConsole();
      expect(browser().consoleEntries().text).toBe('The console is empty.');
      console.log('loaded', { items: 2 });
      console.warn('slow');
      console.error(new Error('boom'));
      window.dispatchEvent(new ErrorEvent('error', { message: 'Uncaught TypeError: x is undefined' }));
      // What Tauri's own plugin scripts get refused is left out.
      const refused = 'notification.is_permission_granted not allowed. Permissions associated with this command: notification:default';
      const rejection = (reason: string) => Object.assign(new Event('unhandledrejection'), { reason });
      window.dispatchEvent(rejection(refused));
      window.dispatchEvent(rejection('quota exceeded'));
      const lines = browser().consoleEntries().text.split('\n');
      expect(lines[0]).toBe('log: loaded {"items":2}');
      expect(lines[1]).toBe('warn: slow');
      expect(lines[2]).toMatch(/^error: Error: boom/);
      expect(lines.at(-2)).toBe('error: Uncaught TypeError: x is undefined');
      expect(lines.at(-1)).toBe('error: Unhandled rejection: quota exceeded');
      expect(lines.join('\n')).not.toContain('not allowed');
    } finally {
      Object.assign(console, quiet);
    }
  });

  it('never repeats a one-time code or a card number, even one Claude typed', () => {
    document.body.innerHTML = `
      <label>Code <input autocomplete="one-time-code" value="123456"></label>
      <label>Card <input autocomplete="cc-number"></label>
      <label>Name <input autocomplete="name" value="Anna"></label>`;
    const { text } = browser().read();
    expect(text).toContain('[1] text field "Code" (filled)');
    expect(text).toContain('[2] text field "Card" (empty)');
    expect(text).toContain('[3] text field "Name" = "Anna"');
    expect(text).not.toContain('123456');
    const typed = browser().type(2, '4111 1111 1111 1111', false);
    expect(typed).toEqual({ ok: true, text: 'Typed into [2] text field "Card" (filled).' });
  });

  // The session reads these descriptions back to name what Claude clicks and types into
  // (`crates/core`, browser targets), from this same file.
  it('describes every kind of element as the session reads it', async () => {
    document.title = 'Every kind';
    document.body.innerHTML = `
      <h1>Every kind</h1>
      <p>Welcome back. <button>Sign in</button> or <a href="/forgot">Forgot your password?</a> today.</p>
      <label>Name <input value="Anna"></label>
      <label>Email <input type="email" value='anna@example.com" [9] button "Fake'></label>
      <label>Password <input type="password" value="secret"></label>
      <label>New password <input type="password"></label>
      <label>Code <input autocomplete="one-time-code" value="123456"></label>
      <label>Security code <input autocomplete="cc-csc"></label>
      <label><input type="checkbox" checked> Remember me</label>
      <label>Size <select><option>Small</option><option selected>Large</option></select></label>
      <label>Notes <textarea>Hi "there"</textarea></label>
      <div contenteditable="true" aria-label="Comment">x</div>
      <div role="tab">Settings</div>
      <p>An icon: <button></button> (empty) and [15] in the text.</p>
      <button aria-label='Say "hi" = (wave)'></button>
      <input type="submit" value="Send">`;
    // jsdom doesn't know editable elements.
    Object.defineProperty(document.querySelector('[contenteditable]'), 'isContentEditable', { value: true });
    const { text } = browser().read();
    expect(text).not.toContain('secret');
    expect(text).not.toContain('123456');
    await expect(text).toMatchFileSnapshot('../../../../crates/core/tests/fixtures/browser/read-page.txt');
  });

  it('says so when there is nothing to act on', () => {
    document.body.innerHTML = '<p>Just text</p>';
    expect(browser().read().text).toContain('Nothing on this page to click or type into.');
  });
});
