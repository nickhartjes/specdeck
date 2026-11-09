// Writes index.html: the HyperFrames composition for the claude-browser video, timed from the
// narration's lengths and pointed at the parts the capture located on screen.
import { Film } from '../lib/film.mjs';

const film = new Film(new URL('./', import.meta.url));
const { at } = film;
// The window shows a 1600×900 capture at 1408×792; Settings an 860×600 one at 1118×780.
const main = film.stage(1408, 792);
const settings = film.stage(1118, 780);
const box = (name, pad) => main.box(name, pad);
const camera = (time, target, duration) => film.camera('#main-camera', time, target, duration);
const ring = (time, target, duration) => film.ring('#main-ring', time, target, duration);
// Brings in capture `shot` on the same stage; the camera moves on from where it is.
const cut = (shot, time) => film.fade(`#main-${shot}`, time - 0.25, 1);
// The pointer glides to a control and presses it; `w(scene, word)` is when the voice says the word.
const point = (time, target) => film.point('#main-pointer', time, target);
const press = (time) => film.press('#main-pointer', time);
const w = (id, word, opts) => film.word(id, word, opts);

const title = film.title({ kicker: 'SpecDeck', title: "Claude's browser", sub: 'A browser of its own, to check the web app it builds' }, at('plain'));
const windowStart = at('plain') - 0.3;
film.enter('#main', windowStart);

// The browser steps in plain words, then every step.
camera(at('plain', 0.02), main.aim(box('turn'), 1.7));
ring(w('plain', 'steps', { offset: -0.2 }), box('steps', 6));
cut('every', at('every'));
camera(at('every', 0.02), main.aim(box('every'), 1.6));
ring(w('every', 'each', { offset: -0.2 }), box('every', 4));

// A screenshot, then full size in a tab.
ring(w('shot', 'screenshots', { offset: -0.2 }), box('every.picture', 6));
point(w('shot', 'Select', { offset: -0.4 }), box('every.picture', 0));
press(w('shot', 'Select', { offset: 0.1 }));
cut('full', w('shot', 'full', { offset: -0.2 }));
camera(w('shot', 'full', { offset: -0.2 }), main.aim(box('full'), 1.25));
ring(w('shot', 'full', { offset: 0 }), box('full', 4));
film.hidePointer('#main-pointer', at('apart'));
camera(at('apart', 0.02), main.whole, 0.8);

// Settings: Give Claude a browser, and Show screenshots in the timeline.
film.fade('#main', at('settings') - 0.2, 0, 0.5);
film.enter('#settings', at('settings') + 0.1, { duration: 0.7 });
film.ring('#settings-ring', w('settings', 'Give', { offset: -0.2 }), settings.box('settings.browser', 10));
film.ring('#settings-ring', w('settings', 'Show', { offset: -0.2 }), settings.box('settings.screenshots', 10));

// Outro.
film.fade('#settings', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: "Claude's browser",
    items: [
      { title: 'Steps in the timeline', note: 'Opened, read, clicked' },
      { title: 'Screenshots', note: 'Full size in a tab' },
      { title: 'Settings', note: 'Give Claude a browser' },
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
      shots: ['plain', 'every', 'full'],
      start: windowStart,
      duration: at('settings') + 0.3 - windowStart,
      top: 80,
    }),
    film.window({
      id: 'settings',
      title: 'Settings',
      stage: settings,
      shots: ['settings'],
      start: at('settings') - 0.2,
      duration: film.scenes.settings.length + film.gap + 0.3,
      top: 54,
    }),
    steps,
  ].join('\n\n'),
});
