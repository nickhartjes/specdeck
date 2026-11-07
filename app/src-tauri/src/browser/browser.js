// Claude's browser (`claude-browser` design decision 5): what runs inside the page. The file is one
// expression, a function of the window's private key, evaluated anew for every action so nothing
// depends on globals a page could replace. Every action returns { ok, text }.
(function (key) {
  'use strict';

  const MAX_TEXT = 20000;
  const MAX_CONSOLE = 500;
  const INTERACTIVE_ROLES = ['button', 'link', 'checkbox', 'radio', 'switch', 'tab', 'menuitem', 'option', 'combobox', 'textbox'];
  const SKIPPED = ['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'HEAD', 'svg'];
  const TAURI_REFUSAL = / not allowed\. (Plugin not found|Permissions associated with this command)/;

  // State kept per document: its generation, so numbers from an earlier page are refused, and
  // the console.
  function state() {
    let own = window[key];
    if (!own) {
      own = { generation: Math.random().toString(36).slice(2, 10), console: [] };
      Object.defineProperty(window, key, { value: own, enumerable: false });
    }
    return own;
  }

  const ok = (text) => ({ ok: true, text });
  const fail = (text) => ({ ok: false, text });
  const squash = (text) => (text || '').replace(/\s+/g, ' ').trim();
  const cut = (text, max) => (text.length > max ? text.slice(0, max - 1) + '…' : text);

  function hidden(element) {
    if (element.hidden || element.getAttribute('aria-hidden') === 'true') return true;
    if (typeof element.checkVisibility === 'function') return !element.checkVisibility();
    const style = window.getComputedStyle(element);
    return style.display === 'none' || style.visibility === 'hidden';
  }

  function interactive(element) {
    const tag = element.tagName;
    if (tag === 'A') return element.hasAttribute('href');
    if (tag === 'INPUT') return element.type !== 'hidden';
    if (['BUTTON', 'TEXTAREA', 'SELECT', 'SUMMARY'].includes(tag)) return true;
    if (element.isContentEditable && !element.parentElement?.isContentEditable) return true;
    return INTERACTIVE_ROLES.includes(element.getAttribute('role'));
  }

  function label(element) {
    const aria = element.getAttribute('aria-label');
    if (aria) return squash(aria);
    const by = element.getAttribute('aria-labelledby');
    if (by) {
      const text = by.split(/\s+/).map((id) => document.getElementById(id)?.textContent).join(' ');
      if (squash(text)) return squash(text);
    }
    if (element.labels?.length) return squash([...element.labels].map((l) => l.textContent).join(' '));
    const own = squash(element.innerText ?? element.textContent);
    if (own) return own;
    const named = element.getAttribute('placeholder') || element.getAttribute('title') || element.getAttribute('alt');
    if (named) return squash(named);
    const image = element.querySelector?.('img[alt]');
    return image ? squash(image.getAttribute('alt')) : '';
  }

  function describe(element) {
    const tag = element.tagName;
    const name = cut(label(element), 80);
    const quoted = name ? ` "${name}"` : '';
    if (tag === 'A') return `link${quoted}`;
    if (tag === 'SELECT') {
      const chosen = element.selectedOptions?.[0];
      const options = [...element.options].slice(0, 20).map((o) => squash(o.textContent));
      return `select${quoted} = "${squash(chosen?.textContent)}" (options: ${options.join(', ')})`;
    }
    if (tag === 'TEXTAREA') return `text area${quoted} = "${cut(element.value, 200)}"`;
    if (tag === 'INPUT') {
      const type = element.type;
      if (type === 'checkbox' || type === 'radio') return `${type}${quoted}${element.checked ? ' (checked)' : ''}`;
      if (['button', 'submit', 'reset'].includes(type)) return `button "${squash(element.value) || name}"`;
      // Never repeat a password, even one Claude typed.
      if (type === 'password') return `password field${quoted}${element.value ? ' (filled)' : ''}`;
      return `${type} field${quoted} = "${cut(element.value, 200)}"`;
    }
    if (element.isContentEditable) return `editable text${quoted}`;
    const role = element.getAttribute('role');
    return `${role || tag.toLowerCase()}${quoted}`;
  }

  // The page's text in reading order, with the elements to act on numbered and marked.
  function read() {
    const own = state();
    own.generation = Math.random().toString(36).slice(2, 10);
    for (const marked of document.querySelectorAll('[data-specdeck-ref]')) marked.removeAttribute('data-specdeck-ref');

    const lines = [`Page: ${squash(document.title) || '(no title)'} (${location.href})`, ''];
    let number = 0;
    let line = '';
    const flush = () => {
      if (squash(line)) lines.push(squash(line));
      line = '';
    };
    const visit = (node) => {
      if (node.nodeType === Node.TEXT_NODE) {
        line += ' ' + node.textContent;
        return;
      }
      if (node.nodeType !== Node.ELEMENT_NODE || SKIPPED.includes(node.tagName) || hidden(node)) return;
      if (interactive(node)) {
        number += 1;
        node.setAttribute('data-specdeck-ref', `${own.generation}-${number}`);
        line += ` [${number}] ${describe(node)}`;
        // A button's or link's own text is in its label; a field has none.
        return;
      }
      const block = window.getComputedStyle(node).display !== 'inline' || /^H[1-6]$/.test(node.tagName);
      if (block) flush();
      if (/^H[1-6]$/.test(node.tagName)) line += '#'.repeat(Number(node.tagName[1])) + ' ';
      for (const child of node.childNodes) visit(child);
      if (block) flush();
    };
    if (document.body) visit(document.body);
    flush();
    if (number === 0) lines.push('', 'Nothing on this page to click or type into.');
    const text = lines.join('\n');
    return ok(text.length > MAX_TEXT ? text.slice(0, MAX_TEXT) + '\n(The page is longer; this is its beginning.)' : text);
  }

  function element(number) {
    const own = state();
    const found = document.querySelector(`[data-specdeck-ref="${own.generation}-${Number(number)}"]`);
    return found && found.isConnected ? found : null;
  }

  const stale = (number) => fail(`Element ${number} is not on the page any more. Read the page again for the current numbers.`);

  function click(number) {
    const target = element(number);
    if (!target) return stale(number);
    target.scrollIntoView?.({ block: 'center', inline: 'center' });
    const init = { bubbles: true, cancelable: true, composed: true, button: 0 };
    target.dispatchEvent(new PointerEvent('pointerdown', init));
    target.dispatchEvent(new MouseEvent('mousedown', init));
    if (typeof target.focus === 'function') target.focus();
    target.dispatchEvent(new PointerEvent('pointerup', init));
    target.dispatchEvent(new MouseEvent('mouseup', init));
    target.click();
    return ok(`Clicked [${number}] ${describe(target)}.`);
  }

  // Through the prototype's setter, so frameworks that track the value (React) see the change.
  function setValue(target, value) {
    const prototype = target.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, 'value').set.call(target, value);
  }

  function type(number, text, submit) {
    const target = element(number);
    if (!target) return stale(number);
    const field = ['INPUT', 'TEXTAREA'].includes(target.tagName);
    if (!field && !target.isContentEditable) return fail(`Element ${number} is ${describe(target)}, which does not take text.`);
    target.scrollIntoView?.({ block: 'center' });
    target.focus();
    if (field) setValue(target, text);
    else target.textContent = text;
    target.dispatchEvent(new InputEvent('input', { bubbles: true, composed: true, inputType: 'insertText', data: text }));
    target.dispatchEvent(new Event('change', { bubbles: true }));
    const typed = `Typed into [${number}] ${describe(target)}.`;
    return submit ? ok(`${typed} ${press('Enter').text}`) : ok(typed);
  }

  const FOCUSABLE = 'a[href], button, input:not([type=hidden]), select, textarea, summary, [tabindex]:not([tabindex="-1"]), [contenteditable=true]';

  function press(name) {
    const target = document.activeElement && document.activeElement !== document.body ? document.activeElement : document.body;
    const init = { key: name, bubbles: true, cancelable: true, composed: true };
    const allowed = target.dispatchEvent(new KeyboardEvent('keydown', init));
    let what = `Pressed ${name}`;
    if (allowed && name === 'Enter' && target.form && target.tagName === 'INPUT') {
      target.form.requestSubmit();
      what += ', which submitted the form';
    } else if (allowed && name === 'Enter' && (target.tagName === 'BUTTON' || target.tagName === 'A')) {
      target.click();
    } else if (allowed && (name === 'Tab' || name === 'Shift+Tab')) {
      // A synthetic Tab does not move focus by itself.
      const all = [...document.querySelectorAll(FOCUSABLE)].filter((e) => !hidden(e) && !e.disabled);
      const next = all[(all.indexOf(target) + (name === 'Tab' ? 1 : all.length - 1)) % all.length];
      next?.focus();
    }
    target.dispatchEvent(new KeyboardEvent('keyup', init));
    return ok(`${what}.`);
  }

  // Runs at the start of every page: keeps what the page logs, and its uncaught errors.
  function installConsole() {
    const own = state();
    const keep = (level, parts) => {
      const text = parts
        .map((part) => {
          if (part instanceof Error) return part.stack || String(part);
          if (typeof part === 'object') {
            try {
              return JSON.stringify(part);
            } catch {
              return String(part);
            }
          }
          return String(part);
        })
        .join(' ');
      own.console.push({ level, text: cut(text, 2000) });
      if (own.console.length > MAX_CONSOLE) own.console.splice(0, own.console.length - MAX_CONSOLE);
    };
    for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
      const original = console[level];
      console[level] = function (...parts) {
        keep(level, parts);
        return original.apply(this, parts);
      };
    }
    window.addEventListener('error', (event) => keep('error', [event.error ?? event.message]));
    window.addEventListener('unhandledrejection', (event) => {
      // Tauri puts its plugins' setup scripts in every window it makes, this one too; their calls
      // are refused, since no capability covers it. Not the page's errors, so not Claude's to fix.
      if (TAURI_REFUSAL.test(String(event.reason))) return;
      keep('error', ['Unhandled rejection:', event.reason]);
    });
    return ok('');
  }

  function consoleEntries() {
    const entries = state().console;
    if (entries.length === 0) return ok('The console is empty.');
    return ok(entries.map((entry) => `${entry.level}: ${entry.text}`).join('\n'));
  }

  function page() {
    return ok(`${squash(document.title) || '(no title)'} (${location.href})`);
  }

  return { read, click, type, press, installConsole, consoleEntries, page };
})
