// Writes index.html: the HyperFrames composition for the self-hosting video, timed from the
// narration's lengths and pointed at the parts the capture located: the docs page, the recorded
// run in a terminal, and the app's Settings.
import { Film, union } from '../lib/film.mjs';

const film = new Film(new URL('./', import.meta.url));
const { at } = film;
// The docs and the terminal show 1600×900 captures at 1408×792; Settings an 860×600 one at 1118×780.
const docs = film.stage(1408, 792);
const term = film.stage(1408, 792);
const settings = film.stage(1118, 780);
const w = (id, word, opts) => film.word(id, word, opts);

const title = film.title({ kicker: 'SpecDeck', title: 'Run your own sharing service', sub: 'Docker Compose on one server' }, at('dns'));

// The docs page: DNS, the firewall and .env.
const docsStart = at('dns') - 0.3;
film.enter('#docs', docsStart);
film.camera('#docs-camera', at('dns', 0.02), docs.aim(docs.box('dns.table'), 1.8));
film.ring('#docs-ring', w('dns', 'two', { offset: -0.2 }), docs.box('dns.table', 8));
film.camera('#docs-camera', at('firewall'), docs.aim(docs.box('firewall.list'), 1.8));
film.ring('#docs-ring', w('firewall', 'Open', { offset: -0.1 }), docs.box('firewall.list', 8));
film.fade('#docs-docs-env', at('env') - 0.25, 1);
film.camera('#docs-camera', at('env', 0.02), docs.aim(union(docs.box('env.copy'), docs.box('env.table')), 1.25));
film.ring('#docs-ring', w('env', 'Copy', { offset: -0.2 }), docs.box('env.copy', 8));
film.ring('#docs-ring', w('env', 'fill', { offset: -0.2 }), docs.box('env.table', 8));
film.fade('#docs', at('local') - 0.2, 0, 0.5);

// The recorded run.
const termStart = at('local') - 0.1;
film.enter('#term', termStart, { duration: 0.6 });
film.camera('#term-camera', at('local', 0.02), term.aim(term.box('term.env'), 2.6), 0.01);
film.ring('#term-ring', w('local', 'both', { offset: -0.2 }), union(term.box('term.domains', 6), term.box('term.env', 6)));
film.fade('#term-up', at('up') - 0.25, 1);
film.camera('#term-camera', at('up', 0.02), term.aim(term.box('term.started'), 1.6));
film.ring('#term-ring', w('up', 'starts', { offset: -0.2 }), term.box('term.started', 6));
film.fade('#term-ps', at('ps') - 0.25, 1);
film.camera('#term-camera', at('ps', 0.02), term.aim(union(term.box('term.ps'), term.box('term.ps.last')), 2));
film.ring('#term-ring', w('ps', 'running', { offset: -0.3 }), union(term.box('term.ps', 6), term.box('term.ps.last', 6)));
film.fade('#term-check', at('check') - 0.25, 1);
film.camera('#term-camera', at('check', 0.02), term.aim(union(term.box('term.health'), term.box('term.version')), 2));
film.ring('#term-ring', w('check', 'health', { offset: -0.2 }), term.box('term.health', 6));
film.ring('#term-ring', w('check', 'release', { offset: -0.3 }), term.box('term.version', 6));
film.fade('#term-logs', at('logs') - 0.25, 1);
film.camera('#term-camera', at('logs', 0.02), term.aim(term.box('term.log'), 1.15));
film.ring('#term-ring', w('logs', 'line', { offset: -0.2 }), term.box('term.log', 6));
film.fade('#term', at('apps') - 0.2, 0, 0.5);

// The app's Settings, with the team's own service.
film.enter('#settings', at('apps') + 0.1, { duration: 0.7 });
film.ring('#settings-ring', w('apps', 'Sharing', { offset: -0.2 }), settings.box('settings.service', 10));

// Outro.
film.fade('#settings', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Run your own sharing service',
    items: [
      { title: 'DNS and firewall', note: 'Two names, four ports' },
      { title: '.env, then up', note: 'docker compose up -d' },
      { title: 'Connect the apps', note: 'Settings › Sharing service' },
    ],
  },
  at('outro') - 0.2,
  film.scenes.outro.length,
);

film.write({
  body: [
    title,
    film.window({ id: 'docs', title: 'nickhartjes.github.io/specdeck', stage: docs, shots: ['docs', 'docs-env'], start: docsStart, duration: at('local') + 0.3 - docsStart, top: 80 }),
    film.window({ id: 'term', title: 'Terminal · a real run, on a laptop', stage: term, shots: ['env', 'up', 'ps', 'check', 'logs'], start: termStart, duration: at('apps') + 0.3 - termStart, top: 80 }),
    film.window({ id: 'settings', title: 'Settings', stage: settings, shots: ['settings'], start: at('apps') - 0.2, duration: film.scenes.apps.length + film.gap + 0.3, top: 54 }),
    steps,
  ].join('\n\n'),
});
