// Screenshots of the UI's demo mode for the diagnostics video, plus where the parts to highlight
// are. Uses the UI dev server on :5173, starting it when it is not running, and the installed
// Google Chrome. Writes assets/shots/*.png and assets/shots/boxes.json.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';

const dir = new URL('./assets/shots/', import.meta.url).pathname;
const ui = new URL('../../../app/ui/', import.meta.url).pathname;
const BASE = 'http://localhost:5173';
mkdirSync(dir, { recursive: true });

const running = () => fetch(BASE).then(() => true, () => false);
let server = null;
if (!(await running())) {
  server = spawn('pnpm', ['exec', 'vite', '--port', '5173', '--strictPort'], { cwd: ui, stdio: 'ignore', detached: true });
  for (let i = 0; !(await running()); i++) {
    if (i === 60) throw new Error('the UI dev server did not start on :5173');
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
}

const W = 1600;
const H = 900;
const browser = await chromium.launch({ channel: 'chrome' });
const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 2 });
const boxes = {};

const box = async (name, locator) => {
  const b = await locator.first().boundingBox();
  if (!b) throw new Error(`no box for ${name}`);
  const { width, height } = page.viewportSize();
  boxes[name] = { x: b.x / width, y: b.y / height, w: b.width / width, h: b.height / height };
};
const shot = async (name) => {
  await page.mouse.move(0, page.viewportSize().height - 1);
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${dir}${name}.png` });
};
const open = async (url) => {
  await page.goto(`${BASE}${url}`);
  await page.waitForTimeout(1500);
};
const statusPanel = async () => {
  await page.evaluate(() => globalThis.demoMenu({ id: 'connection-status' }));
  await page.locator('.connection-status .status-details').waitFor();
  await page.waitForTimeout(1200);
};

// The host.
await open('/?demo=host');
await shot('host');
await statusPanel();
const panel = page.locator('.connection-status');
await box('host.panel', panel);
await box('host.verdict', panel.locator('.verdict'));
await box('host.viewer1', panel.locator('dl[aria-label="Viewer 1"]'));
await box('host.viewer2', panel.locator('dl[aria-label="Viewer 2"]'));
await box('host.terminal', panel.locator('h3:text-is("Terminal") + dl'));
await box('host.hint', panel.locator('#export-hint'));
await box('host.export', panel.getByRole('button', { name: 'Export Diagnostics…' }));
await shot('host-status');
await panel.getByRole('button', { name: 'Export Diagnostics…' }).click();
await page.waitForTimeout(300);
await shot('host-exported');

// A viewer.
await open('/?demo=viewer');
await shot('viewer');
await statusPanel();
await box('viewer.panel', panel);
await box('viewer.verdict', panel.locator('.verdict'));
await box('viewer.connection', panel.locator('h3:text-is("Connection") + dl'));
await box('viewer.previous', panel.locator('dt:text-is("Previous attempt")').locator('..'));
await box('viewer.reconnect', panel.getByRole('button', { name: 'Reconnect' }));
await shot('viewer-status');

// Settings, at the size of its window.
await page.setViewportSize({ width: 860, height: 600 });
await open('/?demo=host&window=settings');
const verbose = page.getByRole('checkbox', { name: 'Diagnostics' });
await box('settings.verbose', verbose.locator('xpath=ancestor::div[contains(@class,"row")]'));
await shot('settings');
await verbose.check();
await shot('settings-verbose');

writeFileSync(`${dir}boxes.json`, JSON.stringify(boxes, null, 2));
await browser.close();
// pnpm runs vite as a child, so stop the whole group.
if (server) process.kill(-server.pid);
console.log(Object.keys(boxes).join(' '));
