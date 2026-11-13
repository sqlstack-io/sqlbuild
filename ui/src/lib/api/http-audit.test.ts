import { describe, expect, it } from 'vitest';
import { auditMatches, toAuditEntry } from './http-audit';

const base = {
	storage_id: 7,
	event_id: 'e-1',
	occurred_at: '2026-11-08T10:00:00Z',
	request_id: 'r-1',
	ip_address: '127.0.0.1',
	before_json: null
};

describe('toAuditEntry', () => {
	it('lists a denial under the refused action with its decision and reason', () => {
		const entry = toAuditEntry({
			...base,
			actor: 'principal:p-2',
			actor_name: 'orders-viewer',
			action: 'authorization.denied',
			granting_role: 'none',
			scope_json: { action: 'run.cancel', location_id: 'loc-1', target: 'prod', nodes: [] },
			after_json: { decision_id: 'd-9', allowed: false, reason: 'missing permission' }
		});
		expect(entry).toMatchObject({
			actor: { kind: 'user', name: 'orders-viewer' },
			action: 'run.cancel',
			outcome: 'denied',
			grantingRole: null,
			decisionId: 'd-9',
			summary: 'missing permission',
			after: null,
			scope: { locationId: 'loc-1:prod', subject: null }
		});
	});

	it('keeps an allowed change with its role, values and remaining scope', () => {
		const entry = toAuditEntry({
			...base,
			actor: 'scheduler/s-1',
			action: 'token.issued',
			granting_role: 'scheduler',
			scope_json: { kind: 'run', run_id: 'run-4' },
			after_json: { token_id: 't-1' }
		});
		expect(entry).toMatchObject({
			actor: { kind: 'system', name: 'scheduler/s-1' },
			action: 'token.issued',
			outcome: 'allowed',
			grantingRole: 'scheduler',
			decisionId: null,
			after: '{"token_id":"t-1"}',
			scope: { locationId: null, subject: 'run_id=run-4' }
		});
		expect(entry.summary).toBe('');
		expect(auditMatches(entry, { outcome: 'denied' })).toBe(false);
		expect(auditMatches(entry, { actor: 'scheduler/s-1', action: 'token.issued' })).toBe(true);
	});

	it('says what work a location was asked for, and which run a read was of', () => {
		const allowed = (action: string, after_json: Record<string, unknown>) =>
			toAuditEntry({ ...base, actor: 'principal:p-1', actor_name: 'ana', action, granting_role: 'admin', scope_json: { location_id: 'loc-1', target: 'prod' }, after_json }).summary;
		const work = (work: Record<string, unknown>) => allowed('location.work_requested', { request_id: 'q-1', reason: 'plan', work });
		expect(work({ kind: 'plan', command: ['build', '--select', 'orders'], selection: ['model:orders'], settings: {} })).toBe('Plan sqb build --select orders');
		expect(work({ kind: 'sync_warehouse' })).toBe('Sync the warehouse copy');
		expect(allowed('error.read', { run_id: 'd36ac8b2-314c-46aa', found: true })).toBe('Run d36ac8b2');
		expect(allowed('output.read', { run_id: 'd36ac8b2-314c-46aa', found: false })).toBe('Run d36ac8b2, not found');
	});
});
