import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';

export default tseslint.config(
  { ignores: ['**/dist/**', '**/node_modules/**', 'target/**', 'app/src-tauri/gen/**', 'docs/site/.astro/**', 'crates/web/pkg/**', '**/dist-web/**', 'docs/video/.venv/**', 'docs/video/*/.hyperframes/**'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['scripts/**/*.{js,mjs}', 'docs/site/scripts/**/*.mjs', 'docs/video/**/*.mjs'],
    languageOptions: { globals: { ...globals.node } },
  },
  {
    // What runs inside Claude's browser: one expression the app evaluates in the page.
    files: ['app/src-tauri/src/browser/browser.js'],
    languageOptions: { globals: { ...globals.browser } },
    rules: { '@typescript-eslint/no-unused-expressions': 'off' },
  },
  {
    files: ['app/ui/**/*.{ts,tsx}'],
    languageOptions: { globals: { ...globals.browser } },
    plugins: { 'react-hooks': reactHooks },
    rules: reactHooks.configs.recommended.rules,
  },
);
