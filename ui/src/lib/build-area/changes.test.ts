import { describe, expect, it } from 'vitest';
import type { ChangeOp, NodeChange } from '$lib/api';
import { nodeChangeGroups, nodeChangeLine } from './changes';

const change = (key: string, op: ChangeOp = 'added'): NodeChange => ({ key, kind: null, op, formerKey: null, compiledOnly: false, detail: '', columns: [] });
const many = (n: number, prefix: string, op: ChangeOp = 'added') => Array.from({ length: n }, (_, i) => change(`${prefix}:n${i}`, op));

describe('node change summaries', () => {
	it('counts a first deploy by change and kind, largest first, in plain nouns', () => {
		const nodes = [...many(3, 'model'), ...many(1200, 'audit'), change('udf:margin'), ...many(2, 'model', 'removed'), change('widget:x')];
		expect(nodeChangeGroups(nodes)).toEqual([
			{ op: 'added', prefix: 'audit', count: 1200, noun: 'checks' },
			{ op: 'added', prefix: 'model', count: 3, noun: 'models' },
			{ op: 'removed', prefix: 'model', count: 2, noun: 'models' },
			{ op: 'added', prefix: 'udf', count: 1, noun: 'function' },
			{ op: 'added', prefix: 'widget', count: 1, noun: 'widget' }
		]);
		expect(nodeChangeLine(nodes)).toBe('added 1,200 checks · added 3 models · removed 2 models · 2 more kinds');
	});

	it('keeps SQL and Python hooks apart, so each kind is its own group', () => {
		expect(nodeChangeGroups([change('sql_hook:a'), change('hook:b')]).map((g) => [g.prefix, g.noun])).toEqual([
			['hook', 'Python hook'],
			['sql_hook', 'SQL hook']
		]);
	});

	it('names the nodes when there are only a few', () => {
		expect(nodeChangeLine([change('model:orders', 'changed'), change('seed:carriers')])).toBe('changed model:orders · added seed:carriers');
		expect(nodeChangeLine([])).toBe('No node changes');
	});
});
