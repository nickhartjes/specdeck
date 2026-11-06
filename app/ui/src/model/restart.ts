// The question before Restart to update while sessions are open: one question for the sessions
// of every window (`multiple-windows` design decision 8). Hosted sessions run in processes of
// their own and keep running through the restart (`session-processes`), so only watched ones are
// asked about.

export interface RestartQuestion {
  title: string;
  confirm: string;
  /** Leaving only affects the user; nobody else is disconnected. */
  danger: boolean;
  text: string;
}

/** What to ask before restarting for `version`, or null when no session is watched. */
export function restartQuestion(version: string, hosted: number, watched: number): RestartQuestion | null {
  if (watched === 0) return null;
  const keep = hosted === 0 ? '' : hosted === 1 ? ' The session you host keeps running.' : ` The ${hosted} sessions you host keep running.`;
  return {
    title: watched === 1 ? 'Leave the session and restart?' : `Leave ${watched} sessions and restart?`,
    confirm: 'Leave and Restart',
    danger: false,
    text: `Restarting installs SpecDeck ${version}. You stop watching${watched === 1 ? '' : ` ${watched} sessions`}; you can join again with the invite code while the session runs.${keep}`,
  };
}
