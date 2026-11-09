// Writes index.html: the HyperFrames composition for the plans video, timed from the narration's
// lengths and pointed at the parts the capture located on screen.
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

const title = film.title({ kicker: 'SpecDeck', title: 'Documents and plans', sub: 'Read what the agent plans, and what changed since you read it' }, at('card'));
const windowStart = at('card') - 0.3;
film.enter('#main', windowStart);

// The plan card at the top of the timeline.
camera(at('card', 0.02), main.aim(box('card'), 2.2));
ring(w('card', 'plan', { offset: -0.2 }), box('card', 4));
ring(w('card', 'button', { offset: -0.2 }), box('card.docs', 6));
ring(w('card', 'tasks', { offset: -0.3 }), box('card.tasks', 6));

// The reader: a tab per document, linked to the plan's other documents.
camera(at('reader', 0.02), main.aim(box('reader'), 1.15));
ring(w('reader', 'links', { offset: -0.2 }), box('reader.links', 6));
point(w('reader', 'design', { offset: -0.5 }), box('reader.design', 0));
press(w('reader', 'design'));
cut('design', w('reader', 'design', { offset: 0.3 }));
camera(at('docs', 0.02), main.aim(box('docs'), 2.4));
ring(w('docs', 'Documents', { offset: -0.3 }), box('docs', 4));

// The agent changes the plan while you read.
cut('updated', at('updated'));
camera(at('updated', 0.02), main.aim(box('card'), 2.2));
ring(w('updated', 'task', { offset: -0.2 }), box('updated.tasks', 6));
ring(w('updated', 'changed', { offset: -0.4 }), box('updated.design', 6));
point(w('updated', 'changed', { offset: -0.4 }), box('updated.design', 0));
press(w('updated', 'changed', { offset: 0.3 }));

// What changed since you last read it.
cut('changed', at('changed'));
camera(at('changed', 0.02), main.aim(union(box('marks'), box('marks.all')), 1.5));
ring(w('changed', 'New', { offset: -0.3 }), union(box('marks', 6), box('marks.all', 6)));
camera(w('changed', 'Highlight', { offset: -0.5 }), main.aim(union(box('highlight'), box('view')), 1.4));
ring(w('changed', 'Highlight', { offset: -0.2 }), box('highlight', 8));
point(w('changed', 'Highlight', { offset: -0.3 }), box('highlight', 0));

// The Markdown source.
ring(w('source', 'Source', { offset: -0.5 }), box('view', 6));
point(w('source', 'Source', { offset: -0.5 }), box('view', 0));
press(w('source', 'Source'));
cut('source', w('source', 'Source', { offset: 0.3 }));
camera(w('source', 'Source', { offset: 0.3 }), main.aim(box('source'), 1.3));
ring(w('source', 'same', { offset: -0.2 }), box('source', 4));
film.hidePointer('#main-pointer', at('outro'));

// Outro.
film.fade('#main', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Read along',
    items: [
      { title: 'The plan card', note: 'At the top of the timeline' },
      { title: 'A tab per document', note: 'Linked to the plan’s others' },
      { title: 'New and Changed', note: 'Since you last read it' },
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
      shots: ['joined', 'design', 'updated', 'changed', 'source'],
      start: windowStart,
      duration: at('outro') + 0.3 - windowStart,
      top: 80,
    }),
    steps,
  ].join('\n\n'),
});
