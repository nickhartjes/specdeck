// What every video's build.mjs shares: scenes timed from the narration, app windows with a camera
// and a highlight ring, title and closing cards, captions, and writing index.html for HyperFrames.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';

const r = (n) => +n.toFixed(1);
const s3 = (n) => +n.toFixed(3);

/** A box around both `a` and `b`. */
export const union = (a, b) => {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
};

export class Film {
  /**
   * `dir` is the video's folder URL; LEAD, GAP and TAIL are the seconds before, between and after lines.
   * `look` is `classic` (a ring that dims around its target, captions per sentence) or `smooth`
   * (a feathered spotlight, a pointer, motion blur, and captions that follow the spoken words).
   */
  constructor(dir, { lead = 0.8, gap = 0.7, tail = 1.6, look = 'smooth' } = {}) {
    this.dir = new URL(dir).pathname;
    const read = (path) => JSON.parse(readFileSync(this.dir + path, 'utf8'));
    this.look = look;
    this.shots = new Set();
    this.script = read('SCRIPT.json');
    this.lengths = read('assets/voice/durations.json');
    this.boxes = read('assets/shots/boxes.json');
    this.words = look === 'smooth' && existsSync(this.dir + 'assets/voice/words.json') ? read('assets/voice/words.json') : {};
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
    if (this.look === 'smooth') return this.spot(selector, time, box, duration);
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
    if (this.look === 'smooth') this.fade(selector.replace(/-ring$/, '-spot'), time, 0, 0.3);
  }

  /**
   * The smooth look's ring: a feathered spotlight (`#<id>-spot`) whose clear area glides to `box`,
   * with a faint outline (the ring element) for precision. `selector` is the ring's.
   */
  spot(selector, time, box, duration = 0.7) {
    const spot = selector.replace(/-ring$/, '-spot');
    const area = { '--x0': `${box.x}px`, '--y0': `${box.y}px`, '--x1': `${r(box.x + box.w)}px`, '--y1': `${r(box.y + box.h)}px` };
    const css = (o) => JSON.stringify(o);
    if (this.shown.has(selector)) {
      this.tween(`tl.to('${spot}', { ...${css(area)}, duration: ${duration}, ease: 'power2.inOut' }, ${time});`);
      this.tween(`tl.to('${selector}', { x: ${box.x}, y: ${box.y}, width: ${box.w}, height: ${box.h}, duration: ${duration}, ease: 'power2.inOut' }, ${time});`);
      return;
    }
    this.shown.add(selector);
    this.tween(`tl.set('${spot}', ${css(area)}, ${time});`);
    this.tween(`tl.set('${selector}', { x: ${box.x}, y: ${box.y}, width: ${box.w}, height: ${box.h} }, ${time});`);
    this.tween(`tl.to(['${spot}', '${selector}'], { opacity: 1, duration: 0.5, ease: 'power2.out' }, ${time});`);
  }

  /** Brings in capture `#<id>-<shot>` over the current one by blurring it into focus, without moving the camera. */
  swap(selector, time) {
    this.tween(`tl.fromTo('${selector}', { opacity: 0, filter: 'blur(10px)' }, { opacity: 1, filter: 'blur(0px)', duration: 0.35, ease: 'power2.out' }, ${time});`);
  }

  /**
   * Glides pointer `selector` (a window's `#<id>-pointer`) so its tip lands in the middle of `box`.
   * The first call fades it in there, coming from slightly below and to the right.
   */
  point(selector, time, box, duration = 0.6) {
    const x = r(box.x + box.w / 2);
    const y = r(box.y + box.h / 2);
    if (this.shown.has(selector)) {
      this.tween(`tl.to('${selector}', { x: ${x}, y: ${y}, duration: ${duration}, ease: 'power3.out' }, ${time});`);
      return;
    }
    this.shown.add(selector);
    this.tween(`tl.fromTo('${selector}', { x: ${x + 60}, y: ${y + 40}, opacity: 0 }, { x: ${x}, y: ${y}, opacity: 1, duration: ${duration}, ease: 'power3.out' }, ${time});`);
  }

  /** The pointer presses: a short squeeze from its tip and back. */
  press(selector, time) {
    this.tween(`tl.to('${selector}', { scale: 0.86, duration: 0.09, ease: 'power2.in' }, ${time});`);
    this.tween(`tl.to('${selector}', { scale: 1, duration: 0.16, ease: 'power2.out' }, ${s3(time + 0.09)});`);
  }

