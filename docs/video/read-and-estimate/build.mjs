// Writes index.html: the HyperFrames composition for the read-and-estimate video, timed from the
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
const cut = (shot, time) => film.fade(`#main-${shot}`, time - 0.25, 1);
// The pointer glides to a control and presses it; `w(scene, word)` is when the voice says the word.
const point = (time, target) => film.point('#main-pointer', time, target);
const press = (time) => film.press('#main-pointer', time);
const w = (id, word, opts) => film.word(id, word, opts);
// Whose screen it is: the host's or a viewer's.
const roles = [];
const role = (text, start, end) => {
  const id = `role-${roles.length}`;
  roles.push(`      <div id="${id}" class="clip role" data-start="${start}" data-duration="${+(end - start).toFixed(3)}" data-track-index="2">${text}</div>`);
  film.tween(`tl.fromTo('#${id}', { opacity: 0, x: -16 }, { opacity: 1, x: 0, duration: 0.4, ease: 'power2.out' }, ${start});`);
};

const title = film.title({ kicker: 'SpecDeck', title: 'Read and estimate', sub: 'Make sure everyone has read it, then size the task together' }, at('ask'));
const windowStart = at('ask') - 0.3;
film.enter('#main', windowStart);

// The host asks to read.
role('The host', windowStart, at('viewer') - 0.2);
camera(at('ask', 0.02), main.aim(union(box('host.ask'), box('host.estimate')), 2));
ring(w('ask', 'Ask', { offset: -0.2 }), box('host.ask', 6));
point(w('ask', 'Ask', { offset: -0.3 }), box('host.ask', 0));
press(w('ask', 'read', { offset: 0.1 }));
cut('ask', at('pick'));
camera(at('pick', 0.02), main.aim(box('ask.dialog'), 1.7));
ring(w('pick', 'shared', { offset: -0.2 }), box('ask.choice', 6));
ring(w('pick', 'limit', { offset: -0.3 }), box('ask.limit', 6));
point(w('pick', 'Ask', { offset: -0.4 }), box('ask.button', 0));
press(w('pick', 'read', { offset: 0.1 }));

// A viewer answers.
role('A viewer', at('viewer') - 0.2, at('host') - 0.2);
cut('viewer-read', at('viewer'));
camera(at('viewer', 0.02), main.aim(box('viewer.request'), 1.25));
ring(w('viewer', 'what', { offset: -0.2 }), box('viewer.request', 4));
ring(w('viewer', 'Open', { offset: -0.2 }), box('viewer.open', 6));
point(w('viewer', 'Open', { offset: -0.3 }), box('viewer.open', 0));
ring(w('viewer', 'ready', { offset: -0.3 }), box('viewer.ready', 6));
point(w('viewer', 'ready', { offset: -0.4 }), box('viewer.ready', 0));
ring(w('viewer', 'question', { offset: -0.3 }), box('viewer.question', 6));
point(w('viewer', 'question', { offset: -0.4 }), box('viewer.question', 0));

// The host sees who has read.
role('The host', at('host') - 0.2, at('outro') - 0.2);
cut('host-read', at('host'));
camera(at('host', 0.02), main.aim(box('row.request'), 1.3));
ring(w('host', 'reading', { offset: -0.3 }), box('row.who', 6));
ring(w('host', 'Go', { offset: -0.2 }), box('row.goon', 6));
point(w('host', 'Go', { offset: -0.3 }), box('row.goon', 0));
press(w('host', 'on', { offset: 0.1 }));

// Estimate: the task, the cards, the votes and the result.
cut('host', at('estimate'));
camera(at('estimate', 0.02), main.aim(union(box('host.ask'), box('host.estimate')), 2));
point(w('estimate', 'Estimate', { nth: 2, offset: -0.4 }), box('host.estimate', 0));
press(w('estimate', 'Estimate', { nth: 2, offset: 0.1 }));
cut('estimate', w('estimate', 'write', { offset: -0.1 }));
camera(w('estimate', 'write', { offset: -0.1 }), main.aim(box('est.dialog'), 1.6));
ring(w('estimate', 'write', { offset: -0.1 }), box('est.task', 6));
ring(w('estimate', 'pick', { offset: -0.2 }), box('est.cards', 6));
cut('vote', at('vote'));
camera(at('vote', 0.02), main.aim(union(box('vote.cards'), box('vote.who')), 1.5));
ring(w('vote', 'card', { offset: -0.3 }), box('vote.cards', 6));
point(w('vote', 'card', { offset: -0.4 }), box('vote.five', 0));
press(w('vote', 'card', { offset: 0.1 }));
ring(w('vote', 'sees', { offset: -0.2 }), box('vote.who', 6));
cut('vote-reveal', w('vote', 'reveal', { offset: -0.6 }));
camera(w('vote', 'reveal', { offset: -0.6 }), main.aim(union(box('vote.who'), box('vote.reveal')), 1.5));
ring(w('vote', 'reveal', { offset: -0.3 }), box('vote.reveal', 6));
point(w('vote', 'reveal', { offset: -0.4 }), box('vote.reveal', 0));
press(w('vote', 'reveal', { offset: 0.2 }));
cut('revealed', at('reveal'));
camera(at('reveal', 0.02), main.aim(box('revealed.votes'), 2));
ring(w('reveal', 'each', { offset: -0.2 }), box('revealed.votes', 6));
cut('revealed-keep', w('reveal', 'spread', { offset: -0.5 }));
camera(w('reveal', 'spread', { offset: -0.5 }), main.aim(union(box('revealed.spread'), box('revealed.again')), 2));
ring(w('reveal', 'spread', { offset: -0.2 }), box('revealed.spread', 6));
ring(w('reveal', 'Keep', { offset: -0.2 }), box('revealed.keep', 6));
point(w('reveal', 'Keep', { offset: -0.3 }), box('revealed.keep', 0));
ring(w('reveal', 'again', { offset: -0.3 }), box('revealed.again', 6));
point(w('reveal', 'again', { offset: -0.4 }), box('revealed.again', 0));
cut('kept', at('kept'));
camera(at('kept', 0.02), main.aim(box('kept.card'), 1.9));
ring(at('kept', 0.08), box('kept.card', 6));
film.hidePointer('#main-pointer', at('kept'));

// Outro.
film.fade('#main', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Read and estimate',
    items: [
      { title: 'Ask to read', note: 'A document, a file or the last changes' },
      { title: 'Estimate', note: 'Hidden votes until you reveal them' },
      { title: 'Keep a value', note: 'or vote again' },
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
      shots: ['host', 'ask', 'viewer-read', 'host-read', 'estimate', 'vote', 'vote-reveal', 'revealed', 'revealed-keep', 'kept'],
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
