// Writes index.html: the HyperFrames composition for the diagnostics video, timed
// from the narration's lengths and pointed at the parts the capture located on screen.
import { readFileSync, writeFileSync } from 'node:fs';

const project = new URL('./', import.meta.url).pathname;
const read = (path) => JSON.parse(readFileSync(project + path, 'utf8'));
const script = read('SCRIPT.json');
const lengths = read('assets/voice/durations.json');
const boxes = read('assets/shots/boxes.json');

// The main window shows a 1600×900 capture at 1408×792; Settings an 860×600 one at 1118×780.
const MAIN = { w: 1408, h: 792 };
const SETTINGS = { w: 1118, h: 780 };
const LEAD = 0.8;
const GAP = 0.7;
const TAIL = 1.6;

// Each scene runs as long as its line, plus a breath.
const scenes = {};
let t = LEAD;
for (const { id, text } of script) {
  scenes[id] = { start: t, length: lengths[id], text };
  t += lengths[id] + GAP;
}
const total = +(t - GAP + TAIL).toFixed(2);
const at = (id, fraction = 0) => +(scenes[id].start + scenes[id].length * fraction).toFixed(3);

const r = (n) => +n.toFixed(1);
const inStage = (stage, name, pad = 8) => {
  const b = boxes[name];
  return { x: r(b.x * stage.w - pad), y: r(b.y * stage.h - pad), w: r(b.w * stage.w + 2 * pad), h: r(b.h * stage.h + 2 * pad) };
};
const union = (a, b) => {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
};
// Where the camera goes to put `box` in the middle at `scale`, without showing past the edges.
const aim = (stage, box, scale) => {
  const clamp = (v, lo) => Math.min(0, Math.max(lo, v));
  const x = clamp(stage.w / 2 - (box.x + box.w / 2) * scale, stage.w - stage.w * scale);
  const y = clamp(stage.h / 2 - (box.y + box.h / 2) * scale, stage.h - stage.h * scale);
  return { x: r(x), y: r(y), scale };
};

const tweens = [];
const tween = (code) => tweens.push(code);
const camera = (time, target, duration = 0.9) =>
  tween(`tl.to('#camera', { x: ${target.x}, y: ${target.y}, scale: ${target.scale}, duration: ${duration}, ease: 'power2.inOut' }, ${time});`);
const ring = (selector, time, box, duration = 0.6) =>
  tween(`tl.to('${selector}', { x: ${box.x}, y: ${box.y}, width: ${box.w}, height: ${box.h}, opacity: 1, duration: ${duration}, ease: 'power2.inOut' }, ${time});`);
const fade = (selector, time, opacity, duration = 0.45) =>
  tween(`tl.to('${selector}', { opacity: ${opacity}, duration: ${duration}, ease: 'power1.inOut' }, ${time});`);
const main = (name, pad) => inStage(MAIN, name, pad);
const whole = { x: 0, y: 0, scale: 1 };

// Intro: the title, then the app.
tween(`tl.fromTo('#title .kicker', { opacity: 0, y: 24 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' }, 0.3);`);
tween(`tl.fromTo('#title h1', { opacity: 0, y: 36 }, { opacity: 1, y: 0, duration: 0.8, ease: 'power3.out' }, 0.45);`);
tween(`tl.fromTo('#title .sub', { opacity: 0, y: 24 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' }, 0.7);`);
tween(`tl.to('#title', { opacity: 0, y: -30, duration: 0.6, ease: 'power2.in' }, ${at('open') - 0.7});`);
tween(`tl.fromTo('#window', { opacity: 0, y: 40, scale: 0.97 }, { opacity: 1, y: 0, scale: 1, duration: 0.8, ease: 'power3.out' }, ${at('open') - 0.3});`);

