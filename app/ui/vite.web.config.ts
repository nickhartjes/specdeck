import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The web viewer (`web-viewer` design decision 4): web.html only, into dist-web, which the sharing
// service builds into its binary. Relative asset paths, so it works under any path prefix.
// Needs the WebAssembly package first: `just web-wasm`.

/** The release this page is built from: the workspace version, for the diagnostics export. */
function releaseVersion(): string {
  const cargo = readFileSync(fileURLToPath(new URL('../../Cargo.toml', import.meta.url)), 'utf8');
  const version = /\[workspace\.package\][^[]*?\bversion\s*=\s*"([^"]+)"/.exec(cargo)?.[1];
  if (!version) throw new Error('No workspace version in Cargo.toml');
  return version;
}

export default defineConfig({
  plugins: [react()],
  define: { __SPECDECK_VERSION__: JSON.stringify(releaseVersion()) },
  base: './',
  resolve: {
    alias: { 'specdeck-web-wasm': fileURLToPath(new URL('../../crates/web/pkg/specdeck_web.js', import.meta.url)) },
  },
  server: { port: 5174, strictPort: true, fs: { allow: ['../..'] } },
  build: {
    target: 'es2023',
    outDir: 'dist-web',
    emptyOutDir: true,
    rollupOptions: { input: fileURLToPath(new URL('web.html', import.meta.url)) },
  },
});
