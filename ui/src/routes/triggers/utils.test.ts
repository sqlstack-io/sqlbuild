import { describe, expect, it } from 'vitest';
import { setInUi, setInUiCount, setInUiExplanation, stateWord } from './utils';

describe('setInUi', () => {
	it('is true only when the state differs from the code', () => {
		expect(setInUi({ state: 'running', defaultState: 'paused' })).toBe(true);
		expect(setInUi({ state: 'paused', defaultState: 'running' })).toBe(true);
		expect(setInUi({ state: 'running', defaultState: 'running' })).toBe(false);
		expect(setInUi({ state: 'paused', defaultState: 'paused' })).toBe(false);
	});

	it('counts the triggers set in the UI', () => {
		expect(
			setInUiCount([
				{ state: 'running', defaultState: 'paused' },
				{ state: 'paused', defaultState: 'paused' },
				{ state: 'paused', defaultState: 'running' }
			])
		).toBe(2);
		expect(setInUiCount([])).toBe(0);
	});
});

describe('setInUiExplanation', () => {
	it("names the code's setting in lowercase", () => {
		expect(setInUiExplanation({ state: 'running', defaultState: 'paused' })).toBe('The code sets this to paused; this choice is kept across deploys.');
		expect(setInUiExplanation({ state: 'paused', defaultState: 'running' })).toBe('The code sets this to on; this choice is kept across deploys.');
	});

	it('words states for mid-sentence use', () => {
		expect([stateWord('running'), stateWord('paused')]).toEqual(['on', 'paused']);
	});
});
