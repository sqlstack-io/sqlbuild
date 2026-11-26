import { describe, expect, it } from 'vitest';
import { describeQuery, holdsText, QUICK, QUICK_LIST, toggleText } from './log-query';

describe('the log filter text', () => {
	it('a quick filter adds its words, and a second click takes out only those', () => {
		const on = toggleText('orders ', QUICK.auditsFailed.text);
		expect(on).toBe('orders kind:audit outcome:failed ');
		expect(holdsText(on, QUICK.auditsFailed.text)).toBe(true);
		expect(toggleText(on, QUICK.auditsFailed.text)).toBe('orders ');
	});

	it('words typed by hand press the same quick filter, in any order or case', () => {
		expect(holdsText('outcome:FAILED  kind:audit', QUICK.auditsFailed.text)).toBe(true);
		expect(holdsText('kind:audit', QUICK.auditsFailed.text)).toBe(false);
	});

	it('a quick filter takes the place of a word with the same key', () => {
		expect(toggleText('level:warn node:a ', QUICK.errors.text)).toBe('node:a level:error ');
		expect(toggleText('outcome:built ', QUICK.skipped.text)).toBe('outcome:skipped ');
	});

	it('the Showing line names a quick filter only when the text is exactly its words', () => {
		expect(describeQuery('outcome:failed kind:audit ', QUICK_LIST)).toEqual({ label: 'failed audits', typed: false });
		expect(describeQuery('level:error orders', QUICK_LIST)).toEqual({ label: 'level:error orders', typed: true });
		expect(describeQuery('  ', QUICK_LIST)).toBeNull();
	});
});
