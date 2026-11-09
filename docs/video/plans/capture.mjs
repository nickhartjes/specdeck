// Screenshots of the UI's demo mode for the plans video, plus where the parts to highlight are.
// Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));
const timeline = () => page.getByRole('complementary', { name: 'Timeline' });
const files = () => page.getByRole('complementary', { name: 'Files' });
const card = () => timeline().getByRole('region', { name: /^Plan:/ });
const links = () => page.getByRole('navigation', { name: /^Plan:/ });
// The newest plan's first document is in front when you join in Overview.
const toTop = async () => {
  await card().scrollIntoViewIfNeeded();
  await page.waitForTimeout(300);
};

// A viewer joins while the agent plans an OpenSpec change.
await go('/?demo=viewer&scene=plans');
await page.locator('.first-join').getByRole('button', { name: /^Overview/ }).click();
await page.waitForTimeout(800);
await toTop();
await box('card', card());
await box('card.docs', card().locator('.plan-docs'));
await box('card.tasks', card().locator('.plan-tasks'));
await box('card.design', card().getByRole('button', { name: /^Design/ }));
await box('reader', page.getByRole('region', { name: 'Open' }));
await box('reader.links', links());
await box('reader.design', links().getByRole('button', { name: 'Design' }));
await box('docs', files().getByRole('list', { name: 'Documents' }));
await shot('joined');

// Reading the design, then the proposal, while the agent changes the design and does two tasks.
await links().getByRole('button', { name: 'Design' }).click();
await page.waitForTimeout(400);
await shot('design');
await links().getByRole('button', { name: 'Proposal' }).click();
await page.waitForTimeout(300);
// The demo agent updates the plan 5 seconds into the page's clock.
await page.clock.runFor(6000);
await page.waitForTimeout(800);
await toTop();
await box('updated.tasks', card().locator('.plan-tasks'));
await box('updated.design', card().getByRole('button', { name: /^Design/ }));
await box('updated.docs', files().getByRole('list', { name: 'Documents' }));
await shot('updated');

// The design again: what changed since you read it.
await links().getByRole('button', { name: 'Design' }).click();
await page.waitForTimeout(500);
const article = page.getByRole('article', { name: 'design.md' });
await box('marks', article.locator('.mark-changed, .mark-new').first());
await box('marks.all', article.locator('.doc-block.mark-changed, .doc-block.mark-new').last());
await box('highlight', page.getByRole('checkbox', { name: 'Highlight changes since you last read it' }));
await box('view', page.getByRole('group', { name: 'How to show the document' }));
await shot('changed');

// The same marks in the Markdown source, line by line.
await page.getByRole('group', { name: 'How to show the document' }).getByRole('button', { name: 'Source' }).click();
await page.waitForTimeout(500);
await box('source', page.getByRole('article', { name: 'design.md' }));
await shot('source');

await close();
