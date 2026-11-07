import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { shortRunId } from '$lib/build-area/labels';
import { checkDetailOf, thresholdsOf, toCheck, toCheckEvidence } from './http-checks';

type W = components['schemas'];

function result(eventId: string, at: string, outcome: string, extra: Partial<W['CheckResult']> = {}): W['CheckResult'] {
	return {
		event_id: eventId,
		binding_key: 'b-orders-row-count',
		audit_name: 'orders_row_count',
		audit_definition_name: 'row_count',
		attachment_kind: 'model',
		attached_target_name: 'orders',
		severity: 'error',
		evaluation_mode: 'measurement',
		run_scope_phase: 'final',
		occurred_at: at,
		run_id: `run-${eventId}`,
		invocation_id: `inv-${eventId}`,
		outcome,
		reused: 'false',
		...extra
	};
}

describe('check mapping', () => {
	it('reads warn and error bounds, keeping both ends of an outside range', () => {
		expect(thresholdsOf('{"error":{"limit":10,"operator":"below"},"warn":{"lower":5,"operator":"outside","upper":500}}')).toEqual([
			{ level: 'warn', operator: 'outside', value: [5, 500] },
			{ level: 'error', operator: 'below', value: 10 }
		]);
		expect(thresholdsOf(null)).toEqual([]);
	});

	it('keeps the sqb run apart from a scheduler run and reads text booleans and numbers', () => {
		const warn = toCheck(result('e1', '2026-11-07T06:00:00Z', 'warn', { measured_value: '12.5', evidence_truncated: 'false' }));
		expect([warn.latest?.runId, warn.latest?.fingerprintRunId, warn.latest?.measuredValue, warn.latest?.evidenceTruncated]).toEqual([null, 'run-e1', 12.5, false]);
		const truncated = toCheck(result('e1', '2026-11-07T06:00:00Z', 'pass', { evidence_truncated: 'true' }));
		expect([truncated.latest?.evidenceTruncated, truncated.kind, truncated.recentOutcomes]).toEqual([true, null, null]);
	});

	it('takes the detail from the newest result and lists recent outcomes oldest first', () => {
		const detail = checkDetailOf([result('e3', '2026-11-07T08:00:00Z', 'error'), result('e2', '2026-11-07T07:00:00Z', 'warn'), result('e1', '2026-11-07T06:00:00Z', 'pass')]);
		expect(detail?.latest?.id).toBe('e3');
		expect(detail?.history.map((e) => e.id)).toEqual(['e3', 'e2', 'e1']);
		expect(detail?.recentOutcomes).toEqual(['pass', 'warn', 'error']);
		expect(checkDetailOf([])).toBeNull();
	});

	it('keeps evidence cells aligned to their columns', () => {
		const evidence = toCheckEvidence({ binding_key: 'b', event_id: 'e1', columns: ['id', 'note'], rows: [[7, null]] as never, truncated: 'true' });
		expect([evidence.rows, evidence.truncated]).toEqual([[[7, null]], true]);
	});

	it('shortens an sqb run id to its random suffix, not its date', () => {
		expect(shortRunId('20261107T061043Z_ea9122fb7f42')).toBe('ea9122fb7f42');
		expect(shortRunId('0f9a3c27-5d61-4e0b-9a88-2c4d7e1b6f30')).toBe('0f9a3c27');
	});
});
