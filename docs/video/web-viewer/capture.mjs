// Screenshots of the web viewer's page on the demo session (web-demo.html, development only) for
// the web-viewer video, plus where the parts to highlight are. Writes assets/shots/*.png and
// assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url), { server: 'web' });
const shotPath = (name) => new URL(`./assets/shots/${name}.png`, import.meta.url).pathname;

// The join page: the code, the name, and Watch in this browser.
await go('/web-demo.html');
const form = page.getByRole('form', { name: 'Watch a SpecDeck session' });
await box('join.form', form);
await box('join.code', page.getByRole('textbox', { name: 'Invite code' }));
await shot('join');
await page.getByRole('textbox', { name: 'Invite code' }).fill('7k2m9qxh4tpa');
await page.waitForTimeout(300);
await box('join.name', page.getByRole('textbox', { name: 'Your name' }));
await box('join.name-hint', page.getByText(/^Shown to everyone in the session/));
await box('join.watch', page.getByRole('button', { name: 'Watch in this browser' }));
await shot('join-typed');

// The session, as in the app.
await page.getByRole('button', { name: 'Watch in this browser' }).click();
await page.waitForTimeout(1500);
await page.locator('.first-join').getByRole('button', { name: /^Developer/ }).click();
await page.waitForTimeout(800);
await box('session.files', page.getByRole('complementary', { name: 'Files' }));
await box('session.middle', page.getByRole('region', { name: 'Open' }));
await box('session.timeline', page.getByRole('complementary', { name: 'Timeline' }));
await shot('session');

// The layout menu: layouts, the appearance, and Export diagnostics.
const layout = page.getByRole('banner').getByRole('button', { name: 'Developer' });
await box('menu.button', layout);
await layout.click();
await page.waitForTimeout(400);
const menu = page.getByRole('banner').getByRole('group', { name: 'Layout' }).getByRole('group', { name: 'Layout' });
await box('menu.layouts', menu);
await box('menu.appearance', page.getByRole('group', { name: 'Appearance' }));
await box('menu.export', page.getByRole('button', { name: 'Export diagnostics…' }));
await page.screenshot({ path: shotPath('menu') });

// The keys: ? shows them all.
await page.keyboard.press('Escape');
await page.waitForTimeout(300);
await page.keyboard.press('Shift+Slash');
await page.waitForTimeout(500);
await box('keys.dialog', page.getByRole('dialog').first());
await page.screenshot({ path: shotPath('keys') });

await close();
