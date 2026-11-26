import { describe, expect, it } from 'vitest';
import { evaluateMockSelection, mockVocabulary, resolveSelection } from './operate-graph';
import './operate';

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
		// 4 plus 42 discovered steps (discovered.ts).
		expect(mockVocabulary().kinds).toEqual([
			{ value: 'asset', count: 46 },
			{ value: 'loader', count: 2 },
			// 2 from the run-page failure mock-up (seed-audit-run.ts); its 92 seeds load only where it is shown.
			{ value: 'model', count: 33 },
			{ value: 'seed', count: 1 },
			{ value: 'source', count: 6 }
		]);
	});

	it("a job run narrowed on the plan page resolves as the server does: job:core_build,fact_orders is fact_orders", () => {
		expect(resolveSelection(['job:core_build,fact_orders'])).toEqual({ nodes: ['fact_orders'], unknown: [] });
		expect(resolveSelection(['job:core_build,no_such_node']).unknown).toEqual(['job:core_build,no_such_node']);
	});
});
