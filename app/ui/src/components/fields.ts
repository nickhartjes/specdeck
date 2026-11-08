/**
 * Props for every text field: what people type reaches the agent, the session folder or other
 * people as typed. With *Use smart quotes and dashes* on (the macOS default), WebKit replaces
 * `--` with `—` and straight quotes with curly ones while typing, unless spell checking is off;
 * it ignores `autocorrect` and `autocapitalize` for that (#85). Those two stop iOS and iPadOS
 * Safari changing words and capitals in the web viewer.
 */
export const PLAIN_TEXT = { spellCheck: false, autoCorrect: 'off', autoCapitalize: 'off' } as const;
