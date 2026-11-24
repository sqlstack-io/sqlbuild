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
	it("reads a worker's preparation as steps with durations that open to their sub-steps", () => {
		const phase = (id: string, at: string, p: Partial<NonNullable<RunEvent['phase']>> & { phase: string }) =>
			event(id, at, { type: 'invocation_phase', nodeKey: null, phase: { durationMs: null, count: null, name: null, detail: null, ...p } });
		const entries = logEntries(
			[
				phase('d', '2026-11-23T10:00:05Z', { phase: 'discovered', durationMs: 5000 }),
				phase('c', '2026-11-23T10:00:09Z', { phase: 'compiled', durationMs: 4100, count: 2281, detail: '12 python nodes, 2269 models' }),
				phase('s', '2026-11-23T10:00:09Z', { phase: 'discovered_set', name: 'fulfilment_plan', count: 3, detail: 'release 2026.11 · discovered 2026-11-23 08:00 UTC' }),
				phase('k', '2026-11-23T10:00:10Z', { phase: 'connected', durationMs: 300, count: 9, name: 'snowflake' }),
				phase('p', '2026-11-23T10:00:12Z', { phase: 'planned', durationMs: 1500, count: 93, detail: 'source freshness 1.12 s' }),
				phase('x', '2026-11-23T10:00:13Z', { phase: 'started_execution', count: 8, detail: '9 connections' })
			],
			[]
		);
		expect(entries.map((e) => [e.text, e.tag?.text, e.tag?.repeats])).toEqual([
			['Discovered project · 5.0 s', 'preparing', false],
			['Compiled project · 2,281 models · 4.1 s', 'preparing', false],
			['Used discovered set fulfilment_plan · 3 items', 'preparing', false],
			['Connected to snowflake · 0.30 s', 'preparing', false],
			['Planned · 93 selected · 1.5 s', 'planned', true],
			['Started execution · concurrency 8 · 9 connections', 'started', true]
		]);
		expect(entries[5].details).toEqual([]);
		expect(entries[2].details).toEqual([
			['Detail', 'release 2026.11'],
			['Detail', 'discovered 2026-11-23 08:00 UTC']
		]);
		expect(entries[3].details).toEqual([['Connections', '9']]);
	});

	it('says when the worker became ready and how long it took, with the command behind the row', () => {
		const entries = logEntries(
			[
				event('launch', '2026-11-23T10:00:00Z', { type: 'launched', nodeKey: null, message: 'Worker started (local process)' }),
				event('ready', '2026-11-23T10:00:04Z', { type: 'invocation_started', nodeKey: null, message: 'sqb build · target uat · 93 selected · SQLBuild 0.259.0' })
			],
			[]
		);
		expect(entries.map((e) => e.text)).toEqual(['Worker started (local process)', 'Worker ready · 4.0 s']);
		expect(entries[1].details).toEqual([['Command', 'sqb build · target uat · 93 selected · SQLBuild 0.259.0']]);
	});

	it('gives run-level rows a kind in plain words, left out where the text already says it', () => {
		const entries = logEntries(
			[
				event('q', '2026-11-23T10:00:00Z', { type: 'queued', nodeKey: null }),
				event('c', '2026-11-23T10:00:01Z', { type: 'claimed', nodeKey: null }),
				event('r', '2026-11-23T10:00:02Z', { type: 'invocation_started', nodeKey: null }),
				event('f', '2026-11-23T10:00:03Z', { type: 'invocation_failed', nodeKey: null, level: 'error', message: '1 failed' }),
				event('t', '2026-11-23T10:00:04Z', { type: 'terminated', nodeKey: null, message: 'Terminated: failed' })
			],
			[]
		);
		expect(entries.map((e) => [e.text, e.tag?.text, e.tag?.repeats])).toEqual([
			['Queued', 'queued', true],
			['Claimed write lock', 'queued', false],
			['Worker ready', 'ready', true],
			['sqb finished with failures: 1 failed', 'failed', true],
			['Finished: failed', 'finished', true]
		]);
	});

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
