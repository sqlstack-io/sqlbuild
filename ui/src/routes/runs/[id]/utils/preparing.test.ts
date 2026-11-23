import { describe, expect, it } from 'vitest';
import type { RunEvent, RunNode } from '$lib/api';
import { preparingSpan } from './preparing';

const event = (type: RunEvent['type'], at: string, extra: Partial<RunEvent> = {}): RunEvent => ({
	id: `${type}-${at}`,
	storageId: 1,
	runId: 'run',
	producer: 'sqb',
	type,
	occurredAt: at,
	level: 'info',
	invocationId: null,
	nodeKey: null,
	statementId: null,
	attempt: null,
	message: '',
	...extra
});
const node = (startedAt: string | null) => ({ runId: 'run', nodeKey: 'model:orders', startedAt }) as RunNode;

describe('preparing span', () => {
	const events = [
		event('launched', '2026-11-23T10:00:00Z'),
		event('invocation_phase', '2026-11-23T10:00:09Z', { phase: { phase: 'compiled', durationMs: 4100, count: 12, name: null, detail: null } })
	];

	it("runs from the worker's launch to the first node's start, with its steps", () => {
		expect(preparingSpan(events, 'run', [node('2026-11-23T10:00:17Z'), node(null)], 0)).toEqual({
			start: Date.parse('2026-11-23T10:00:00Z'),
			end: Date.parse('2026-11-23T10:00:17Z'),
			live: false,
			steps: ['Compiled project · 12 models · 4.1 s']
		});
	});

	it('runs to now while the worker still prepares, and is absent before it launches', () => {
		const now = Date.parse('2026-11-23T10:00:12Z');
		expect(preparingSpan(events, 'run', [node(null)], now)).toMatchObject({ end: now, live: true });
		expect(preparingSpan([], 'run', [], now)).toBeNull();
	});
});
