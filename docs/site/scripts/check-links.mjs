// Checks the built site (dist) after `astro build`: every link to a page or anchor of the site
// resolves, so does every video, poster and image it shows, every page is in the sidebar, and the
// README's links into the site resolve.
// Run with `pnpm check-links` from docs/site; exits non-zero and lists each failure.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { sidebar } from '../sidebar.mjs';

const site = join(dirname(fileURLToPath(import.meta.url)), '..');
const dist = join(site, 'dist');
const content = join(site, 'src/content/docs');
const readme = join(site, '../../README.md');
const base = '/specdeck/';
const published = `https://nickhartjes.github.io${base}`;

if (!existsSync(dist)) {
  console.error('docs/site/dist is missing: run `pnpm build` first');
  process.exit(1);
}

function files(dir, keep) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return files(path, keep);
    return keep(name) ? [path] : [];
  });
}

const ids = new Map();
function idsOf(file) {
  if (!ids.has(file)) {
    const html = readFileSync(file, 'utf8');
    ids.set(file, new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => decodeURIComponent(m[1]))));
  }
  return ids.get(file);
}

// The file in dist that a path under the base serves: a page's index.html, or a file such as a video.
function target(path) {
  const rest = decodeURIComponent(path.slice(base.length));
  const candidates = rest === '' || rest.endsWith('/') ? [join(dist, rest, 'index.html')] : [join(dist, rest), join(dist, rest, 'index.html')];
  return candidates.find((file) => existsSync(file) && statSync(file).isFile());
}

// Why a link fails, or undefined when it resolves. `from` is the page it is on, for links to `#anchor` alone.
function check(href, from) {
  let url;
  try {
    url = new URL(href, `https://nickhartjes.github.io${from ? base + relative(dist, from).replaceAll('\\', '/') : base}`);
  } catch {
    return 'not a valid link';
  }
  if (url.origin !== 'https://nickhartjes.github.io' || !url.pathname.startsWith(base)) return undefined;
  const file = target(url.pathname);
  if (!file) return 'no such page or file';
  const anchor = decodeURIComponent(url.hash.slice(1));
  if (anchor && file.endsWith('.html') && !idsOf(file).has(anchor)) return `no #${anchor} on that page`;
  return undefined;
}

const failures = [];

for (const page of files(dist, (name) => name.endsWith('.html'))) {
  const html = readFileSync(page, 'utf8');
  for (const [, href] of html.matchAll(/<a\s[^>]*?href="([^"]*)"/g)) {
    if (href.startsWith('mailto:')) continue;
    const absolute = href.startsWith(published) ? href.slice('https://nickhartjes.github.io'.length) : href;
    if (/^[a-z]+:/i.test(absolute)) continue;
    const why = check(absolute.replaceAll('&amp;', '&'), page);
    if (why) failures.push(`${relative(site, page)}: ${href}: ${why}`);
  }
  // Videos, their posters and images; data: and external ones are left alone.
  for (const [tag] of html.matchAll(/<(?:video|img|source)\s[^>]*>/g)) {
    for (const [, src] of tag.matchAll(/\s(?:src|poster)="([^"]*)"/g)) {
      if (/^[a-z]+:/i.test(src)) continue;
      const why = check(src.replaceAll('&amp;', '&'), page);
      if (why) failures.push(`${relative(site, page)}: ${src}: ${why}`);
    }
  }
}

const listed = new Set();
(function collect(items) {
  for (const item of items) {
    if (typeof item === 'string') listed.add(item);
    else if (item.slug) listed.add(item.slug);
    else if (item.items) collect(item.items);
  }
})(sidebar);
for (const file of files(content, (name) => /\.mdx?$/.test(name))) {
  const slug = relative(content, file).replaceAll('\\', '/').replace(/\.mdx?$/, '');
  if (slug === 'index' || slug === '404') continue;
  if (!listed.has(slug)) failures.push(`${relative(site, file)}: not in the sidebar (sidebar.mjs)`);
}
for (const slug of listed) {
  if (!target(`${base}${slug}/`)) failures.push(`sidebar.mjs: ${slug}: no such page`);
}

for (const [href] of readFileSync(readme, 'utf8').matchAll(/https:\/\/nickhartjes\.github\.io\/specdeck\/[^\s)"'>\]]*/g)) {
  const why = check(href.slice('https://nickhartjes.github.io'.length));
  if (why) failures.push(`README.md: ${href}: ${why}`);
}

if (failures.length) {
  console.error(`${failures.length} broken link${failures.length === 1 ? '' : 's'}:`);
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}
console.log('Links, anchors, sidebar and README links are fine.');
