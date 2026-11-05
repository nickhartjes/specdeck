import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The web viewer (`web-viewer` design decision 4): web.html only, into dist-web, which the sharing
// service builds into its binary. Relative asset paths, so it works under any path prefix.
// Needs the WebAssembly package first: `just web-wasm`.
export default defineConfig({
  plugins: [react()],
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
