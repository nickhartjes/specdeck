// Screenshots for the self-hosting video, plus where the parts to highlight are: the Docker Compose
// docs page (from the built site: run `just docs-build` first), the recorded run in run/ replayed
// in terminal.html, and the app's Settings with a sharing service. Needs no Docker.
// Writes assets/shots/*.png and assets/shots/boxes.json.
import { execFileSync, spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { capture } from '../lib/capture.mjs';

const { page, go, open, box, shot, close } = await capture(new URL('./', import.meta.url));
const run = (step) => readFileSync(new URL(`./run/${step}.txt`, import.meta.url), 'utf8');

// The built docs site, served as it is published, under /specdeck/.
const site = new URL('../../site/', import.meta.url).pathname;
const DOCS = 'http://localhost:4329';
spawn('pnpm', ['exec', 'astro', 'preview', '--port', '4329'], { cwd: site, stdio: 'ignore' });
for (let i = 0; !(await fetch(DOCS).then(() => true, () => false)); i++) {
  if (i === 60) throw new Error('the docs preview did not start on :4329; run `just docs-build` first');
  await new Promise((resolve) => setTimeout(resolve, 500));
}
await open(`${DOCS}/specdeck/self-host/docker-compose/`);
// Puts a section's heading, by its id, just under Starlight's header.
const section = async (id) => {
  const heading = page.locator(`[id="${id}"]`);
  await heading.evaluate((e) => {
    e.scrollIntoView({ block: 'start' });
    globalThis.scrollBy(0, -96);
  });
  await page.waitForTimeout(300);
  return heading;
};
const dns = await section('1-dns');
await box('dns.table', dns.locator('xpath=following::table[1]'));
await box('firewall.list', page.locator('[id="2-firewall"]').locator('xpath=following::ul[1]'));
await shot('docs');
const configure = await section('3-configure');
await box('env.copy', configure.locator('xpath=following::pre[1]'));
await box('env.table', configure.locator('xpath=following::table[1]'));
await shot('docs-env');
// Astro starts the server as a process of its own and exits, so stop whatever listens on the port.
for (const pid of execFileSync('lsof', ['-tiTCP:4329', '-sTCP:LISTEN'], { encoding: 'utf8' }).split('\n').filter(Boolean)) process.kill(Number(pid));

// The recorded run, step by step, as it builds up in one terminal.
await open(new URL('./terminal.html', import.meta.url));
const screen = page.locator('#screen');
let shown = '';
const replay = async (name, ...steps) => {
  shown += steps.map(run).join('');
  await page.evaluate((text) => globalThis.show(text), shown);
  await page.waitForTimeout(200);
  await shot(name);
};
await replay('env', '1-env');
await box('term.env', screen.locator('span').filter({ hasText: /^TURN_SECRET/ }));
await box('term.domains', screen.locator('span').filter({ hasText: /^SPECDECK_DOMAIN/ }));
await replay('up', '2-up');
await box('term.started', screen.locator('span').filter({ hasText: /Started/ }).last());
await replay('ps', '3-ps');
await box('term.ps', screen.locator('span').filter({ hasText: /^(SERVICE|caddy|coturn|server) / }).first());
await box('term.ps.last', screen.locator('span').filter({ hasText: /^server / }));
await replay('check', '4-health', '5-version');
await box('term.health', screen.locator('span').filter({ hasText: /^ok$/ }));
await box('term.version', screen.locator('span').filter({ hasText: /^\{"protocol"/ }));
await replay('logs', '6-logs');
await box('term.log', screen.locator('span').filter({ hasText: /INFO/ }));

// The app's Settings, at the size of its window, with the team's own service entered.
await page.setViewportSize({ width: 860, height: 600 });
await go('/?demo=start&window=settings');
const service = page.getByRole('textbox', { name: /Sharing service/ });
await service.scrollIntoViewIfNeeded();
await service.fill('specdeck.example.com');
await page.waitForTimeout(400);
await box('settings.service', service.locator('xpath=..'));
await shot('settings');

await close();
