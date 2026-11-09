// Screenshots of the UI's demo mode for the read-and-estimate video, plus where the parts to
// highlight are. Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));
const shotPath = (name) => new URL(`./assets/shots/${name}.png`, import.meta.url).pathname;
const toolbar = () => page.getByRole('banner');

// The host asks everyone to read a shared file, with a time limit.
await go('/?demo=host');
await box('host.ask', toolbar().getByRole('button', { name: 'Ask to read' }));
await box('host.estimate', toolbar().getByRole('button', { name: 'Estimate' }));
await shot('host');
await toolbar().getByRole('button', { name: 'Ask to read' }).click();
const ask = page.getByRole('dialog', { name: 'Ask everyone to read' });
await ask.getByRole('radio', { name: 'acceptance-criteria.md' }).check();
await ask.getByRole('combobox', { name: 'Time limit' }).selectOption('5 minutes');
await page.waitForTimeout(300);
await box('ask.dialog', ask);
await box('ask.choice', ask.getByRole('group', { name: 'Shared' }));
await box('ask.limit', ask.getByRole('combobox', { name: 'Time limit' }));
await box('ask.button', ask.getByRole('button', { name: 'Ask to read' }));
await page.screenshot({ path: shotPath('ask') });
await ask.getByRole('button', { name: 'Ask to read' }).click();
// The demo viewers answer over the next seconds of the page's clock.
await page.clock.runFor(8000);
await page.waitForTimeout(400);
const row = page.getByRole('region', { name: /^You asked everyone to read/ });
await box('row.request', row);
await box('row.who', row.getByRole('list', { name: 'Who read' }));
await box('row.timer', row.getByRole('timer'));
await box('row.goon', row.getByRole('button', { name: 'Go on' }));
await shot('host-read');

// A viewer: what to read, the time left, and three answers.
await go('/?demo=viewer&ask=read');
await page.locator('.first-join').getByRole('button', { name: /^Developer/ }).click();
await page.waitForTimeout(600);
const asked = page.getByRole('region', { name: /asks you to read/ });
await box('viewer.request', asked);
await box('viewer.open', asked.getByRole('button', { name: 'Open it' }));
await box('viewer.ready', asked.getByRole('button', { name: /ready$/ }));
await box('viewer.question', asked.getByRole('button', { name: 'I have a question' }));
await shot('viewer-read');

// The host starts a vote, votes, and reveals.
await go('/?demo=host');
await toolbar().getByRole('button', { name: 'Estimate' }).click();
const est = page.getByRole('dialog', { name: 'Estimate a task' });
await est.getByRole('textbox', { name: 'Task' }).fill('Show unreachable after three missed heartbeats');
await page.waitForTimeout(300);
await box('est.dialog', est);
await box('est.task', est.getByRole('textbox', { name: 'Task' }));
await box('est.cards', est.getByRole('group', { name: 'Cards' }));
await box('est.start', est.getByRole('button', { name: 'Start the vote' }));
await page.screenshot({ path: shotPath('estimate') });
await est.getByRole('button', { name: 'Start the vote' }).click();
await page.waitForTimeout(400);
const vote = page.getByRole('region', { name: 'How big is it?' });
await vote.getByRole('group', { name: 'Your vote' }).getByRole('button', { name: '5', exact: true }).click();
await page.clock.runFor(6000);
await page.waitForTimeout(400);
await box('vote.cards', vote.getByRole('group', { name: 'Your vote' }));
await box('vote.five', vote.getByRole('button', { name: '5', exact: true }));
await box('vote.who', vote.getByRole('list', { name: 'Who voted' }));
await box('vote.reveal', vote.getByRole('button', { name: 'Reveal the votes' }));
await shot('vote');
await vote.getByRole('button', { name: 'Reveal the votes' }).click();
await page.waitForTimeout(500);
// Revealed, the row is called Votes.
const votes = page.getByRole('region', { name: 'Votes' });
await box('revealed.votes', votes.getByRole('list', { name: 'Votes' }));
await box('revealed.spread', votes.getByText(/^From |^Everyone agrees/));
await box('revealed.keep', votes.getByRole('button', { name: /^Keep/ }));
await box('revealed.again', votes.getByRole('button', { name: 'Vote again' }));
await shot('revealed');
await votes.getByRole('button', { name: /^Keep/ }).click();
await page.waitForTimeout(600);
const card = page.getByRole('complementary', { name: 'Timeline' }).getByText(/Show unreachable after three missed heartbeats/).last();
await card.scrollIntoViewIfNeeded();
await page.waitForTimeout(300);
await box('kept.card', card.locator('xpath=ancestor::*[self::article or self::li or contains(@class,"step")][1]'));
await shot('kept');

await close();
