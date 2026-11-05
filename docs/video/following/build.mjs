// Writes index.html: the HyperFrames composition for the following-a-session video, timed from
// the narration's lengths and pointed at the parts the capture located on screen.
import { Film, union } from '../lib/film.mjs';

const film = new Film(new URL('./', import.meta.url));
const { at } = film;
// The window shows a 1600×900 capture at 1408×792.
const main = film.stage(1408, 792);
const box = (name, pad) => main.box(name, pad);
const camera = (time, target, duration) => film.camera('#main-camera', time, target, duration);
const ring = (time, target, duration) => film.ring('#main-ring', time, target, duration);
const hideRing = (time) => film.hideRing('#main-ring', time);
// Switches to capture `shot`, from the whole window.
const cut = (shot, time) => {
  hideRing(time - 0.4);
  camera(time - 0.4, main.whole, 0.6);
  film.fade(`#main-${shot}`, time - 0.2, 1, 0.5);
};

const title = film.title({ kicker: 'SpecDeck', title: 'Following a session', sub: "Read along with the host's agent, at your own pace" }, at('ask'));
const windowStart = at('ask') - 0.3;
film.enter('#main', windowStart);

// The first-join question: Overview or Developer.
ring(at('ask', 0.3), box('ask.dialog'));
camera(at('overview'), main.aim(box('ask.dialog'), 1.5));
ring(at('overview', 0.05), box('ask.overview', 4));
ring(at('developer', 0.05), box('ask.developer', 4));

// Overview: steps, a question, a permission and what happens now.
cut('overview', at('steps'));
camera(at('steps', 0.1), main.aim(union(box('overview.looked'), box('overview.changed')), 1.45));
ring(at('steps', 0.2), box('overview.looked', 4));
ring(at('steps', 0.5), box('overview.changed', 4));
ring(at('asked', 0.05), box('overview.asked', 4));
camera(at('allowed'), main.aim(box('overview.ran'), 1.6));
ring(at('allowed', 0.05), box('overview.ran', 4));
camera(at('now'), main.aim(box('overview.now'), 1.6));
ring(at('now', 0.05), box('overview.now', 4));

// View, then the terminal.
cut('view', at('view'));
camera(at('view', 0.1), main.aim(box('view.menu'), 1.9));
ring(at('view', 0.15), box('view.menu', 4));
cut('developer', at('terminal'));
ring(at('terminal', 0.1), box('developer.terminal', 4));

// Search, Files and the status bar.
cut('search', at('search'));
camera(at('search', 0.05), main.aim(box('search.box'), 1.9));
ring(at('search', 0.1), box('search.box', 6));
cut('files', at('files'));
const rows = union(box('files.first', 4), box('files.last', 4));
camera(at('files', 0.05), main.aim(union(rows, box('files.detail')), 1.6));
ring(at('files', 0.1), rows);
film.fade('#main-files-diff', at('files', 0.55), 1, 0.4);
ring(at('files', 0.55), box('files.detail', 4));
camera(at('statusbar'), main.aim(box('developer.statusbar'), 1.25));
ring(at('statusbar', 0.05), box('developer.statusbar', 4));

// Outro.
film.fade('#main', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Follow at your own pace',
    items: [
      { title: 'Overview', note: 'The conversation, in plain words' },
      { title: 'Developer', note: 'The terminal and every tool call' },
      { title: '⌘F to search', note: 'The terminal and the timeline' },
    ],
  },
  at('outro') - 0.2,
  film.scenes.outro.length,
);

film.write({
  body: [
    title,
    film.window({
      id: 'main',
      title: 'SpecDeck',
      stage: main,
      shots: ['ask', 'overview', 'view', 'developer', 'search', 'files', 'files-diff'],
      start: windowStart,
      duration: at('outro') + 0.3 - windowStart,
    }),
    steps,
  ].join('\n\n'),
});
