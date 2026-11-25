import { describe, expect, it } from 'vitest';
import { toSelectionEvaluation, toSelectionVocabulary } from './http-selection';

describe('selection mapping', () => {
	it('keeps a page of keys and the total, with no error', () => {
		expect(toSelectionEvaluation({ snapshot_id: 'snap-1', count: 3, audits: 2, keys: ['model:customers', 'model:orders'], next_offset: 2, error: null })).toEqual({
			snapshotId: 'snap-1',
			count: 3,
			audits: 2,
			keys: ['model:customers', 'model:orders'],
			nextOffset: 2,
			error: null
		});
	});

	it('keeps where a refused selector is and what might have been meant', () => {
		const evaluation = toSelectionEvaluation({
			snapshot_id: 'snap-1',
			count: 0,
			keys: [],
			next_offset: null,
			error: {
				code: 'S000',
				message: "no SQL resources or Python nodes found with tag 'ordrs'",
				help: null,
				location: { field: 'select', token: 'tag:ordrs', start: 10, end: 19 },
				suggestions: ['tag:orders'],
				refused: { part: '+tag:ordrs', index: 1, type: 'tag', value: 'ordrs' }
			}
		});
		expect(evaluation.error).toEqual({
			code: 'S000',
			message: "no SQL resources or Python nodes found with tag 'ordrs'",
			help: null,
			location: { field: 'select', token: 'tag:ordrs', start: 10, end: 19 },
			suggestions: ['tag:orders'],
			refused: { part: 1, type: 'tag', value: 'ordrs' }
		});
	});

	it('reads a refused bare name or name pattern as an untyped part, and no refused part as null', () => {
		const refusal = (refused: { part: string; index: number; type: string; value: string } | null) =>
			toSelectionEvaluation({
				snapshot_id: 'snap-1',
				count: 0,
				keys: [],
				next_offset: null,
				error: { code: 'S007', message: 'unknown selector', help: null, location: null, suggestions: [], refused }
			}).error?.refused;
		expect([
			refusal({ part: 'custmers', index: 0, type: 'name', value: 'custmers' }),
			refusal({ part: 'missing_*', index: 0, type: 'glob', value: 'missing_*' }),
			refusal(null)
		]).toEqual([{ part: 0, type: null, value: 'custmers' }, { part: 0, type: null, value: 'missing_*' }, null]);
	});

	it('maps each selector type to its values and counts', () => {
		const entry = (value: string, count: number) => ({ value, count });
		expect(
			toSelectionVocabulary({
				snapshot_id: null,
				tags: [entry('orders', 4)],
				paths: [entry('models/marts', 3)],
				jobs: [entry('nightly_orders', 12)],
				sources: [entry('shop.orders', 1)],
				seeds: [entry('currencies', 1)],
				kinds: [entry('model', 4)],
				audits: [entry('not_null_orders_id', 1)]
			})
		).toEqual({
			snapshotId: null,
			tags: [entry('orders', 4)],
			paths: [entry('models/marts', 3)],
			jobs: [entry('nightly_orders', 12)],
			sources: [entry('shop.orders', 1)],
			seeds: [entry('currencies', 1)],
			kinds: [entry('model', 4)],
			audits: [entry('not_null_orders_id', 1)]
		});
	});
});
