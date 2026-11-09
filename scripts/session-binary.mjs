// Builds specdeck-session and puts it where Tauri's bundle.externalBin looks for it,
// app/src-tauri/binaries/specdeck-session-<target triple>[.exe], so the installers carry it next to
// the app (session-processes design decision 1). Run before every installer build by
// app/src-tauri/tauri.bundle.conf.json; Tauri sets TAURI_ENV_TARGET_TRIPLE and TAURI_ENV_DEBUG.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)[1];
const target = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const debug = process.env.TAURI_ENV_DEBUG === 'true';
const profile = debug ? 'debug' : 'release';

// With the app selected too and Tauri's feature, so Cargo resolves the features `tauri build` does
// next: built for specdeck-session alone, tokio, rustls and with them the vendored libdatachannel
// and OpenSSL came out different and were compiled twice per installer build (8 to 12 minutes on
// Windows). --bin still builds only the session binary.
const args = [
  'build',
  '-p',
  'specdeck-app',
  '-p',
  'specdeck-session',
  '--bin',
  'specdeck-session',
  '--features',
  'tauri/custom-protocol',
  '--locked',
];
if (!debug) args.push('--release');
// Built where the app is, so the dependencies compile once: like `tauri build`, with --target only
// for another platform, unless the app is built with --target anyway (SESSION_BINARY_TARGET=1, as
// in the Nix package).
const cross = target !== host || process.env.SESSION_BINARY_TARGET === '1';
if (cross) args.push('--target', target);
execFileSync('cargo', args, { cwd: root, stdio: 'inherit' });

const exe = target.includes('windows') ? '.exe' : '';
const targetDir = process.env.CARGO_TARGET_DIR || join(root, 'target');
const built = join(targetDir, ...(cross ? [target] : []), profile, `specdeck-session${exe}`);
const bundled = join(root, 'app', 'src-tauri', 'binaries', `specdeck-session-${target}${exe}`);
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
console.log(`specdeck-session for ${target}: ${bundled}`);
