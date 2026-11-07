import { useEffect, useRef } from 'react';
import { MessagesSquare, SquareTerminal } from 'lucide-react';
import type { Preset } from '../model/layout';

/** Asked the first time someone joins a session: how they want to follow it, as a starting layout. */
export function FirstJoinDialog({ onChoose }: { onChoose: (preset: Preset) => void }) {
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => first.current?.focus(), []);
  return (
    <div className="overlay">
      <div className="dialog first-join" role="dialog" aria-modal="true" aria-labelledby="first-join-title" aria-describedby="first-join-hint">
        <h2 id="first-join-title">How do you want to follow sessions?</h2>
        <p id="first-join-hint">This sets up your window. You can change the layout any time in the toolbar.</p>
        <div className="choices">
          <button ref={first} className="choice" onClick={() => onChoose('overview')}>
            <MessagesSquare aria-hidden />
            <strong>Overview</strong>
            <span>The documents and the timeline: what was asked, what the agent did in plain words, and the plans it wrote. No terminal.</span>
          </button>
          <button className="choice" onClick={() => onChoose('developer')}>
            <SquareTerminal aria-hidden />
            <strong>Developer</strong>
            <span>The code, the terminal, and every tool call, command and diff.</span>
          </button>
        </div>
      </div>
    </div>
  );
}
