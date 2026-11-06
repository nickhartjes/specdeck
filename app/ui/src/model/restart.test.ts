import { describe, expect, it } from 'vitest';
import { restartQuestion } from './restart';

describe('the question before restarting', () => {
  it('asks nothing without a session', () => {
    expect(restartQuestion('1.7.0', 0, 0)).toBeNull();
  });

  it('asks a host and a viewer as before', () => {
    expect(restartQuestion('1.7.0', 1, 0)).toEqual({
      title: 'End the session for everyone and restart?',
      confirm: 'End Session and Restart',
      danger: true,
      text: 'Restarting installs SpecDeck 1.7.0. It ends your session: everyone watching is disconnected and the agent stops.',
    });
    expect(restartQuestion('1.7.0', 0, 1)).toEqual({
      title: 'Leave the session and restart?',
      confirm: 'Leave and Restart',
      danger: false,
      text: 'Restarting installs SpecDeck 1.7.0. You stop watching; you can join again with the invite code while the session runs.',
    });
  });

  it('asks once for the sessions of every window', () => {
    const question = restartQuestion('1.7.0', 2, 1)!;
    expect(question.title).toBe('End 2 sessions for everyone and restart?');
    expect(question.confirm).toBe('End Sessions and Restart');
    expect(question.danger).toBe(true);
    expect(question.text).toBe(
      'Restarting installs SpecDeck 1.7.0. It ends the 2 sessions you host: everyone watching them is disconnected and the agents stop. You also stop watching the session you watch.',
    );
    expect(restartQuestion('1.7.0', 1, 2)!.text).toMatch(/It ends your session: .* You also stop watching the 2 sessions you watch\.$/);
    expect(restartQuestion('1.7.0', 0, 3)!.title).toBe('Leave 3 sessions and restart?');
  });
});
