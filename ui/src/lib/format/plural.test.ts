import { describe, expect, it } from 'vitest';
import { nodeCount, nodeNoun, plural } from './plural';

describe('counts', () => {
	it('use the singular for exactly one and the plural otherwise', () => {
		expect(plural(1, 'run')).toBe('1 run');
		expect(plural(0, 'run')).toBe('0 runs');
		expect(plural(1204, 'node')).toBe('1,204 nodes');
		expect(plural(2, 'job or trigger', 'jobs and triggers')).toBe('2 jobs and triggers');
	});

	it('name node kinds, telling SQL audits from Python checks', () => {
		expect(nodeCount(407, 'asset')).toBe('407 assets');
		expect(nodeCount(1, 'asset')).toBe('1 asset');
		expect(nodeCount(35, 'check')).toBe('35 Python checks');
		expect(nodeCount(4668, 'audit')).toBe('4,668 audits');
		expect(nodeNoun('table_fn', 2)).toBe('table functions');
		expect(nodeCount(3, 'widget')).toBe('3 widgets');
	});
});
