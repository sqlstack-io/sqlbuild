import { describe, expect, it } from 'vitest';
import type { Check, CheckEvaluation } from '$lib/api';
import { ignoredFailuresOf } from './ignored-audits';

const check = (id: string, latest: Partial<CheckEvaluation> | null): Check =>
	({ id, name: id, latest: latest && { id: `ar-${id}`, runId: null, fingerprintRunId: 'other', evaluatedAt: '2026-11-17T00:00:00Z', outcome: 'fail', violationCount: 1, measuredValue: null, evidenceCount: 1, evidenceTruncated: false, ignored: false, ...latest } }) as Check;

describe('a run’s ignored audit failures', () => {
	it('are the checks whose newest result is a failure this run, or a member run, let through', () => {
		const checks = [
			check('by-run-id', { ignored: true, fingerprintRunId: 'run-1' }),
			check('by-scheduler-run', { ignored: true, runId: 'run-2' }),
			check('not-ignored', { fingerprintRunId: 'run-1' }),
			check('another-run', { ignored: true, fingerprintRunId: 'run-9' }),
			check('never-ran', null)
		];
		expect(ignoredFailuresOf(checks, new Set(['run-1', 'run-2'])).map((c) => c.id)).toEqual(['by-run-id', 'by-scheduler-run']);
	});
});
