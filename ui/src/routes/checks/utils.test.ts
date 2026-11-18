import { describe, expect, it } from 'vitest';
import { parseOutcomeFilter } from './utils';

describe('parseOutcomeFilter', () => {
	it('reads the outcome a link asks for', () => {
		expect(parseOutcomeFilter('failing')).toBe('failing');
		expect(parseOutcomeFilter('warn')).toBe('warn');
	});

	it('shows every check for a missing or unknown outcome', () => {
		expect(parseOutcomeFilter(null)).toBe('all');
		expect(parseOutcomeFilter('broken')).toBe('all');
	});
});
