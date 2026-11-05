// What every video's build.mjs shares: scenes timed from the narration, app windows with a camera
// and a highlight ring, title and closing cards, captions, and writing index.html for HyperFrames.
import { readFileSync, writeFileSync } from 'node:fs';

const r = (n) => +n.toFixed(1);
const s3 = (n) => +n.toFixed(3);

/** A box around both `a` and `b`. */
export const union = (a, b) => {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
};

export class Film {
  /** `dir` is the video's folder URL; LEAD, GAP and TAIL are the seconds before, between and after lines. */
  constructor(dir, { lead = 0.8, gap = 0.7, tail = 1.6 } = {}) {
    this.dir = new URL(dir).pathname;
    const read = (path) => JSON.parse(readFileSync(this.dir + path, 'utf8'));
    this.script = read('SCRIPT.json');
    this.lengths = read('assets/voice/durations.json');
    this.boxes = read('assets/shots/boxes.json');
    this.gap = gap;
    this.tweens = [];
    this.shown = new Set();
    this.scenes = {};
    let t = lead;
    for (const { id, text } of this.script) {
      this.scenes[id] = { start: t, length: this.lengths[id], text };
      t += this.lengths[id] + gap;
    }
    this.total = s3(t - gap + tail);
    /** The time `fraction` of the way through scene `id`; bound, so it can be destructured. */
    this.at = (id, fraction = 0) => s3(this.scenes[id].start + this.scenes[id].length * fraction);
  }

  /** A screen area of `width`×`height`, showing captures scaled to it. */
  stage(width, height) {
    const boxes = this.boxes;
    return {
      w: width,
      h: height,
      /** Where capture box `name` is on this stage, with `pad` around it. */
      box(name, pad = 8) {
        const b = boxes[name];
        if (!b) throw new Error(`no box ${name} in boxes.json`);
        return { x: r(b.x * width - pad), y: r(b.y * height - pad), w: r(b.w * width + 2 * pad), h: r(b.h * height + 2 * pad) };
      },
      /** The camera that puts `box` in the middle at `scale`, without showing past the edges. */
      aim(box, scale) {
        const clamp = (v, lo) => Math.min(0, Math.max(lo, v));
        const x = clamp(width / 2 - (box.x + box.w / 2) * scale, width - width * scale);
        const y = clamp(height / 2 - (box.y + box.h / 2) * scale, height - height * scale);
        return { x: r(x), y: r(y), scale };
      },
      whole: { x: 0, y: 0, scale: 1 },
    };
  }

  tween(code) {
    this.tweens.push(code);
  }

  camera(selector, time, target, duration = 0.9) {
    this.tween(`tl.to('${selector}', { x: ${target.x}, y: ${target.y}, scale: ${target.scale}, duration: ${duration}, ease: 'power2.inOut' }, ${time});`);
  }

  /** Moves ring `selector` to `box`; a hidden ring appears there instead of travelling to it. */
  ring(selector, time, box, duration = 0.6) {
    if (this.shown.has(selector)) {
      this.tween(`tl.to('${selector}', { x: ${box.x}, y: ${box.y}, width: ${box.w}, height: ${box.h}, duration: ${duration}, ease: 'power2.inOut' }, ${time});`);
      return;
    }
    this.shown.add(selector);
    this.tween(`tl.set('${selector}', { x: ${box.x}, y: ${box.y}, width: ${box.w}, height: ${box.h} }, ${time});`);
    this.tween(`tl.to('${selector}', { opacity: 1, duration: 0.4, ease: 'power1.out' }, ${time});`);
  }

  hideRing(selector, time) {
    this.shown.delete(selector);
    this.fade(selector, time, 0, 0.3);
  }

  fade(selector, time, opacity, duration = 0.45) {
    this.tween(`tl.to('${selector}', { opacity: ${opacity}, duration: ${duration}, ease: 'power1.inOut' }, ${time});`);
  }

  /** Rises `selector` into view at `time`. */
  enter(selector, time, { y = 40, duration = 0.8 } = {}) {
    this.tween(`tl.fromTo('${selector}', { opacity: 0, y: ${y}, scale: 0.97 }, { opacity: 1, y: 0, scale: 1, duration: ${duration}, ease: 'power3.out' }, ${time});`);
  }

  /** The title card, shown until `until`. */
  title({ kicker, title, sub }, until) {
    this.tween(`tl.fromTo('#title .kicker', { opacity: 0, y: 24 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' }, 0.3);`);
    this.tween(`tl.fromTo('#title h1', { opacity: 0, y: 36 }, { opacity: 1, y: 0, duration: 0.8, ease: 'power3.out' }, 0.45);`);
    this.tween(`tl.fromTo('#title .sub', { opacity: 0, y: 24 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' }, 0.7);`);
    this.tween(`tl.to('#title', { opacity: 0, y: -30, duration: 0.6, ease: 'power2.in' }, ${s3(until - 0.7)});`);
    return `      <section id="title" class="clip" data-start="0" data-duration="${s3(until)}" data-track-index="1">
        <div class="kicker">${kicker}</div>
        <h1>${title}</h1>
        <div class="sub">${sub}</div>
      </section>`;
  }

