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
			[event('e2', '2026-11-09T08:17:38Z', { type: 'resource_attempt_failed', level: 'error', message: 'ConversionError: no', runId: 'staging' }), event('e1', '2026-11-09T08:17:35Z')],
			[{ runId: 'orders', invocationId: 'i', stream: 'stderr', at: '2026-11-09T08:17:36Z', text: 'Compiling project...' }]
		);
		expect(entries.map((e) => [e.id, e.runId, e.category, e.level])).toEqual([
			['e1', 'staging', 'nodes', 'info'],
			['out-orders-0', 'orders', 'output', 'info'],
			['e2', 'staging', 'nodes', 'error']
		]);
		expect(entries[2].text).toBe('Failed: no');
	});
});
