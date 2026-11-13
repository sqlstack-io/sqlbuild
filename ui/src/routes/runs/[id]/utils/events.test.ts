import { describe, expect, it } from 'vitest';
import type { RunEvent } from '$lib/api';
import { logEntries } from './events';

const event = (id: string, at: string, extra: Partial<RunEvent> = {}): RunEvent => ({
	id,
	storageId: 1,
	runId: 'staging',
	producer: 'sqb',
	type: 'resource_attempt_started',
	occurredAt: at,
	level: 'info',
	invocationId: null,
	nodeKey: 'model:raw',
	statementId: null,
	attempt: null,
	message: '',
	...extra
});

describe('run log', () => {
	it("interleaves a group's events and printed lines by time and keeps each line's run", () => {
		const entries = logEntries(
			[event('e2', '2026-11-09T08:17:38Z', { type: 'resource_attempt_failed', level: 'error', message: 'ConversionError: no', runId: 'staging' }), event('e1', '2026-11-09T08:17:35Z', { nodeKey: 'model:orders' })],
			[{ runId: 'orders', invocationId: 'i', stream: 'stderr', at: '2026-11-09T08:17:36Z', text: 'Compiling project...', error: false }]
		);
		expect(entries.map((e) => [e.id, e.runId, e.category, e.level])).toEqual([
			['e1', 'staging', 'nodes', 'info'],
			['out-orders-0', 'orders', 'output', 'info'],
			['e2', 'staging', 'nodes', 'error']
		]);
		expect(entries[2].text).toBe('Failed: no');
	});

	it("folds a node's finished queries under its outcome, drops its start row, and leaves run-level queries out", () => {
		const entries = logEntries(
			[
				event('start', '2026-11-09T08:00:01Z'),
				event('sent', '2026-11-09T08:00:02Z', { type: 'statement_submitted', statementId: 's1' }),
				event('q1', '2026-11-09T08:00:03Z', { type: 'statement_completed', statementId: 's1', message: '3 rows' }),
				event('q2', '2026-11-09T08:00:04Z', { type: 'statement_completed', statementId: 's2', message: '0 rows' }),
				event('done', '2026-11-09T08:00:05Z', { type: 'resource_attempt_completed', message: '1.2s · 3 rows' }),
				event('other', '2026-11-09T08:00:06Z', { nodeKey: 'model:orders' }),
				event('book', '2026-11-09T08:00:07Z', { type: 'statement_completed', statementId: 's3', nodeKey: null })
			],
			[]
		);
		expect(entries.map((e) => e.id)).toEqual(['done', 'other']);
		expect(entries[0].text).toBe('Finished · 1.2s · 3 rows');
		expect(entries[0].queries.map((q) => q.id)).toEqual(['q1', 'q2']);
		// A node still running keeps its start row and has nothing folded.
		expect(entries[1].queries).toEqual([]);
	});

	it('keeps a failed query on its own row as well as under its node', () => {
		const entries = logEntries(
			[
				event('q', '2026-11-09T08:00:03Z', { type: 'statement_failed', level: 'error', statementId: 's1', message: 'bad cast' }),
				event('fail', '2026-11-09T08:00:04Z', { type: 'resource_attempt_failed', level: 'error', message: 'ConversionError: bad cast' })
			],
			[]
		);
		expect(entries.map((e) => [e.id, e.category])).toEqual([
			['q', 'queries'],
			['fail', 'nodes']
		]);
		expect(entries[1].queries.map((q) => q.id)).toEqual(['q']);
	});

	it("pins a failing run's last stderr block and every recorded error, and no other output", () => {
		const out = (runId: string, stream: 'stdout' | 'stderr', at: string, error = false) => ({ runId, invocationId: 'i', stream, at, text: `${runId} ${stream}`, error });
		const entries = logEntries(
			[event('fail', '2026-11-09T08:00:04Z', { type: 'resource_attempt_failed', level: 'error', message: 'X: no' }), event('ok', '2026-11-09T08:00:04Z', { runId: 'orders', type: 'resource_attempt_completed' })],
			[out('staging', 'stderr', '2026-11-09T08:00:01Z'), out('staging', 'stderr', '2026-11-09T08:00:05Z'), out('staging', 'stdout', '2026-11-09T08:00:06Z'), out('orders', 'stderr', '2026-11-09T08:00:07Z'), out('orders', 'stderr', '2026-11-09T08:00:08Z', true)]
		);
		expect(entries.filter((e) => e.pinned).map((e) => [e.id, e.level])).toEqual([
			['out-staging-1', 'info'],
			['out-orders-4', 'error']
		]);
	});
});
