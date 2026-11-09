// Writes index.html: the HyperFrames composition for the getting-started video, timed from the
// narration's lengths and pointed at the parts the capture located on screen.
import { Film } from '../lib/film.mjs';

const film = new Film(new URL('./', import.meta.url));
const { at } = film;
// The window shows a 1600×900 capture at 1408×792.
const main = film.stage(1408, 792);
const box = (name, pad) => main.box(name, pad);
const camera = (time, target, duration) => film.camera('#main-camera', time, target, duration);
const ring = (time, target, duration) => film.ring('#main-ring', time, target, duration);
const hideRing = (time) => film.hideRing('#main-ring', time);
// Whose screen it is: the host's or a teammate's.
const roles = [];
const role = (text, start, end) => {
  const id = `role-${roles.length}`;
  roles.push(`      <div id="${id}" class="clip role" data-start="${start}" data-duration="${+(end - start).toFixed(3)}" data-track-index="2">${text}</div>`);
  film.tween(`tl.fromTo('#${id}', { opacity: 0, x: -16 }, { opacity: 1, x: 0, duration: 0.4, ease: 'power2.out' }, ${start});`);
};

// The pointer glides to a control and presses it; `w(scene, word)` is when the voice says the word.
const point = (time, target) => film.point('#main-pointer', time, target);
const press = (time) => film.press('#main-pointer', time);
const w = (id, word, opts) => film.word(id, word, opts);

const title = film.title({ kicker: 'SpecDeck', title: 'Getting started', sub: 'Run a coding agent with your team watching along' }, at('agent'));
const windowStart = at('agent') - 0.3;
film.enter('#main', windowStart);

// Hosting: the agent, the folder, the terminal and the invite code.
role('You, the host', windowStart, at('join') - 0.2);
camera(at('agent'), main.aim(box('start.agents'), 1.35));
ring(at('agent', 0.15), box('start.agents'));
point(w('agent', 'Claude', { offset: -0.4 }), box('start.claude', 0));
press(w('agent', 'Claude', { offset: 0.2 }));
camera(at('folder'), main.aim(box('start.folders'), 1.35));
ring(at('folder', 0.1), box('start.folders'));
point(w('folder', 'folder', { offset: -0.3 }), box('start.webshop', 0));
press(w('folder', 'start', { offset: 0.1 }));
hideRing(at('hosting') - 0.3);
camera(at('hosting') - 0.3, main.whole, 0.8);
film.fade('#main-host', at('hosting'), 1, 0.5);
ring(at('hosting', 0.3), box('host.terminal', 4));
camera(at('invite'), main.aim(box('host.invite'), 1.9));
ring(at('invite', 0.05), box('host.invite', 6));
point(w('invite', 'Copy', { offset: -0.4 }), box('host.copy', 0));
press(w('invite', 'Copy', { offset: 0.1 }));

// Joining: the code, then the viewer's screen.
role('A teammate', at('join') - 0.2, at('people') - 0.2);
hideRing(at('join') - 0.4);
camera(at('join') - 0.4, main.whole, 0.6);
film.fade('#main-start-typed', at('join') - 0.2, 1, 0.5);
camera(at('join', 0.15), main.aim(box('typed.join'), 1.5));
ring(at('join', 0.2), box('typed.join'));
point(w('join', 'selects', { offset: -0.3 }), box('typed.button', 0));
press(w('join', 'Join', { nth: 2 }));
hideRing(at('viewer') - 0.3);
camera(at('viewer') - 0.3, main.whole, 0.8);
film.fade('#main-viewer', at('viewer'), 1, 0.5);
ring(at('viewer', 0.15), box('viewer.terminal', 4));
ring(at('viewer', 0.55), box('viewer.side', 4));
camera(at('readonly'), main.aim(box('viewer.readonly'), 2.2));
ring(at('readonly', 0.05), box('viewer.readonly', 6));

// Back to the host: People and End session.
role('You, the host', at('people') - 0.2, at('outro') - 0.2);
hideRing(at('people') - 0.4);
camera(at('people') - 0.4, main.whole, 0.6);
film.fade('#main-host-people', at('people') - 0.2, 1, 0.5);
camera(at('people', 0.05), main.aim(box('people.list'), 1.5));
ring(at('people', 0.1), box('people.list', 4));
ring(at('people', 0.4), box('people.remove', 6));
point(w('people', 'remove', { offset: -0.3 }), box('people.remove', 0));
camera(at('people', 0.62), main.aim(box('people.end'), 1.8));
ring(at('people', 0.65), box('people.end', 6));
point(w('people', 'End', { offset: -0.3 }), box('people.end', 0));

// Outro: the three steps.
film.fade('#main', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Share a session',
    items: [
      { title: 'Host a session', note: 'Pick an agent and a folder' },
      { title: 'Share the invite code', note: 'or copy the join link' },
      { title: 'Teammates join', note: 'in the app, or in a browser' },
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
      shots: ['start', 'host', 'start-typed', 'viewer', 'host-people'],
      start: windowStart,
      duration: at('outro') + 0.3 - windowStart,
      top: 80,
    }),
    ...roles,
    steps,
  ].join('\n\n'),
  css: `
      .role {
        left: 256px;
        top: 22px;
        padding: 6px 16px;
        border-radius: 999px;
        background: rgba(111, 155, 255, 0.18);
        box-shadow: inset 0 0 0 1px rgba(111, 155, 255, 0.45);
        color: var(--ink);
        font-size: 28px;
        font-weight: 600;
        opacity: 0;
      }
`,
});