  hidePointer(selector, time) {
    this.shown.delete(selector);
    this.fade(selector, time, 0, 0.25);
  }

  fade(selector, time, opacity, duration = 0.45) {
    // In the smooth look, a capture coming in blurs into focus instead. Which selectors are captures
    // is known once the windows are written, so write() decides.
    if (this.look === 'smooth' && opacity === 1) return this.tween({ fadeIn: selector, time, duration });
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
    for (const shot of shots.slice(1)) this.shots.add(`#${id}-${shot}`);
    return `      <section id="${id}" class="clip window" data-start="${s3(start)}" data-duration="${s3(duration)}" data-track-index="1" style="width: ${stage.w}px; margin-left: -${stage.w / 2}px; top: ${top}px">
        <div class="titlebar"><i></i><i></i><i></i><span>${title}</span></div>
        <div class="stage" style="width: ${stage.w}px; height: ${stage.h}px">
          <div id="${id}-camera" class="camera">
${shots.map((shot, i) => `            <img id="${id}-${shot}" class="shot${i ? ' later' : ''}" src="assets/shots/${shot}.png" alt="" />`).join('\n')}
${this.look === 'smooth' ? `            <div id="${id}-spot" class="spot" data-layout-allow-overflow></div>
` : ''}            <div id="${id}-ring" class="ring${this.look === 'smooth' ? ' outline' : ''}"></div>
${this.look === 'smooth' ? `            <svg id="${id}-pointer" class="pointer" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 3 L5 19 L9 15 L12 22 L15 20.5 L11.5 14 L18 14 Z" /></svg>
` : ''}          </div>
        </div>
      </section>`;
  }

  /**
   * When each word of scene `id`'s script is spoken, from the transcript. The two can split words
   * differently ("braindump" is heard as "brain dump"), so they are aligned by their letters: a
   * script word starts with the transcript word holding its first letter and ends with the one
   * holding its last. Null without a transcript, or when the letters differ.
   */
  wordTimes(id) {
    const spoken = this.words[id];
    if (!spoken?.length) return null;
    const letters = (s) => s.toLowerCase().replace(/[^a-z0-9]/g, '');
    const owner = [];
    spoken.forEach((w, i) => owner.push(...Array(letters(w.text).length).fill(i)));
    const words = this.scenes[id].text.split(/\s+/);
    if (owner.length !== letters(words.join('')).length) return null;
    let at = 0;
    return words.map((text) => {
      const n = Math.max(1, letters(text).length);
      const first = spoken[owner[Math.min(at, owner.length - 1)]];
      const last = spoken[owner[Math.min(at + n - 1, owner.length - 1)]];
      at += letters(text).length;
      return { start: first.start, end: last.end };
    });
  }

  /**
   * When the voice says `word` in scene `id` (its first occurrence, or the `nth`), plus `offset`
   * seconds. Without word times, the word's place in the line stands in for it.
   */
  word(id, word, { nth = 1, offset = 0 } = {}) {
    const letters = (s) => s.toLowerCase().replace(/[^a-z0-9]/g, '');
    const words = this.scenes[id].text.split(/\s+/);
    const index = words.map(letters).reduce((found, w, i) => (w === letters(word) ? [...found, i] : found), [])[nth - 1];
    if (index === undefined) throw new Error(`no "${word}" (${nth}) in scene ${id}`);
    const times = this.wordTimes(id);
    const start = times ? times[index].start : (this.scenes[id].length * index) / words.length;
    return s3(this.scenes[id].start + start + offset);
  }

  /**
   * The caption groups of scene `id` from the times of its spoken words: a clause of up to 8 words, broken at a
   * sentence end, or at a comma once 3 words are in, and at 8 words at most. Null when there are no word times, so the
   * caller falls back to sentences.
   */
  wordGroups(id) {
    const scene = this.scenes[id];
    const times = this.wordTimes(id);
    const words = scene.text.split(/\s+/);
    if (!times) return null;
    const groups = [];
    let group = [];
    words.forEach((text, i) => {
      group.push({ text, ...times[i] });
      const end = /[.;:?!]$/.test(text) || group.length === 8 || (group.length >= 3 && /,$/.test(text) && words.length - i > 2);
      if (end || i === words.length - 1) {
        groups.push(group);
        group = [];
      }
    });
    // A lone word, such as "one:" before a list, joins the group before it (or after, when first).
    for (let i = groups.length - 1; i >= 0 && groups.length > 1; i--) {
      if (groups[i].length > 1) continue;
      if (i > 0) groups[i - 1].push(...groups.splice(i, 1)[0]);
      else groups[1].unshift(...groups.splice(0, 1)[0]);
    }
    return groups.map((g, i) => {
      const start = scene.start + g[0].start - 0.08;
      const next = groups[i + 1];
      const end = next ? scene.start + next[0].start - 0.08 : scene.start + g.at(-1).end + this.gap * 0.8;
      return { start: s3(start), length: s3(end - start), text: g.map((w) => w.text).join(' ') };
    });
  }

