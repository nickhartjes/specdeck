// Screenshots of the UI's demo mode for the taking-part video, plus where the parts to highlight
// are. Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));
const shotPath = (name) => new URL(`./assets/shots/${name}.png`, import.meta.url).pathname;
const timeline = () => page.getByRole('complementary', { name: 'Timeline' });
const files = () => page.getByRole('complementary', { name: 'Files' });

// A viewer suggests a prompt.
await go('/?demo=viewer');
await page.locator('.first-join').getByRole('button', { name: /^Developer/ }).click();
await page.waitForTimeout(600);
await timeline().getByRole('tab', { name: /^Prompts/ }).click();
await page.getByRole('textbox', { name: 'Prompt to suggest' }).fill('Also report the app version in /health, so we can see which release answers.');
await page.waitForTimeout(300);
await box('suggest.tab', timeline().getByRole('tab', { name: /^Prompts/ }));
await box('suggest.form', page.locator('.suggest-form'));
await box('suggest.button', page.getByRole('button', { name: /^Suggest to / }));
await box('suggest.hint', timeline().locator('.suggest > .field-hint').first());
await shot('suggest');
await page.getByRole('button', { name: /^Suggest to / }).click();
// The demo host answers after 4 seconds of the page's clock.
await page.clock.runFor(4500);
await page.waitForTimeout(400);
await box('suggest.mine', page.getByRole('list', { name: 'Your suggestions' }));
await shot('suggested');

// The host's Prompts tab: suggestions waiting, to send, edit or decline.
await go('/?demo=host');
await timeline().getByRole('tab', { name: /^Prompts/ }).click();
await page.waitForTimeout(400);
const waiting = page.getByRole('list', { name: 'Suggestions waiting' });
await box('inbox.tab', timeline().getByRole('tab', { name: /^Prompts/ }));
await box('inbox.list', waiting);
await box('inbox.actions', waiting.locator('.suggest-actions').first());
await box('inbox.send', waiting.getByRole('button', { name: 'Send' }).first());
await box('inbox.edit', waiting.getByRole('button', { name: 'Edit' }).first());
await box('inbox.keep', waiting.getByRole('button', { name: 'Keep for later' }).first());
await box('inbox.decline', waiting.getByRole('button', { name: 'Decline' }).first());
await shot('inbox');

// The host's links waiting, from a viewer.
await timeline().getByRole('tab', { name: 'Timeline' }).click();
await page.waitForTimeout(400);
// The timeline opens at the latest step; the links wait above the first prompt.
const links = page.getByRole('list', { name: 'Links waiting for you' });
await links.scrollIntoViewIfNeeded();
await page.waitForTimeout(400);
await box('links.waiting', links);
await shot('host-links');

// A viewer sends a link to the host.
await go('/?demo=viewer');
await page.locator('.first-join').getByRole('button', { name: /^Developer/ }).click();
await page.waitForTimeout(600);
await page.getByRole('button', { name: /^Send a link to/ }).click();
const linkDialog = page.getByRole('dialog', { name: /^Send a link to/ });
await linkDialog.getByRole('textbox', { name: 'Address' }).fill('https://pr-412.preview.example.com/health');
await linkDialog.getByRole('textbox', { name: 'Title (optional)' }).fill('Preview of the health endpoint');
await page.waitForTimeout(300);
await box('link.dialog', linkDialog);
await box('link.send', linkDialog.getByRole('button', { name: /^Send to / }));
await page.screenshot({ path: shotPath('link') });

// A viewer adds a note to the braindump.
await page.keyboard.press('Escape');
await files().getByRole('tab', { name: /^Shared/ }).click();
await page.waitForTimeout(300);
await box('shared.tab', files().getByRole('tab', { name: /^Shared/ }));
await box('shared.add', files().getByRole('button', { name: 'Add files or a note' }));
await shot('shared-open');
await files().getByRole('button', { name: 'Add files or a note' }).click();
const add = page.getByRole('dialog', { name: 'Add files or a note' });
await add.getByRole('textbox', { name: /^Or write a note/ }).fill('Edge case: the database answers, but slowly. Should /health time out after 2 seconds?');
await page.waitForTimeout(300);
await box('add.dialog', add);
await box('add.where', add.getByRole('combobox', { name: 'Where it goes' }));
await box('add.note', add.getByRole('textbox', { name: /^Or write a note/ }));
await box('add.button', add.getByRole('button', { name: 'Add', exact: true }));
await page.screenshot({ path: shotPath('add') });
await add.getByRole('button', { name: 'Add' }).click();
await page.waitForTimeout(500);
await box('shared.braindump', files().getByRole('region', { name: /^Braindump for/ }));
await box('shared.everyone', files().getByRole('region', { name: 'Shared with everyone' }));
await shot('shared');

// The host gives the braindump to the agent, with a prompt to edit.
await go('/?demo=host');
await files().getByRole('tab', { name: /^Shared/ }).click();
await page.waitForTimeout(300);
await box('give.button', files().getByRole('button', { name: /^Give \d+ to/ }));
await shot('host-shared');
await files().getByRole('button', { name: /^Give \d+ to/ }).click();
const give = page.getByRole('dialog', { name: /^Give the braindump to/ });
await page.waitForTimeout(400);
await box('give.dialog', give);
await box('give.prompt', give.getByRole('textbox', { name: /^Prompt for/ }));
await box('give.send', give.getByRole('button', { name: /^Give to / }));
await page.screenshot({ path: shotPath('give') });

await close();
