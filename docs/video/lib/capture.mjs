// What every video's capture.mjs shares: the UI dev server on :5173 (started when it is not
// running), the installed Google Chrome, a fixed clock, and recording where things are on screen.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';

const BASE = 'http://localhost:5173';
const ui = new URL('../../../app/ui/', import.meta.url).pathname;

// The demo's last timeline event is 13:06 UTC on 31 October 2026; a clock just after it makes
// "for how long" read seconds, not days.
const CLOCK = Date.UTC(2026, 9, 31, 13, 6, 12);

// The demo profile has an update waiting; a how-to video should not show it.
const HIDE = '.update-notice, button[title$="is ready. Restart to update."] { display: none !important; }';

/** Opens the demo for the video in `dir` (its folder URL); shots go to its assets/shots/. */
export async function capture(dir) {
  const shots = new URL('./assets/shots/', dir).pathname;
  mkdirSync(shots, { recursive: true });

  const running = () => fetch(BASE).then(() => true, () => false);
  let server = null;
  if (!(await running())) {
    server = spawn('pnpm', ['exec', 'vite', '--port', '5173', '--strictPort'], { cwd: ui, stdio: 'ignore', detached: true });
    for (let i = 0; !(await running()); i++) {
      if (i === 60) throw new Error('the UI dev server did not start on :5173');
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  }

  const browser = await chromium.launch({ channel: 'chrome' });
  const page = await browser.newPage({ viewport: { width: 1600, height: 900 }, deviceScaleFactor: 2 });
  await page.clock.install({ time: CLOCK });
  // Runs in the page, where globalThis is the window.
  await page.addInitScript((css) => {
    const { document } = globalThis;
    document.addEventListener('DOMContentLoaded', () => {
      const style = document.createElement('style');
      style.textContent = css;
      document.head.append(style);
    });
  }, HIDE);
  const boxes = {};

  return {
    page,
    /** Opens a demo URL such as `/?demo=host` and waits for it to settle. */
    async go(path) {
      await page.goto(BASE + path);
      await page.waitForTimeout(1500);
    },
    /** Records where `locator` is, as fractions of the viewport. */
    async box(name, locator) {
      const b = await locator.first().boundingBox();
      if (!b) throw new Error(`no box for ${name}`);
      const { width, height } = page.viewportSize();
      boxes[name] = { x: b.x / width, y: b.y / height, w: b.width / width, h: b.height / height };
    },
    /** Screenshots the page as `name`, with the pointer out of the way. */
    async shot(name) {
      await page.mouse.move(0, page.viewportSize().height - 1);
      await page.waitForTimeout(500);
      await page.screenshot({ path: `${shots}${name}.png` });
    },
    async close() {
      writeFileSync(`${shots}boxes.json`, JSON.stringify(boxes, null, 2));
      await browser.close();
      // pnpm runs vite as a child, so stop the whole group.
      if (server) process.kill(-server.pid);
      console.log(Object.keys(boxes).join(' '));
    },
  };
}
