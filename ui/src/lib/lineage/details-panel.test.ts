import { describe, expect, it } from 'vitest';
import { detailsOpen, parseChoice, toggledChoice } from './details-panel';

describe('details sidebar', () => {
	it('is closed by default and opens while something is selected', () => {
		expect(detailsOpen(null, false)).toBe(false);
		expect(detailsOpen(null, true)).toBe(true);
	});

	it('keeps the person’s own choice, selection or not', () => {
		expect(detailsOpen('closed', true)).toBe(false);
		expect(detailsOpen('open', false)).toBe(true);
	});

	it('records the opposite of what is shown when toggled', () => {
		expect(toggledChoice(null, false)).toBe('open');
		expect(toggledChoice(null, true)).toBe('closed');
		expect(toggledChoice('closed', true)).toBe('open');
		expect(toggledChoice('open', false)).toBe('closed');
	});

	it('reads only choices it wrote', () => {
		expect(parseChoice('open')).toBe('open');
		expect(parseChoice('closed')).toBe('closed');
		expect(parseChoice('true')).toBeNull();
		expect(parseChoice(null)).toBeNull();
	});
});
