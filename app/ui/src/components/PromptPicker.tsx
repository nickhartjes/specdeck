import type { Turn } from '../model/timeline';

/** A list of the session's prompts; picking one jumps to it (`viewer-experience` spec: Jump to prompt). */
export function PromptPicker({ id, label, turns, onPick }: { id: string; label: string; turns: Turn[]; onPick: (promptId: string) => void }) {
  return (
    <>
      <label htmlFor={id} className="field-hint">
        {label}
      </label>
      <select id={id} value="" onChange={(event) => onPick(event.target.value)}>
        <option value="" disabled>
          Prompt…
        </option>
        {turns.map((turn, i) => (
          <option key={turn.promptId} value={turn.promptId}>
            {i + 1}. {turn.prompt.slice(0, 60) || 'Earlier prompt'}
          </option>
        ))}
      </select>
    </>
  );
}
