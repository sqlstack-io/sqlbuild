// Pure helpers for the Triggers list.
import type { Trigger, TriggerState } from '$lib/api';

type StateOf = Pick<Trigger, 'state' | 'defaultState'>;

/** Whether someone set this trigger in the UI, so it differs from the code's default. */
export function setInUi(t: StateOf): boolean {
	return t.state !== t.defaultState;
}

/** How many triggers are set in the UI. */
export function setInUiCount(triggers: readonly StateOf[]): number {
	return triggers.filter(setInUi).length;
}

/** `on` or `paused`, lowercase for use mid-sentence. */
export function stateWord(state: TriggerState): 'on' | 'paused' {
	return state === 'running' ? 'on' : 'paused';
}

/** The pin popup's sentence after "Set in UI.". */
export function setInUiExplanation(t: StateOf): string {
	return `The code sets this to ${stateWord(t.defaultState)}; this choice is kept across deploys.`;
}
