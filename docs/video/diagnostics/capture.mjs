// Screenshots of the UI's demo mode for the diagnostics video, plus where the parts to highlight
// are. Writes assets/shots/*.png and assets/shots/boxes.json.
import { capture } from '../lib/capture.mjs';

const { page, go, box, shot, close } = await capture(new URL('./', import.meta.url));

// The demo has no native menu; window.demoMenu stands in for Help › Connection Status.
const statusPanel = async () => {
  await page.evaluate(() => globalThis.demoMenu({ id: 'connection-status' }));
  await page.locator('.connection-status .status-details').waitFor();
  await page.waitForTimeout(1200);
};

// The host.
await go('/?demo=host');
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
await go('/?demo=viewer');
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
await go('/?demo=host&window=settings');
const verbose = page.getByRole('checkbox', { name: 'Diagnostics' });
// Settings is taller than its window: scroll to the row first, as ticking it would.
await verbose.scrollIntoViewIfNeeded();
await page.waitForTimeout(300);
await box('settings.verbose', verbose.locator('xpath=ancestor::div[contains(@class,"row")]'));
await shot('settings');
await verbose.check();
await shot('settings-verbose');

await close();
