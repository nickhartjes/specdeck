// Writes index.html: the HyperFrames composition for the taking-part video, timed from the
// narration's lengths and pointed at the parts the capture located on screen.
import { Film, union } from '../lib/film.mjs';

const film = new Film(new URL('./', import.meta.url));
const { at } = film;
// The window shows a 1600×900 capture at 1408×792.
const main = film.stage(1408, 792);
const box = (name, pad) => main.box(name, pad);
const camera = (time, target, duration) => film.camera('#main-camera', time, target, duration);
const ring = (time, target, duration) => film.ring('#main-ring', time, target, duration);
// Brings in capture `shot` on the same stage; the camera moves on from where it is.
const cut = (shot, time) => film.swap(`#main-${shot}`, time - 0.25);
// The pointer glides to a control, and presses it.
const point = (time, target) => film.point('#main-pointer', time, target);
const press = (time) => film.press('#main-pointer', time);
// Whose screen it is: a viewer's or the host's.
const roles = [];
const role = (text, start, end) => {
  const id = `role-${roles.length}`;
  roles.push(`      <div id="${id}" class="clip role" data-start="${start}" data-duration="${+(end - start).toFixed(3)}" data-track-index="2">${text}</div>`);
  film.tween(`tl.fromTo('#${id}', { opacity: 0, x: -16 }, { opacity: 1, x: 0, duration: 0.4, ease: 'power2.out' }, ${start});`);
};

const title = film.title({ kicker: 'SpecDeck', title: 'Taking part', sub: 'Suggest prompts, share links and files, add to the braindump' }, at('prompts'));
const windowStart = at('prompts') - 0.3;
film.enter('#main', windowStart);

// Cues are on the narration's words: `w(scene, word)` is when the voice says it.
const w = (id, word, opts) => film.word(id, word, opts);

// A viewer suggests a prompt, and sees what became of it.
role('A viewer', windowStart, at('inbox') - 0.2);
ring(w('prompts', 'Prompts', { offset: -0.3 }), box('suggest.tab', 4));
point(w('prompts', 'Prompts', { offset: -0.3 }), box('suggest.tab', 0));
press(w('prompts', 'Prompts', { offset: 0.15 }));
camera(w('prompts', 'write', { offset: -0.5 }), main.aim(union(box('suggest.hint'), box('suggest.form')), 1.9));
ring(w('prompts', 'write'), box('suggest.form', 6));
point(w('prompts', 'Suggest', { nth: 2, offset: -0.5 }), box('suggest.button', 0));
press(w('prompts', 'Suggest', { nth: 2 }));
ring(w('only', 'Only', { offset: -0.2 }), box('suggest.hint', 6));
cut('suggested', at('mine'));
camera(at('mine', 0.02), main.aim(box('suggest.mine'), 1.9));
ring(w('mine', 'Your', { offset: -0.1 }), box('suggest.mine', 6));

// The host's Prompts tab.
role('The host', at('inbox') - 0.2, at('link') - 0.2);
cut('inbox', at('inbox'));
camera(at('inbox', 0.02), main.aim(box('inbox.list'), 1.7));
ring(w('inbox', 'Prompts', { offset: -0.3 }), box('inbox.list', 6));
camera(at('actions'), main.aim(box('inbox.actions'), 2.2));
ring(w('actions', 'Send', { offset: -0.2 }), box('inbox.send', 6));
point(w('actions', 'Send', { offset: -0.3 }), box('inbox.send', 0));
ring(w('actions', 'Edit', { offset: -0.2 }), box('inbox.edit', 6));
point(w('actions', 'Edit', { offset: -0.3 }), box('inbox.edit', 0));
ring(w('actions', 'Decline', { offset: -0.2 }), box('inbox.decline', 6));
point(w('actions', 'Decline', { offset: -0.3 }), box('inbox.decline', 0));

// A link: sent by a viewer, shared by the host.
role('A viewer', at('link') - 0.2, at('hostlinks') - 0.2);
cut('link', at('link'));
camera(at('link', 0.02), main.aim(box('link.dialog'), 1.5));
ring(at('link', 0.08), box('link.dialog', 6));
point(w('link', 'Send', { offset: -0.5 }), box('link.send', 0));
press(w('link', 'Send', { offset: 0.1 }));
role('The host', at('hostlinks') - 0.2, at('add') - 0.2);
cut('host-links', at('hostlinks'));
camera(at('hostlinks', 0.02), main.aim(box('links.waiting'), 2));
ring(w('hostlinks', 'shares', { offset: -0.3 }), box('links.waiting', 6));

// A viewer adds a note to the braindump.
role('A viewer', at('add') - 0.2, at('give') - 0.2);
cut('shared-open', at('add'));
camera(at('add', 0.02), main.whole, 0.6);
ring(w('add', 'Shared', { offset: -0.3 }), box('shared.tab', 4));
point(w('add', 'Shared', { offset: -0.3 }), box('shared.tab', 0));
press(w('add', 'Shared', { offset: 0.1 }));
point(w('add', 'Add', { offset: -0.4 }), box('shared.add', 0));
press(w('add', 'Add'));
cut('add', w('add', 'Add', { offset: 0.35 }));
camera(w('add', 'Add', { offset: 0.35 }), main.aim(box('add.dialog'), 1.5));
ring(w('add', 'note', { offset: -0.2 }), box('add.note', 6));
ring(w('add', 'Choose', { offset: -0.2 }), box('add.where', 6));
point(w('add', 'everyone', { offset: -0.3 }), box('add.button', 0));
press(w('add', 'everyone', { offset: 0.3 }));
cut('shared', at('shared'));
camera(at('shared', 0.02), main.aim(union(box('shared.braindump'), box('shared.everyone')), 1.25));
ring(w('shared', 'braindump', { offset: -0.2 }), box('shared.braindump', 4));
ring(w('shared', 'What', { offset: -0.2 }), box('shared.everyone', 4));

// The host gives the braindump to the agent.
role('The host', at('give') - 0.2, at('outro') - 0.2);
cut('host-shared', at('give'));
camera(at('give', 0.02), main.aim(box('give.button'), 2));
ring(at('give', 0.06), box('give.button', 6));
point(w('give', 'selects', { offset: -0.3 }), box('give.button', 0));
press(w('give', 'Give'));
cut('give', w('give', 'Give', { offset: 0.3 }));
camera(w('give', 'Give', { offset: 0.3 }), main.aim(box('give.dialog'), 1.6));
ring(w('give', 'edits', { offset: -0.2 }), box('give.prompt', 6));
point(w('give', 'gives', { offset: -0.4 }), box('give.send', 0));
press(w('give', 'gives', { offset: 0.1 }));

// Outro: the three ways to take part.
film.fade('#main', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Take part',
    items: [
      { title: 'Suggest a prompt', note: 'In Prompts, next to the timeline' },
      { title: 'Send a link', note: 'Above the timeline' },
      { title: 'Add files or a note', note: 'In Files › Shared' },
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
      shots: ['suggest', 'suggested', 'inbox', 'link', 'host-links', 'shared-open', 'add', 'shared', 'host-shared', 'give'],
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
