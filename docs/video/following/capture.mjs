// Screenshots of the UI's demo mode for the following-a-session video, plus where the parts to
// highlight are. Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));
const shotPath = (name) => new URL(`./assets/shots/${name}.png`, import.meta.url).pathname;

// The first-join question.
await go('/?demo=viewer');
const ask = page.locator('.first-join');
await box('ask.dialog', ask);
await box('ask.overview', ask.getByRole('button', { name: /^Overview/ }));
await box('ask.developer', ask.getByRole('button', { name: /^Developer/ }));
await shot('ask');

// Overview: the steps in plain words. The demo viewer joined late, so close the "So far" card and
// unfold the first prompt.
await ask.getByRole('button', { name: /^Overview/ }).click();
await page.waitForTimeout(800);
await page.getByRole('button', { name: 'Close the overview' }).click();
await page.getByText(/^Show the \d+ steps/).click();
await page.waitForTimeout(600);
const step = (text) => page.getByRole('complementary', { name: 'Timeline' }).getByRole('group').filter({ hasText: text });
await box('overview.looked', step(/^Looked at 1 file/));
await box('overview.changed', step(/^Changed 2 files/));
await box('overview.asked', step(/^Asked: What should/));
await box('overview.ran', step(/^Ran the health tests/));
await shot('overview');

// What the agent is doing now: the line at the bottom of the timeline.
const live = page.locator('p.live-line');
await live.scrollIntoViewIfNeeded();
await page.waitForTimeout(400);
await box('now.line', live);
await shot('now');

// The layout menu, and how the timeline shows steps.
await box('view.button', page.getByRole('banner').getByRole('button', { name: 'Overview' }));
await page.getByRole('banner').getByRole('button', { name: 'Overview' }).click();
await page.waitForTimeout(400);
await box('view.menu', page.getByRole('banner').getByRole('group', { name: 'Layout' }).getByRole('group', { name: 'Layout' }));
await box('view.steps', page.getByRole('group', { name: 'Steps' }));
await page.screenshot({ path: shotPath('view') });

// Developer: the terminal, search and Files.
await page.getByRole('radio', { name: /^Developer/ }).click();
await page.keyboard.press('Escape');
await page.waitForTimeout(600);
await box('developer.terminal', page.locator('.terminal-pane'));
await box('developer.statusbar', page.getByRole('contentinfo', { name: 'Session details' }));
await shot('developer');
await page.keyboard.press('Meta+f');
await page.keyboard.type('health');
await page.waitForTimeout(400);
await box('search.box', page.getByRole('search').or(page.locator('.search')).first());
await shot('search');
await page.keyboard.press('Escape');
await page.getByRole('tab', { name: /^Code/ }).click();
await page.waitForTimeout(400);
await box('files.first', page.getByRole('button', { name: /src\/routes\/health\.ts/ }));
await box('files.last', page.getByRole('button', { name: /src\/routes\/index\.ts/ }));
await shot('files');
await page.getByRole('button', { name: /src\/routes\/health\.ts/ }).click();
await page.waitForTimeout(600);
await box('files.detail', page.getByRole('region', { name: 'Open' }).getByRole('tabpanel'));
await shot('files-diff');

await close();