  /** A tween as code: a deferred fade-in becomes a blur swap when it brings in a capture. */
  resolve(t) {
    if (typeof t === 'string') return t;
    if (this.shots.has(t.fadeIn)) {
      return `tl.fromTo('${t.fadeIn}', { opacity: 0, filter: 'blur(10px)' }, { opacity: 1, filter: 'blur(0px)', duration: 0.35, ease: 'power2.out' }, ${t.time});`;
    }
    return `tl.to('${t.fadeIn}', { opacity: 1, duration: ${t.duration}, ease: 'power1.inOut' }, ${t.time});`;
  }

  /** Writes index.html: `body` (the visual clips), captions per sentence, the narration and the timeline. */
  write({ body, css = '' }) {
    const captions = [];
    for (const [id, scene] of Object.entries(this.scenes)) {
      const grouped = this.look === 'smooth' ? this.wordGroups(id) : null;
      if (grouped) {
        grouped.forEach((g, index) => captions.push({ id: `cap-${id}-${index}`, ...g, quick: true }));
        continue;
      }
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
      if (c.quick) {
        // Word groups follow each other closely: fade in fast, and out just before the next one.
        this.tween(`tl.fromTo('#${c.id}', { opacity: 0 }, { opacity: 1, duration: 0.12, ease: 'power2.out' }, ${c.start});`);
        this.tween(`tl.to('#${c.id}', { opacity: 0, duration: 0.1, ease: 'power2.in' }, ${s3(c.start + c.length - 0.1)});`);
      } else {
        this.tween(`tl.fromTo('#${c.id}', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' }, ${c.start});`);
      }
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
    <div id="root" data-composition-id="main" data-start="0" data-duration="${this.total}" data-width="1920" data-height="1080"${this.look === 'smooth' ? ' data-motion-blur="on"' : ''}>
${body}

${captions.map((c) => `      <p id="${c.id}" class="clip caption" data-start="${c.start}" data-duration="${c.length}" data-track-index="3">${escape(c.text)}</p>`).join('\n')}

${this.script.map(({ id }) => `      <audio id="vo-${id}" src="assets/voice/${id}.wav" data-start="${this.scenes[id].start.toFixed(3)}" data-duration="${this.lengths[id]}" data-track-index="4"></audio>`).join('\n')}
    </div>
    <script>
      window.__timelines = window.__timelines || {};
      const tl = gsap.timeline({ paused: true });
      ${this.tweens.map((t) => this.resolve(t)).join('\n      ')}
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
      .ring.outline { border: 1.5px solid rgba(255, 255, 255, 0.75); box-shadow: 0 0 10px rgba(111, 155, 255, 0.25); }
      /* A dim layer, clear inside the box between --x0/--y0 and --x1/--y1 and feathered around it. */
      .spot {
        position: absolute;
        inset: -2000px;
        opacity: 0;
        background: rgba(8, 14, 28, 0.42);
        --f: 36px;
        --x0: 0px; --y0: 0px; --x1: 0px; --y1: 0px;
        mask-image:
          linear-gradient(to right, #000 calc(2000px + var(--x0) - var(--f)), transparent calc(2000px + var(--x0)), transparent calc(2000px + var(--x1)), #000 calc(2000px + var(--x1) + var(--f))),
          linear-gradient(to bottom, #000 calc(2000px + var(--y0) - var(--f)), transparent calc(2000px + var(--y0)), transparent calc(2000px + var(--y1)), #000 calc(2000px + var(--y1) + var(--f)));
        mask-composite: add;
      }
      .pointer {
        position: absolute;
        /* The arrow's tip is at (5, 3) of its 24-unit box: at 30 px, 6.25 px and 3.75 px in. */
        left: -6.25px;
        top: -3.75px;
        width: 30px;
        height: 30px;
        opacity: 0;
        transform-origin: 6.25px 3.75px;
        filter: drop-shadow(0 2px 3px rgba(0, 0, 0, 0.35));
        z-index: 5;
      }
      .pointer path { fill: #fff; stroke: #111; stroke-width: 1.3; stroke-linejoin: round; }

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
