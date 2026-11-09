// When each spoken word starts and ends, for captions that follow the voice. Runs
// `hyperframes transcribe` (local whisper) on each line speak.py wrote. The captions show the
// script's own words; lib/film.mjs aligns them to these by their letters. Writes
// <video>/assets/voice/words.json; a line whose wav is older than its entry is not transcribed again.
// Usage: node words.mjs <video folder>
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const dir = resolve(process.argv[2] ?? '.');
const voice = join(dir, 'assets/voice');
const out = join(voice, 'words.json');
const script = JSON.parse(readFileSync(join(dir, 'SCRIPT.json'), 'utf8'));
const words = existsSync(out) ? JSON.parse(readFileSync(out, 'utf8')) : {};
const done = existsSync(out) ? statSync(out).mtimeMs : 0;
const scratch = join(voice, 'transcript.json');

for (const { id } of script) {
  const wav = join(voice, `${id}.wav`);
  if (words[id] && statSync(wav).mtimeMs < done) continue;
  // Its JSON summary names the transcript it wrote (assets/voice/transcript.json, whatever --output says).
  const summary = execFileSync('pnpm', ['exec', 'hyperframes', 'transcribe', wav, '--model', 'small.en', '--json'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  });
  const { transcriptPath } = JSON.parse(summary.trim().split('\n').at(-1));
  words[id] = JSON.parse(readFileSync(transcriptPath, 'utf8')).map(({ text, start, end }) => ({ text, start, end }));
  console.log(`${id}: ${words[id].length} words`);
}
rmSync(scratch, { force: true });  // left by the last transcription
writeFileSync(out, JSON.stringify(words, null, 2));