// Open: Help ▸ Connection Status.
tween(`tl.fromTo('#menubar', { opacity: 0, y: -20 }, { opacity: 1, y: 0, duration: 0.5, ease: 'power2.out' }, ${at('open', 0.05)});`);
tween(`tl.fromTo('#help-menu', { opacity: 0, y: -8 }, { opacity: 1, y: 0, duration: 0.3, ease: 'power2.out' }, ${at('open', 0.22)});`);
tween(`tl.to('#help-title', { backgroundColor: 'rgba(255,255,255,0.22)', duration: 0.2 }, ${at('open', 0.22)});`);
tween(`tl.to('#menu-status', { backgroundColor: '#2f6fe4', color: '#ffffff', duration: 0.25 }, ${at('open', 0.42)});`);
fade('#menubar', at('open', 0.88), 0, 0.3);
fade('#shot-host-status', at('open', 0.9), 1);

// Verdict, then each viewer, then the terminal and window.
camera(at('verdict'), aim(MAIN, main('host.verdict'), 1.55));
ring('#ring', at('verdict', 0.1), main('host.verdict'));
const viewers = union(main('host.viewer1'), main('host.viewer2'));
camera(at('viewers'), aim(MAIN, viewers, 1.5));
ring('#ring', at('viewers', 0.05), main('host.viewer1'));
ring('#ring', at('viewers', 0.55), main('host.viewer2'));
camera(at('terminal'), aim(MAIN, main('host.terminal'), 1.4));
ring('#ring', at('terminal', 0.05), main('host.terminal'));

// Export, its toast, and what the file holds.
camera(at('export'), aim(MAIN, union(main('host.export'), main('host.hint')), 1.6));
ring('#ring', at('export', 0.05), main('host.export', 6));
fade('#shot-host-exported', at('export', 0.62), 1, 0.3);
camera(at('privacy'), aim(MAIN, main('host.hint'), 1.7));
ring('#ring', at('privacy', 0.05), main('host.hint'));

// A viewer's panel and Reconnect.
fade('#ring', at('viewer') - 0.3, 0, 0.3);
camera(at('viewer') - 0.3, whole, 0.8);
fade('#shot-viewer-status', at('viewer'), 1, 0.5);
camera(at('viewer', 0.3), aim(MAIN, main('viewer.connection'), 1.5));
ring('#ring', at('viewer', 0.32), main('viewer.connection'));
ring('#ring', at('viewer', 0.62), main('viewer.previous', 4));
camera(at('reconnect'), aim(MAIN, union(main('viewer.reconnect'), main('viewer.previous')), 1.3));
ring('#ring', at('reconnect', 0.05), main('viewer.reconnect', 6));

// Settings: Verbose logging.
fade('#window', at('verbose') - 0.2, 0, 0.5);
tween(`tl.fromTo('#settings', { opacity: 0, y: 40, scale: 0.97 }, { opacity: 1, y: 0, scale: 1, duration: 0.7, ease: 'power3.out' }, ${at('verbose') + 0.1});`);
ring('#settings-ring', at('verbose', 0.25), inStage(SETTINGS, 'settings.verbose', 10));
fade('#shot-settings-verbose', at('verbose', 0.42), 1, 0.25);

// Outro: the three steps.
fade('#settings', at('outro') - 0.2, 0, 0.5);
tween(`tl.fromTo('#steps li', { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.6, stagger: ${+(scenes.outro.length / 3.2).toFixed(2)}, ease: 'power3.out' }, ${at('outro') + 0.2});`);
tween(`tl.fromTo('#steps h2', { opacity: 0 }, { opacity: 1, duration: 0.6 }, ${at('outro')});`);

// Captions: one per sentence, timed by its share of the line's characters.
const captions = [];
for (const [id, scene] of Object.entries(scenes)) {
  const sentences = scene.text.split(/(?<=[.;])\s+/);
  const chars = sentences.reduce((sum, s) => sum + s.length, 0);
  let start = scene.start;
  sentences.forEach((sentence, index) => {
    const length = (scene.length * sentence.length) / chars;
    const last = index === sentences.length - 1;
    captions.push({ id: `cap-${id}-${index}`, start: +start.toFixed(3), length: +(length + (last ? GAP * 0.8 : 0)).toFixed(3), text: sentence });
    start += length;
  });
}
for (const c of captions) {
  tween(`tl.fromTo('#${c.id}', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' }, ${c.start});`);
}

