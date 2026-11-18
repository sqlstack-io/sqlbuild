import { describe, expect, it } from 'vitest';
import { fitStatus, STATUS_ROOM } from './status-line';
import type { Measure } from '$lib/components/graph/compact-title';

// Every character 6 px wide at any size, so widths are easy to reason about.
const measure: Measure = (text) => text.length * 6;

describe('a card status line on one line', () => {
	it('shows every part in full when they fit', () => {
		expect(fitStatus({ label: 'Success', when: '2m ago', checks: '✓ 1' }, measure)).toEqual({ label: true, driftWord: true });
	});

	it('shortens the drift chip to its icon first, keeping the status word and time', () => {
		const parts = { label: 'Success', when: '23m ago', drift: 'upstream', checks: '✓ 2' };
		expect(fitStatus(parts, measure, 170)).toEqual({ label: true, driftWord: false });
	});

	it('then drops the status word to its dot; the time stays', () => {
		const parts = { label: 'Warning', when: '23 minutes ago', drift: 'upstream', checks: '0/12' };
		expect(fitStatus(parts, measure, 150)).toEqual({ label: false, driftWord: false });
	});

	it('has the room of a card', () => {
		expect(STATUS_ROOM).toBe(214);
	});
});
