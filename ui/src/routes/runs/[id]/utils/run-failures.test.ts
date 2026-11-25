import { describe, expect, it } from 'vitest';
import type { RunEvent, RunNode, RunStatement } from '$lib/api';
import { logEntries, markExpectedLookups } from './events';
import { failureLine, relationsRead, runFailures, sqlState } from './run-failures';

const MISSING = "002003 (42S02): SQL compilation error:\nObject 'SHOP.RECON.FCT_ORDER_PAYMENTS' does not exist or not authorized.";
const NO_SCHEMA = "002043 (02000): SQL compilation error:\nSchema 'SHOP.SNAPSHOTS' does not exist or not authorized.";

const event = (id: string, extra: Partial<RunEvent>): RunEvent => ({
	id,
	storageId: 1,
	runId: 'r',
	producer: 'sqb',
	type: 'statement_failed',
	occurredAt: `2026-11-25T08:00:0${id.length}Z`,
	level: 'error',
	invocationId: 'inv',
	nodeKey: null,
	statementId: null,
	attempt: null,
	message: '',
	...extra
});
const statement = (id: string, kind: string, error: string | null): RunStatement => ({
	id,
	runId: 'r',
	invocationId: 'inv',
	sequence: 1,
	nodeKey: '',
	kind,
	status: error ? 'failed' : 'completed',
	startedAt: '2026-11-25T08:00:00Z',
	endedAt: '2026-11-25T08:00:01Z',
	rowsAffected: null,
	rowsSoFar: null,
	queryId: null,
	sqlRecorded: true,
	error
});
const built = { nodeKey: 'country_codes', status: 'success', runId: 'r', kind: 'seed', rowsAffected: 1, skippedBecauseOf: null } as RunNode;

const statements = [statement('lookup', 'inspect', NO_SCHEMA), statement('audit', 'audit', MISSING)];
const events = [
	event('l', { statementId: 'lookup', message: 'Schema does not exist' }),
	event('aa', { statementId: 'audit', message: 'Object does not exist' }),
	event('aaa', { type: 'audit_completed', statementId: 'audit', message: 'orders_match_payments: query failed (severity error)', check: { name: 'orders_match_payments', outcome: 'failed', rows: null, bindingKey: 'chk-1', resultId: null } })
];

describe('a run that failed on a check after every node built', () => {
	it('leads with the check, its warehouse code and the cause, never the worker’s end line', () => {
		const [failure, ...rest] = runFailures([built], events, statements, 'prod');
		expect(rest).toEqual([]);
		expect([failure.kind, failure.code, failure.checkId, failureLine(failure)]).toEqual(['check', '42S02', 'chk-1', 'Failed: check orders_match_payments after the build · input not built in prod']);
	});

	it('reads the SQLSTATE and the relations a query reads', () => {
		expect([sqlState(MISSING), sqlState('no code here')]).toEqual(['42S02', null]);
		expect(relationsRead('SELECT 1 FROM "SHOP"."RECON"."FCT_ORDER_PAYMENTS" o LEFT JOIN shop.recon.fct_payment_totals p ON true')).toEqual(['fct_order_payments', 'fct_payment_totals']);
	});

	it('logs an expected schema lookup as muted info and the check’s failed query once, on the check’s line', () => {
		const rows = markExpectedLookups(logEntries(events, []), statements);
		expect(rows.map((r) => [r.category, r.level, r.expected ?? false])).toEqual([
			['queries', 'info', true],
			['checks', 'error', false]
		]);
		expect(rows[0].text).toBe('Schema not created yet · SHOP.SNAPSHOTS');
	});
});
