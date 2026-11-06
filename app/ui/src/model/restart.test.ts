import { describe, expect, it } from 'vitest';
import { restartQuestion } from './restart';

describe('the question before restarting', () => {
  it('asks nothing without a watched session', () => {
    expect(restartQuestion('1.7.0', 0, 0)).toBeNull();
  });

  it('asks nothing for hosted sessions, which keep running', () => {
    expect(restartQuestion('1.7.0', 1, 0)).toBeNull();
    expect(restartQuestion('1.7.0', 3, 0)).toBeNull();
  });

  it('asks a viewer as before', () => {
    expect(restartQuestion('1.7.0', 0, 1)).toEqual({
      title: 'Leave the session and restart?',
      confirm: 'Leave and Restart',
      danger: false,
      text: 'Restarting installs SpecDeck 1.7.0. You stop watching; you can join again with the invite code while the session runs.',
    });
  });

  it('asks once for every watched session, and says hosted ones keep running', () => {
    const question = restartQuestion('1.7.0', 2, 1)!;
    expect(question.title).toBe('Leave the session and restart?');
    expect(question.text).toMatch(/ The 2 sessions you host keep running\.$/);
    expect(restartQuestion('1.7.0', 1, 3)!.title).toBe('Leave 3 sessions and restart?');
    expect(restartQuestion('1.7.0', 1, 3)!.text).toMatch(/ The session you host keeps running\.$/);
  });
});
