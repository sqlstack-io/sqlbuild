import { describe, expect, it } from 'vitest';
import { describeWait, onFailureText, overlapPlain, poolText, runConcurrencyHelp, triggerKindLabel, waitTechnical } from './labels';
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
		expect(text({ kind: 'pool_full', pool: 'warehouse_heavy', holders: ['run-1'] } as QueueWait)).toBe('warehouse_heavy pool full · run 1');
		expect(waitTechnical({ kind: 'pool_full', pool: 'warehouse_heavy', holders: ['run-1'] } as QueueWait)).toBe('pool warehouse_heavy full, slots held by run-1');
		expect(text({ kind: 'earlier_run', runId: 'run-2', pool: 'warehouse_heavy' } as QueueWait)).toBe('Behind run 2 · warehouse_heavy pool');
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
	const use = { pool: 'supplier_api', amount: 1, limit: 2, cap: null, serverLimit: 2, clamped: false, sharedWith: ['orders_refresh'] };

	it('states a pool the job takes against the limit on this server, with what shares it', () => {
		expect(poolText(use)).toEqual({ text: 'supplier_api: takes 1 of 2', detail: null, clamped: false, shared: ['orders_refresh'], more: 0 });
	});

	it('names only the first few jobs sharing a pool', () => {
		const text = poolText({ ...use, sharedWith: ['a', 'b', 'c', 'd', 'e'] });
		expect(text.shared).toEqual(['a', 'b', 'c']);
		expect(text.more).toBe(2);
	});

	it('says when this server lowered the pool, and takes the whole pool when the job asks for more', () => {
		expect(poolText({ ...use, limit: 3, cap: 2 }).detail).toBe('capped on this server; the project allows 3');
		const clamped = poolText({ pool: 'warehouse_heavy', amount: 3, limit: 3, cap: 1, serverLimit: 1, clamped: true, sharedWith: [] });
		expect(clamped.clamped).toBe(true);
		expect(`${clamped.text} (${clamped.detail})`).toBe('Takes the whole warehouse_heavy pool on this server (capped at 1; the project asks for 3)');
	});

	it("explains a run's node limit for a job and for a group's members", () => {
		expect(runConcurrencyHelp('job')).toBe("At most this many of the job's nodes run together in one run.");
		expect(runConcurrencyHelp('group')).toBe('Members without their own limit run at most this many nodes together.');
	});

	it('names what a job triggered while it runs does', () => {
		expect(overlapPlain).toEqual({ queue: 'Queue', skip: 'Skip' });
	});
});

