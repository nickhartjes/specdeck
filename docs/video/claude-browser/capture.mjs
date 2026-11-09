// Screenshots of the UI's demo mode for the claude-browser video, plus where the parts to
// highlight are. Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));
const timeline = () => page.getByRole('complementary', { name: 'Timeline' });
const turn = () => timeline().getByRole('article', { name: 'Prompt 3' });

// The host's timeline after Claude checked a page in its browser: in plain words.
await go('/?demo=host&scene=browser');
await timeline().getByRole('button', { name: 'Plain words' }).click();
await page.waitForTimeout(300);
await turn().scrollIntoViewIfNeeded();
await page.waitForTimeout(300);
await box('turn', turn());
await box('steps', turn().getByRole('group').first());
await box('picture', turn().getByRole('figure'));
await shot('plain');

// Every step: each browser call on its own.
await timeline().getByRole('button', { name: 'Every step' }).click();
await page.waitForTimeout(400);
await turn().getByRole('figure').scrollIntoViewIfNeeded();
await page.waitForTimeout(300);
await box('every', turn());
await box('every.picture', turn().getByRole('figure'));
await shot('every');

// The screenshot full size, in a tab.
await turn().getByRole('button', { name: /^Screenshot of/ }).click();
await page.waitForTimeout(600);
await box('full', page.getByRole('region', { name: 'Open' }));
await shot('full');

// Settings, at the size of its window: Give Claude a browser, and Show screenshots in the timeline.
await page.setViewportSize({ width: 860, height: 600 });
await go('/?demo=host&window=settings');
const give = page.getByRole('checkbox', { name: 'Browser' });
const pictures = page.getByRole('checkbox', { name: 'Screenshots' });
// Settings is taller than its window: scroll to the rows first.
await pictures.scrollIntoViewIfNeeded();
await page.waitForTimeout(300);
await box('settings.browser', give.locator('xpath=ancestor::div[contains(@class,"row")]'));
await box('settings.screenshots', pictures.locator('xpath=ancestor::div[contains(@class,"row")]'));
await shot('settings');

await close();
