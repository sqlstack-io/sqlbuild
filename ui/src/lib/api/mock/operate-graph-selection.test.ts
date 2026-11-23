import { describe, expect, it } from 'vitest';
import { evaluateMockSelection, mockVocabulary } from './operate-graph';

describe('mock selection evaluation', () => {
	it('expands + on its own comma part before intersecting', () => {
		expect(evaluateMockSelection('+fact_orders,kind:source', '')).toEqual({ keys: ['raw__customers', 'raw__payments'], error: null });
	});

	it('locates an unknown tag in the field it was typed in and suggests a close one', () => {
		expect(evaluateMockSelection('tag:core', 'tag:fin').error).toEqual({
			code: 'S007',
			message: "unknown selector name 'tag:fin'",
			help: null,
			location: { field: 'exclude', token: 'tag:fin', start: 0, end: 7 },
			suggestions: ['tag:finance'],
			refused: { part: 0, type: 'tag', value: 'fin' }
		});
	});

	it('keeps the + of the part it fixes in a suggestion', () => {
		expect(evaluateMockSelection('+tag:finanse,kind:model', '').error?.suggestions).toEqual(['+tag:finance,kind:model']);
	});

	it('counts models of every materialization as one kind', () => {
		// 4 plus 44 discovered and completing nodes (discovered.ts).
		expect(mockVocabulary().kinds).toEqual([
			{ value: 'asset', count: 48 },
			{ value: 'loader', count: 2 },
			{ value: 'model', count: 31 },
			{ value: 'seed', count: 1 },
			{ value: 'source', count: 6 }
		]);
	});
});
