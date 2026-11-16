import { describe, expect, it } from 'vitest';
import { concurrencyText, describeWait, onFailureText, overlapPlain, triggerKindLabel, waitTechnical } from './labels';
import type { QueueWait } from '$lib/api';

const text = (w: QueueWait) => describeWait(w).map((p) => p.text).join('');

describe('operate labels are short facts', () => {
	it('words a retry policy beside its Retries key', () => {
		expect(onFailureText({ retries: 2, retryDelaySeconds: 300 })).toBe('2 × 5 min');
		expect(onFailureText({ retries: 1, retryDelaySeconds: 900 })).toBe('1 × 15 min');
		expect(onFailureText({ retries: 3, retryDelaySeconds: 0 })).toBe('3 × immediate');
	});

	it('words queue waits as facts, keeping the scheduler’s wording for the tooltip', () => {
		const write = { kind: 'write_claim', runId: 'run-7f3a', nodeKey: 'model:orders', otherNodes: 2 } as QueueWait;
		expect(text(write)).toBe('Writing model:orders +2 · run 7f3a');
		expect(waitTechnical(write)).toContain('write claim');
		expect(text({ kind: 'concurrency_limit', limit: 'warehouse_heavy', holders: ['run-1'] } as QueueWait)).toBe('warehouse heavy limit full · run 1');
		expect(text({ kind: 'earlier_run', runId: 'run-2', limit: 'warehouse_heavy' } as QueueWait)).toBe('Behind run 2 · warehouse heavy limit');
		expect(text({ kind: 'job_running', runId: 'run-3' } as QueueWait)).toBe('Job running · run 3');
		expect(text({ kind: 'upstream_job', runId: 'run-4', jobKey: 'orders', attempt: null } as QueueWait)).toBe('After orders');
		expect(text({ kind: 'group_parent', runId: 'run-5' } as QueueWait)).toBe('Group run 5 not started');
	});

	it('names policies and sensor kinds in a word or two', () => {
		expect(overlapPlain).toEqual({ queue: 'Queue', skip: 'Skip' });
		expect(triggerKindLabel.job_sensor).toBe('Job sensor');
	});
});

describe('concurrency in plain words', () => {
	it('states a limit of one as one run at a time, with what shares it and the group as detail', () => {
		expect(concurrencyText({ group: 'nightly_window', limit: 1, sharedWith: ['orders_refresh'] })).toEqual({
			rule: 'One run at a time',
			shared: ['orders_refresh'],
			more: 0,
			detail: 'concurrency group nightly_window, limit 1'
		});
	});

	it('states a larger limit as at most so many runs, and names only the first few sharing it', () => {
		const text = concurrencyText({ group: 'warehouse', limit: 3, sharedWith: ['a', 'b', 'c', 'd', 'e'] });
		expect(text.rule).toBe('At most 3 runs at a time');
		expect(text.shared).toEqual(['a', 'b', 'c']);
		expect(text.more).toBe(2);
	});

	it("doesn't imply a limit the snapshot doesn't record", () => {
		const text = concurrencyText({ group: 'warehouse', limit: null, sharedWith: [] });
		expect(text.rule).toBe('Runs in a concurrency group');
		expect(text.detail).toBe('concurrency group warehouse, no limit recorded in this snapshot');
	});
});
