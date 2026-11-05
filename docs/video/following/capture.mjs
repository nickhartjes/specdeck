// Screenshots of the UI's demo mode for the following-a-session video, plus where the parts to
// highlight are. Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));

// The first-join question.
await go('/?demo=viewer');
await box('ask.dialog', page.locator('.first-join'));
await box('ask.overview', page.getByRole('button', { name: /^Overview/ }));
await box('ask.developer', page.getByRole('button', { name: /^Developer/ }));
await shot('ask');

// Overview: the steps in plain words.
await page.getByRole('button', { name: /^Overview/ }).click();
await page.waitForTimeout(800);
const step = (text) => page.locator('details.step').filter({ hasText: text });
await box('overview.looked', step('Looked at 1 file'));
await box('overview.changed', step('Changed 2 files'));
await box('overview.asked', step('Asked: What should'));
await box('overview.ran', step('Ran the health tests').first());
await box('overview.now', page.locator('p.live-line'));
await shot('overview');

// The View menu.
await page.getByRole('button', { name: 'View' }).click();
await page.waitForTimeout(400);
await box('view.menu', page.locator('#view-menu-panel'));
await page.screenshot({ path: new URL('./assets/shots/view.png', import.meta.url).pathname });

// Developer: the terminal, search and Files.
await page.getByText('Show terminal').click();
await page.keyboard.press('Escape');
await page.waitForTimeout(600);
await box('developer.terminal', page.locator('.terminal-pane'));
await box('developer.statusbar', page.locator('footer.statusbar'));
await shot('developer');
await page.keyboard.press('Meta+f');
await page.keyboard.type('health');
await page.waitForTimeout(400);
await box('search.box', page.locator('.terminal-pane .search'));
await shot('search');
await page.keyboard.press('Escape');
await page.getByRole('tab', { name: /Files/ }).click();
await page.waitForTimeout(400);
await box('files.first', page.getByRole('button', { name: /src\/routes\/health\.ts/ }));
await box('files.last', page.getByRole('button', { name: /src\/routes\/index\.ts/ }));
await shot('files');
await page.getByRole('button', { name: /src\/routes\/health\.ts/ }).click();
await page.waitForTimeout(600);
await box('files.detail', page.locator('.file-detail'));
await shot('files-diff');

await close();
