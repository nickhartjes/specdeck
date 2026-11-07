import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    // styles.test.ts reads the stylesheet's text (`?raw`), which Vitest otherwise empties.
    css: { include: [/styles\.css/] },
  },
});