const escape = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
const shots = ['host', 'host-status', 'host-exported', 'viewer-status'];
const html = `<!doctype html>
<html lang="en" data-resolution="landscape">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=1920, height=1080" />
    <script src="https://cdn.jsdelivr.net/npm/gsap@3.14.2/dist/gsap.min.js"></script>
    <style>
      :root {
        --ground: #0d1526;
        --ground-2: #1b2a4a;
        --ink: #f4f6fb;
        --muted: #a9b4cc;
        --accent: #6f9bff;
        --ring: #ffcf4a;
        --font: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
      }
      * { margin: 0; padding: 0; box-sizing: border-box; }
      html, body { width: 1920px; height: 1080px; overflow: hidden; background: var(--ground); }
      #root {
        position: relative;
        width: 1920px;
        height: 1080px;
        overflow: hidden;
        font-family: var(--font);
        color: var(--ink);
        background: radial-gradient(1400px 900px at 30% 10%, var(--ground-2), var(--ground) 70%);
      }
      .clip { position: absolute; }

      #title { inset: 0; display: grid; place-content: center; text-align: center; gap: 18px; }
      #title .kicker { font-size: 34px; font-weight: 600; color: var(--accent); letter-spacing: 0.02em; }
      #title h1 { font-size: 112px; font-weight: 700; letter-spacing: -0.03em; }
      #title .sub { font-size: 36px; color: var(--muted); }

      .window {
        left: 50%;
        top: 60px;
        border-radius: 14px;
        overflow: hidden;
        background: #f3f4f6;
        box-shadow: 0 40px 120px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255, 255, 255, 0.08);
        transform-origin: 50% 0;
      }
      .titlebar {
        height: 34px;
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 0 14px;
        background: #e6e7ea;
        border-bottom: 1px solid #d3d5da;
        color: #3c4048;
        font-size: 14px;
        font-weight: 600;
      }
      .titlebar i { width: 12px; height: 12px; border-radius: 50%; display: block; }
      .titlebar i:nth-child(1) { background: #ff5f57; }
      .titlebar i:nth-child(2) { background: #febc2e; }
      .titlebar i:nth-child(3) { background: #28c840; }
      .titlebar span { flex: 1; text-align: center; margin-right: 52px; }
      .stage { position: relative; overflow: hidden; }
      #window { width: ${MAIN.w}px; margin-left: -${MAIN.w / 2}px; }
      #window .stage { width: ${MAIN.w}px; height: ${MAIN.h}px; }
      #camera { position: absolute; inset: 0; transform-origin: 0 0; }
      .shot { position: absolute; inset: 0; width: 100%; height: 100%; }
      .shot.later { opacity: 0; }
      .ring {
        position: absolute;
        left: 0;
        top: 0;
        opacity: 0;
        border: 3px solid var(--ring);
        border-radius: 10px;
        box-shadow: 0 0 0 4000px rgba(8, 14, 28, 0.38), 0 0 24px rgba(255, 207, 74, 0.55);
      }
      #settings { width: ${SETTINGS.w}px; margin-left: -${SETTINGS.w / 2}px; top: 54px; opacity: 0; }
      #settings .stage { width: ${SETTINGS.w}px; height: ${SETTINGS.h}px; }

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
        backdrop-filter: blur(20px);
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

      .caption {
        left: 50%;
        bottom: 34px;
        transform: translateX(-50%);
        width: max-content;
        max-width: 1560px;
        padding: 14px 28px;
        border-radius: 14px;
        background: rgba(8, 12, 22, 0.78);
        font-size: 34px;
        line-height: 1.32;
        text-align: center;
        opacity: 0;
        z-index: 10;
      }

      #steps { inset: 0; display: grid; place-content: center; gap: 46px; text-align: center; }
      #steps h2 { font-size: 64px; letter-spacing: -0.02em; }
      #steps ol { list-style: none; display: flex; gap: 34px; justify-content: center; }
      #steps li {
        width: 400px;
        padding: 34px 30px;
        border-radius: 20px;
        background: rgba(255, 255, 255, 0.07);
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
        font-size: 30px;
        line-height: 1.35;
      }
      #steps li b { display: block; color: var(--accent); font-size: 52px; margin-bottom: 12px; }
      #steps li small { display: block; color: var(--muted); font-size: 24px; margin-top: 10px; }
    </style>
  </head>
  <body>
    <div id="root" data-composition-id="main" data-start="0" data-duration="${total}" data-width="1920" data-height="1080">
      <section id="title" class="clip" data-start="0" data-duration="${at('open')}" data-track-index="1">
        <div class="kicker">SpecDeck</div>
        <h1>Diagnostics</h1>
        <div class="sub">Connection status, and a file for your bug report</div>
      </section>

      <section id="window" class="clip window" data-start="${at('open') - 0.3}" data-duration="${+(at('verbose') + 0.3 - (at('open') - 0.3)).toFixed(3)}" data-track-index="1">
        <div class="titlebar"><i></i><i></i><i></i><span>SpecDeck</span></div>
        <div class="stage">
          <div id="camera">
${shots.map((s, i) => `            <img id="shot-${s}" class="shot${i ? ' later' : ''}" src="assets/shots/${s}.png" alt="" />`).join('\n')}
            <div id="ring" class="ring"></div>
          </div>
        </div>
      </section>

      <section id="menubar" class="clip" data-start="${at('open')}" data-duration="${+scenes.open.length.toFixed(3)}" data-track-index="2">
        <b></b><b>SpecDeck</b><span>File</span><span>Edit</span><span>View</span><span>Window</span><span id="help-title">Help</span>
        <div id="help-menu">
          <div>Keyboard Shortcuts<kbd>⌘/</kbd></div>
          <div id="menu-status">Connection Status<kbd>⇧⌘D</kbd></div>
          <div>Export Diagnostics…<kbd></kbd></div>
        </div>
      </section>

      <section id="settings" class="clip window" data-start="${at('verbose') - 0.2}" data-duration="${+(scenes.verbose.length + GAP + 0.3).toFixed(3)}" data-track-index="1">
        <div class="titlebar"><i></i><i></i><i></i><span>Settings</span></div>
        <div class="stage">
          <img id="shot-settings" class="shot" src="assets/shots/settings.png" alt="" />
          <img id="shot-settings-verbose" class="shot later" src="assets/shots/settings-verbose.png" alt="" />
          <div id="settings-ring" class="ring"></div>
        </div>
      </section>

      <section id="steps" class="clip" data-start="${at('outro') - 0.2}" data-duration="${+(total - at('outro') + 0.2).toFixed(3)}" data-track-index="1">
        <h2>Reporting a connection problem</h2>
        <ol>
          <li><b>1</b>Reproduce the problem<small>Verbose logging on, if it is hard to catch</small></li>
          <li><b>2</b>Help ▸ Export Diagnostics…<small>Choose where to save the file</small></li>
          <li><b>3</b>Attach the file<small>to your bug report</small></li>
        </ol>
      </section>

${captions.map((c) => `      <p id="${c.id}" class="clip caption" data-start="${c.start}" data-duration="${c.length}" data-track-index="3">${escape(c.text)}</p>`).join('\n')}

${script.map(({ id }) => `      <audio id="vo-${id}" src="assets/voice/${id}.wav" data-start="${scenes[id].start.toFixed(3)}" data-duration="${lengths[id]}" data-track-index="4"></audio>`).join('\n')}
    </div>
    <script>
      window.__timelines = window.__timelines || {};
      const tl = gsap.timeline({ paused: true });
      ${tweens.join('\n      ')}
      window.__timelines.main = tl;
    </script>
  </body>
</html>
`;
writeFileSync(`${project}index.html`, html);
console.log(`index.html: ${total} s, ${captions.length} captions, ${tweens.length} tweens`);
