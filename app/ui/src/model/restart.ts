// The question before Restart to update while sessions are open: one question for the sessions
// of every window (`multiple-windows` design decision 8).

export interface RestartQuestion {
  title: string;
  confirm: string;
  /** Ending hosted sessions disconnects other people. */
  danger: boolean;
  text: string;
}

const sessions = (n: number, what: string) => (n === 1 ? `the session you ${what}` : `the ${n} sessions you ${what}`);

/** What to ask before restarting for `version`, or null when no session is open. */
export function restartQuestion(version: string, hosted: number, watched: number): RestartQuestion | null {
  if (hosted === 0 && watched === 0) return null;
  const installs = `Restarting installs SpecDeck ${version}.`;
  if (hosted === 0) {
    return {
      title: watched === 1 ? 'Leave the session and restart?' : `Leave ${watched} sessions and restart?`,
      confirm: 'Leave and Restart',
      danger: false,
      text: `${installs} You stop watching${watched === 1 ? '' : ` ${watched} sessions`}; you can join again with the invite code while the session runs.`,
    };
  }
  const alsoWatching = watched > 0 ? ` You also stop watching ${sessions(watched, 'watch')}.` : '';
  if (hosted === 1) {
    return {
      title: 'End the session for everyone and restart?',
      confirm: 'End Session and Restart',
      danger: true,
      text: `${installs} It ends your session: everyone watching is disconnected and the agent stops.${alsoWatching}`,
    };
  }
  return {
    title: `End ${hosted} sessions for everyone and restart?`,
    confirm: 'End Sessions and Restart',
    danger: true,
    text: `${installs} It ends ${sessions(hosted, 'host')}: everyone watching them is disconnected and the agents stop.${alsoWatching}`,
  };
}
