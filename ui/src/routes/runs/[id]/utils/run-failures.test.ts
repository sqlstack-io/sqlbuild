import { describe, expect, it } from 'vitest';
import type { RunEvent, RunNode, RunStatement } from '$lib/api';
import { logEntries, withoutAuditQueries } from './events';
import { failureLine, relationsRead, runFailures, sqlState } from './run-failures';

const MISSING = "002003 (42S02): SQL compilation error:\nObject 'SHOP.RECON.FCT_ORDER_PAYMENTS' does not exist or not authorized.";

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

// As the server lists them: error type and code only; the text is read from the audit's result.
const statements = [statement('audit', 'audit', 'ProgrammingError: 002003')];
const events = [
	event('aa', { statementId: 'audit', message: 'ProgrammingError: 002003' }),
	event('aaa', { type: 'audit_completed', statementId: 'audit', message: 'error', check: { name: 'orders_match_payments', outcome: 'failed', rows: null, bindingKey: 'chk-1', resultId: null } })
];
const unread = { node: () => null, audit: () => null };
const read = { node: () => null, audit: (id: string) => (id === 'aaa' ? MISSING : null) };

describe('a run that failed on an audit after every node built', () => {
	it('leads with the audit, its warehouse code and the cause once its error is read, never the worker’s end line', () => {
		const [failure, ...rest] = runFailures([built], events, statements, 'prod', read);
		expect(rest).toEqual([]);
		expect([failure.kind, failure.code, failure.checkId, failureLine(failure)]).toEqual(['check', '42S02', 'chk-1', 'Failed: audit orders_match_payments after the build · input not built in prod']);
	});

	it('names the failed audit before its error is read, or where it can’t be', () => {
		const [failure] = runFailures([built], events, statements, 'prod', unread);
		expect([failure.message, failure.code, failureLine(failure)]).toEqual([null, null, 'Failed: audit orders_match_payments after the build']);
	});

	it('reads the SQLSTATE and the relations a query reads', () => {
		expect([sqlState(MISSING), sqlState('no code here')]).toEqual(['42S02', null]);
		expect(relationsRead('SELECT 1 FROM "SHOP"."RECON"."FCT_ORDER_PAYMENTS" o LEFT JOIN shop.recon.fct_payment_totals p ON true')).toEqual(['fct_order_payments', 'fct_payment_totals']);
	});

	it('logs the audit’s failed query once, on the audit’s line', () => {
		const rows = withoutAuditQueries(logEntries(events, []));
		expect(rows.map((r) => [r.category, r.level])).toEqual([['checks', 'error']]);
	});
});