  /** The closing card from `start` to the end: a heading and numbered steps, one after another over `span` seconds. */
  steps({ heading, items }, start, span) {
    this.tween(`tl.fromTo('#steps h2', { opacity: 0 }, { opacity: 1, duration: 0.6 }, ${s3(start + 0.2)});`);
    this.tween(`tl.fromTo('#steps li', { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.6, stagger: ${s3(span / (items.length + 0.2))}, ease: 'power3.out' }, ${s3(start + 0.4)});`);
    return `      <section id="steps" class="clip" data-start="${s3(start)}" data-duration="${s3(this.total - start)}" data-track-index="1">
        <h2>${heading}</h2>
        <ol>
${items.map((item, i) => `          <li><b>${i + 1}</b>${item.title}<small>${item.note}</small></li>`).join('\n')}
        </ol>
      </section>`;
  }

  /**
   * An app window named `id` from `start` for `duration` seconds, showing the captures `shots`
   * (the first one at once, the others faded in with `fade('#<id>-<shot>', …)`) on `stage`, under a
   * camera `#<id>-camera` with a ring `#<id>-ring`.
   */
  window({ id, title, stage, shots, start, duration, top = 60 }) {
    return `      <section id="${id}" class="clip window" data-start="${s3(start)}" data-duration="${s3(duration)}" data-track-index="1" style="width: ${stage.w}px; margin-left: -${stage.w / 2}px; top: ${top}px">
        <div class="titlebar"><i></i><i></i><i></i><span>${title}</span></div>
        <div class="stage" style="width: ${stage.w}px; height: ${stage.h}px">
          <div id="${id}-camera" class="camera">
${shots.map((shot, i) => `            <img id="${id}-${shot}" class="shot${i ? ' later' : ''}" src="assets/shots/${shot}.png" alt="" />`).join('\n')}
            <div id="${id}-ring" class="ring"></div>
          </div>
        </div>
      </section>`;
  }

  /** Writes index.html: `body` (the visual clips), captions per sentence, the narration and the timeline. */
  write({ body, css = '' }) {
    const captions = [];
    for (const [id, scene] of Object.entries(this.scenes)) {
      const sentences = scene.text.split(/(?<=[.;?])\s+/);
      const chars = sentences.reduce((sum, s) => sum + s.length, 0);
      let start = scene.start;
      sentences.forEach((sentence, index) => {
        const length = (scene.length * sentence.length) / chars;
        const last = index === sentences.length - 1;
        captions.push({ id: `cap-${id}-${index}`, start: s3(start), length: s3(length + (last ? this.gap * 0.8 : 0)), text: sentence });
        start += length;
      });
    }
    for (const c of captions) {
      this.tween(`tl.fromTo('#${c.id}', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' }, ${c.start});`);
    }
    const escape = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
    const html = `<!doctype html>
<html lang="en" data-resolution="landscape">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=1920, height=1080" />
    <script src="https://cdn.jsdelivr.net/npm/gsap@3.14.2/dist/gsap.min.js"></script>
    <style>
${STYLE}${css}
    </style>
  </head>
  <body>
    <div id="root" data-composition-id="main" data-start="0" data-duration="${this.total}" data-width="1920" data-height="1080">
${body}

${captions.map((c) => `      <p id="${c.id}" class="clip caption" data-start="${c.start}" data-duration="${c.length}" data-track-index="3">${escape(c.text)}</p>`).join('\n')}

${this.script.map(({ id }) => `      <audio id="vo-${id}" src="assets/voice/${id}.wav" data-start="${this.scenes[id].start.toFixed(3)}" data-duration="${this.lengths[id]}" data-track-index="4"></audio>`).join('\n')}
    </div>
    <script>
      window.__timelines = window.__timelines || {};
      const tl = gsap.timeline({ paused: true });
      ${this.tweens.join('\n      ')}
      window.__timelines.main = tl;
    </script>
  </body>
</html>
`;
    writeFileSync(`${this.dir}index.html`, html);
    console.log(`index.html: ${this.total} s, ${captions.length} captions, ${this.tweens.length} tweens`);
  }
}

const STYLE = `      :root {
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
        border-radius: 14px;
        overflow: hidden;
        background: #f3f4f6;
        box-shadow: 0 40px 120px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(255, 255, 255, 0.08);
        transform-origin: 50% 0;
        opacity: 0;
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
      .camera { position: absolute; inset: 0; transform-origin: 0 0; }
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
`;
