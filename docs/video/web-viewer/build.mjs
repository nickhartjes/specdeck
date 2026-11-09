// Writes index.html: the HyperFrames composition for the web-viewer video, timed from the
// narration's lengths and pointed at the parts the capture located on screen.
import { Film } from '../lib/film.mjs';

const film = new Film(new URL('./', import.meta.url));
const { at } = film;
// The window shows a 1600×900 capture at 1408×792.
const main = film.stage(1408, 792);
const box = (name, pad) => main.box(name, pad);
const camera = (time, target, duration) => film.camera('#main-camera', time, target, duration);
const ring = (time, target, duration) => film.ring('#main-ring', time, target, duration);
// Brings in capture `shot` on the same stage; the camera moves on from where it is.
const cut = (shot, time) => film.fade(`#main-${shot}`, time - 0.25, 1);
// The pointer glides to a control and presses it; `w(scene, word)` is when the voice says the word.
const point = (time, target) => film.point('#main-pointer', time, target);
const press = (time) => film.press('#main-pointer', time);
const w = (id, word, opts) => film.word(id, word, opts);

const title = film.title({ kicker: 'SpecDeck', title: 'Watch in a browser', sub: 'No app needed to follow a session' }, at('link'));
const windowStart = at('link') - 0.3;
film.enter('#main', windowStart);

// The join page: the code, then the name.
camera(at('link', 0.02), main.aim(box('join.form'), 1.5));
ring(w('link', 'type', { offset: -0.2 }), box('join.code', 6));
point(w('link', 'invite', { offset: -0.3 }), box('join.code', 0));
cut('join-typed', w('link', 'code', { offset: 0.2 }));
ring(w('private', 'hash', { offset: -0.3 }), box('join.code', 6));
ring(w('name', 'name', { offset: -0.3 }), box('join.name', 6));
point(w('name', 'Watch', { offset: -0.5 }), box('join.watch', 0));
ring(w('name', 'Watch', { offset: -0.2 }), box('join.watch', 6));
press(w('name', 'browser', { offset: 0.1 }));
ring(w('name', 'account', { offset: -0.2 }), box('join.name-hint', 6));

// The session, as in the app.
cut('session', at('session'));
camera(at('session', 0.02), main.whole, 0.8);
ring(w('session', 'files', { offset: -0.2 }), box('session.files', 4));
ring(w('session', 'terminal', { offset: -0.2 }), box('session.middle', 4));
ring(w('session', 'timeline', { nth: 1, offset: -0.2 }), box('session.timeline', 4));

// The layout menu: layouts, appearance and Export diagnostics.
point(w('layout', 'layout', { offset: -0.4 }), box('menu.button', 0));
press(w('layout', 'menu'));
cut('menu', w('layout', 'menu', { offset: 0.25 }));
camera(w('layout', 'menu', { offset: 0.25 }), main.aim(box('menu.layouts'), 1.9));
ring(w('layout', 'Overview', { offset: -0.2 }), box('menu.layouts', 6));
ring(w('layout', 'appearance', { offset: -0.2 }), box('menu.appearance', 6));
ring(w('export', 'Export', { offset: -0.2 }), box('menu.export', 6));
point(w('export', 'Export', { offset: -0.3 }), box('menu.export', 0));

// The keys.
cut('keys', at('keys'));
camera(at('keys', 0.02), main.aim(box('keys.dialog'), 1.6));
ring(w('keys', 'question', { offset: -0.3 }), box('keys.dialog', 6));
film.hidePointer('#main-pointer', at('keys'));

// Outro.
film.fade('#main', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Watch in a browser',
    items: [
      { title: 'Open the link', note: 'or the service, and type the code' },
      { title: 'Enter your name', note: 'It stays in this browser' },
      { title: 'Watch in this browser', note: 'Chrome, Edge, Firefox or Safari' },
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
      title: 'specdeck.example.com',
      stage: main,
      shots: ['join', 'join-typed', 'session', 'menu', 'keys'],
      start: windowStart,
      duration: at('outro') + 0.3 - windowStart,
    }),
    steps,
  ].join('\n\n'),
});
