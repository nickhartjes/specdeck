// Screenshots of the UI's demo mode for the getting-started video, plus where the parts to
// highlight are. Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));

// The start screen, and hosting from a recent folder.
await go('/?demo=start');
await box('start.agents', page.locator('.agents'));
await box('start.folders', page.locator('ul.recent').locator('xpath=..'));
await box('start.join', page.locator('section.card').nth(1));
await shot('start');
await page.getByRole('button', { name: /webshop/ }).first().click();
await page.waitForTimeout(1200);
await box('host.terminal', page.locator('.terminal-pane'));
await box('host.invite', page.locator('div.invite'));
await shot('host');

// A teammate joins with the code and follows as a developer.
await go('/?demo=start');
await page.locator('#invite-code').fill('7k2m9qxh4tpa');
await box('typed.join', page.locator('section.card').nth(1));
await shot('start-typed');
await page.getByRole('button', { name: 'Join' }).click();
await page.getByRole('button', { name: /^Developer/ }).click();
await page.waitForTimeout(800);
await box('viewer.terminal', page.locator('.terminal-pane'));
await box('viewer.side', page.locator('aside.side'));
await box('viewer.readonly', page.locator('.pill.readonly'));
await shot('viewer');

// Back on the host: People, with Remove showing, and End session.
await go('/?demo=host');
await page.getByRole('tab', { name: /People/ }).click();
await page.waitForTimeout(400);
await box('people.list', page.locator('#panel-people'));
await box('people.end', page.getByRole('button', { name: 'End session' }));
await page.getByText('Anna de Vries').first().hover();
await page.waitForTimeout(300);
await box('people.remove', page.getByRole('button', { name: 'Remove' }).first());
// shot() moves the pointer away, which hides Remove, so this one is taken as is.
await page.screenshot({ path: new URL('./assets/shots/host-people.png', import.meta.url).pathname });

await close();
