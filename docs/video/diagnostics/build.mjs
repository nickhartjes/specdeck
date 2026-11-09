// Writes index.html: the HyperFrames composition for the diagnostics video, timed from the
// narration's lengths and pointed at the parts the capture located on screen.
import { Film, union } from '../lib/film.mjs';

const film = new Film(new URL('./', import.meta.url));
const { at } = film;
// The main window shows a 1600×900 capture at 1408×792; Settings an 860×600 one at 1118×780.
const main = film.stage(1408, 792);
const settings = film.stage(1118, 780);
const box = (name, pad) => main.box(name, pad);
const camera = (time, target, duration) => film.camera('#main-camera', time, target, duration);
const ring = (time, target, duration) => film.ring('#main-ring', time, target, duration);

// The pointer glides to a control and presses it; `w(scene, word)` is when the voice says the word.
const point = (time, target) => film.point('#main-pointer', time, target);
const press = (time) => film.press('#main-pointer', time);
const w = (id, word, opts) => film.word(id, word, opts);

const title = film.title({ kicker: 'SpecDeck', title: 'Diagnostics', sub: 'Connection status, and a file for your bug report' }, at('open'));
film.enter('#main', at('open') - 0.3);

// Open: Help ▸ Connection Status.
film.tween(`tl.fromTo('#menubar', { opacity: 0, y: -20 }, { opacity: 1, y: 0, duration: 0.5, ease: 'power2.out' }, ${at('open', 0.05)});`);
film.tween(`tl.fromTo('#help-menu', { opacity: 0, y: -8 }, { opacity: 1, y: 0, duration: 0.3, ease: 'power2.out' }, ${at('open', 0.22)});`);
film.tween(`tl.to('#help-title', { backgroundColor: 'rgba(255,255,255,0.22)', duration: 0.2 }, ${at('open', 0.22)});`);
film.tween(`tl.to('#menu-status', { backgroundColor: '#2f6fe4', color: '#ffffff', duration: 0.25 }, ${at('open', 0.42)});`);
film.fade('#menubar', at('open', 0.88), 0, 0.3);
film.fade('#main-host-status', at('open', 0.9), 1);

// Verdict, then each viewer, then the terminal and window.
camera(at('verdict'), main.aim(box('host.verdict'), 1.55));
ring(at('verdict', 0.1), box('host.verdict'));
camera(at('viewers'), main.aim(union(box('host.viewer1'), box('host.viewer2')), 1.5));
ring(at('viewers', 0.05), box('host.viewer1'));
ring(at('viewers', 0.55), box('host.viewer2'));
camera(at('terminal'), main.aim(box('host.terminal'), 1.4));
ring(at('terminal', 0.05), box('host.terminal'));

// Export, its toast, and what the file holds.
camera(at('export'), main.aim(union(box('host.export'), box('host.hint')), 1.6));
ring(at('export', 0.05), box('host.export', 6));
point(w('export', 'Export', { offset: -0.4 }), box('host.export', 0));
press(w('export', 'Export', { offset: 0.2 }));
film.fade('#main-host-exported', at('export', 0.62), 1, 0.3);
camera(at('privacy'), main.aim(box('host.hint'), 1.7));
ring(at('privacy', 0.05), box('host.hint'));

// A viewer's panel and Reconnect.
film.hideRing('#main-ring', at('viewer') - 0.3);
camera(at('viewer') - 0.3, main.whole, 0.8);
film.fade('#main-viewer-status', at('viewer'), 1, 0.5);
camera(at('viewer', 0.3), main.aim(box('viewer.connection'), 1.5));
ring(at('viewer', 0.32), box('viewer.connection'));
ring(at('viewer', 0.62), box('viewer.previous', 4));
camera(at('reconnect'), main.aim(union(box('viewer.reconnect'), box('viewer.previous')), 1.3));
ring(at('reconnect', 0.05), box('viewer.reconnect', 6));
point(w('reconnect', 'Reconnect', { offset: -0.4 }), box('viewer.reconnect', 0));
press(w('reconnect', 'Reconnect', { offset: 0.2 }));

// Settings: Verbose logging.
film.fade('#main', at('verbose') - 0.2, 0, 0.5);
film.enter('#settings', at('verbose') + 0.1, { duration: 0.7 });
film.ring('#settings-ring', at('verbose', 0.25), settings.box('settings.verbose', 10));
film.fade('#settings-settings-verbose', at('verbose', 0.42), 1, 0.25);

// Outro: the three steps.
film.fade('#settings', at('outro') - 0.2, 0, 0.5);
const steps = film.steps(
  {
    heading: 'Reporting a connection problem',
    items: [
      { title: 'Reproduce the problem', note: 'Verbose logging on, if it is hard to catch' },
      { title: 'Help ▸ Export Diagnostics…', note: 'Choose where to save the file' },
      { title: 'Attach the file', note: 'to your bug report' },
    ],
  },
  at('outro') - 0.2,
  film.scenes.outro.length,
);

const windowStart = at('open') - 0.3;
film.write({
  body: [
    title,
    film.window({
      id: 'main',
      title: 'SpecDeck',
      stage: main,
      shots: ['host', 'host-status', 'host-exported', 'viewer-status'],
      start: windowStart,
      duration: at('verbose') + 0.3 - windowStart,
    }),
    `      <section id="menubar" class="clip" data-start="${at('open')}" data-duration="${film.scenes.open.length.toFixed(3)}" data-track-index="2">
        <b></b><b>SpecDeck</b><span>File</span><span>Edit</span><span>View</span><span>Window</span><span id="help-title">Help</span>
        <div id="help-menu">
          <div>Keyboard Shortcuts<kbd>⌘/</kbd></div>
          <div id="menu-status">Connection Status<kbd>⇧⌘D</kbd></div>
          <div>Export Diagnostics…<kbd></kbd></div>
        </div>
      </section>`,
    film.window({
      id: 'settings',
      title: 'Settings',
      stage: settings,
      shots: ['settings', 'settings-verbose'],
      start: at('verbose') - 0.2,
      duration: film.scenes.verbose.length + film.gap + 0.3,
      top: 54,
    }),
    steps,
  ].join('\n\n'),
  css: `
      #menubar {
        left: 0;
        top: 0;
        width: 1920px;
        height: 34px;
        display: flex;
        align-items: center;
        gap: 26px;
        padding: 0 22px;
        background: rgba(30, 34, 44, 0.82);
        font-size: 16px;
        color: #f2f3f5;
        opacity: 0;
        z-index: 5;
      }
      #menubar b { font-weight: 700; }
      #menubar span { padding: 3px 9px; border-radius: 5px; }
      #help-menu {
        position: absolute;
        top: 34px;
        left: 486px;
        width: 330px;
        padding: 6px;
        border-radius: 10px;
        background: rgba(246, 246, 248, 0.97);
        color: #1d1f24;
        box-shadow: 0 18px 50px rgba(0, 0, 0, 0.35), 0 0 0 1px rgba(0, 0, 0, 0.1);
        opacity: 0;
      }
      #help-menu div { display: flex; justify-content: space-between; padding: 6px 12px; border-radius: 6px; font-size: 16px; }
      #help-menu kbd { font-family: var(--font); color: inherit; opacity: 0.6; }
`,
});
