// A mock run for the run page's failure mock-up: a job run that built all 93 seeds, then failed one
// end-of-build singular audit that reads two models never built in the target. The warehouse
// inspection also looked up two schemas that don't exist yet, which is expected and not a failure.

import type { RunEvent, RunEventCheck, RunEventLevel, RunEventPhase, RunNode, RunOutputLine, RunStatement } from '../types';

export const SEED_AUDIT_RUN_ID = 'run-pa93';
export const SEED_AUDIT_JOB = 'load_reference';
export const SEED_AUDIT_CHECK = 'orders_match_payments';
export const SEED_AUDIT_CHECK_FILE = 'sql/audits/singular/orders_match_payments.sql';
/** The two models the audit reads; no run builds them in the mock project. */
export const UNBUILT_INPUTS = ['fct_order_payments', 'fct_payment_totals'] as const;
const DATABASE = 'WAFFLE_SHOP';

const DOMAINS = [
	'currency', 'language', 'region', 'timezone', 'payment_method', 'carrier', 'warehouse', 'product_category',
	'return_reason', 'tax_rate', 'shipping_zone', 'sales_channel', 'unit_of_measure', 'loyalty_tier', 'discount_type',
	'store_format', 'supplier_type', 'card_brand', 'refund_reason', 'contact_channel', 'packaging_type', 'holiday', 'fiscal_period'
];
const SUFFIXES = ['codes', 'names', 'mapping', 'aliases'];
/**
 * 92 reference seeds, beside the project's own `country_codes`: 93 in the run. Only where the mock-up is
 * looked at (the dev server, or `?failure-mock`): 92 more seeds change the Lineage overview's layout, which
 * the layout tests check against the usual mock project.
 */
const SHOWN = import.meta.env.DEV || (typeof location !== 'undefined' && new URLSearchParams(location.search).has('failure-mock'));
export const REFERENCE_SEEDS: string[] = SHOWN ? DOMAINS.flatMap((d) => SUFFIXES.map((s) => `${d}_${s}`)) : [];

export const ORDERS_MATCH_PAYMENTS_SQL = `SELECT o.order_id, o.amount_cents, p.amount_cents AS paid_cents
FROM "${DATABASE}"."RECONCILIATION"."FCT_ORDER_PAYMENTS" o
LEFT JOIN "${DATABASE}"."RECONCILIATION"."FCT_PAYMENT_TOTALS" p ON p.order_id = o.order_id
WHERE p.order_id IS NULL OR p.amount_cents <> o.amount_cents`;

export const MISSING_OBJECT_ERROR = `002003 (42S02): SQL compilation error:\nObject '${DATABASE}.RECONCILIATION.FCT_ORDER_PAYMENTS' does not exist or not authorized.`;

type Draft = Omit<RunEvent, 'id' | 'storageId' | 'level' | 'runId' | 'producer'> & { producer?: RunEvent['producer']; level?: RunEventLevel; check?: RunEventCheck };
type Emit = { push: (d: Draft) => void; statements: RunStatement[]; output: RunOutputLine[]; inv: string; runId: string; command: string };

const iso = (ms: number) => new Date(ms).toISOString();

/** What the run's worker did and printed: preparation, 93 seed loads, then the failed end-of-build audit. */
export function seedAuditExecution(start: number, end: number, nodes: readonly RunNode[], { push, statements, output, inv, runId, command }: Emit): void {
	const ev = (type: Draft['type'], at: number, message: string, extra: Partial<Draft> = {}) =>
		push({ type, occurredAt: iso(at), invocationId: inv, nodeKey: null, statementId: null, attempt: null, message, ...extra });
	const out = (stream: RunOutputLine['stream'], at: number, text: string, error = false) => output.push({ invocationId: inv, stream, at: iso(at), text, error });
	const phase = (at: number, p: RunEventPhase) => ev('invocation_phase', at, p.phase, { phase: p });
	const statement = (s: Omit<RunStatement, 'runId' | 'invocationId' | 'sequence' | 'rowsSoFar' | 'sqlRecorded'>) =>
		statements.push({ ...s, runId, invocationId: inv, sequence: statements.length + 1, rowsSoFar: null, sqlRecorded: true });

	ev('invocation_started', start, command);
	out('stdout', start + 100, `sqb ${command.replace(/^sqb /, '')}`);
	phase(start + 900, { phase: 'discovered', durationMs: 780, count: null, name: null, detail: null });
	phase(start + 2_300, { phase: 'compiled', durationMs: 1_400, count: 93, name: null, detail: null });
	phase(start + 2_500, { phase: 'matched', durationMs: 120, count: null, name: 'generation 42', detail: null });
	phase(start + 3_100, { phase: 'connected', durationMs: 560, count: 1, name: 'snowflake', detail: null });
	// Warehouse inspection: two schemas the target hasn't created yet. sqb expects these and carries on.
	const lookups = ['SNAPSHOTS', 'RECONCILIATION'].map((schema, i) => {
		const at = start + 3_300 + i * 180;
		const id = `stmt-pa93-inspect-${i + 1}`;
		const error = `002043 (02000): SQL compilation error:\nSchema '${DATABASE}.${schema}' does not exist or not authorized.`;
		statement({ id, nodeKey: '', kind: 'inspect', status: 'failed', startedAt: iso(at - 90), endedAt: iso(at), rowsAffected: null, queryId: `01b8e2a0-0000-4c1f-0000-00a93${i}`, error });
		ev('statement_failed', at, error.split('\n').pop()!, { statementId: id });
		return schema;
	});
	phase(start + 3_900, { phase: 'inspected', durationMs: 780, count: null, name: null, detail: `${lookups.length} schemas not created yet` });
	phase(start + 4_100, { phase: 'planned', durationMs: 190, count: 93, name: null, detail: null });
	phase(start + 4_200, { phase: 'started_execution', durationMs: null, count: 8, name: null, detail: null });
	out('stdout', start + 4_200, '✓ Project compiled  93 seeds  1.4s');
	out('stdout', start + 4_250, `Building 93 nodes, 8 at a time`);

	for (const n of nodes) {
		if (!n.startedAt || !n.endedAt) continue;
		const s = Date.parse(n.startedAt);
		const e = Date.parse(n.endedAt);
		const id = `stmt-pa93-${n.nodeKey}`;
		ev('resource_attempt_started', s, `Started seed ${n.nodeKey}`, { nodeKey: n.nodeKey, attempt: 1 });
		statement({ id, nodeKey: n.nodeKey, kind: 'load_seed', status: 'completed', startedAt: iso(s + 40), endedAt: iso(e - 60), rowsAffected: n.rowsAffected, error: null, queryId: `01b8e2a1-0000-4c1f-0000-${n.nodeKey.length.toString(16).padStart(4, '0')}` });
		ev('statement_completed', e - 60, `${(n.rowsAffected ?? 0).toLocaleString('en-GB')} rows`, { nodeKey: n.nodeKey, statementId: id, attempt: 1 });
		ev('resource_attempt_completed', e, `Completed in ${((e - s) / 1000).toFixed(1)}s`, { nodeKey: n.nodeKey, attempt: 1 });
		out('stdout', e, `  ✓ ${n.nodeKey}  seed  ${((e - s) / 1000).toFixed(1)}s`);
	}

	// The end-of-build singular audit: its query can't run, as the models it reads were never built here.
	const auditAt = end - 2_400;
	const auditId = 'stmt-pa93-audit-orders_match_payments';
	statement({ id: auditId, nodeKey: '', kind: 'audit', status: 'failed', startedAt: iso(auditAt - 350), endedAt: iso(auditAt), rowsAffected: null, queryId: '01b8e2a3-0000-4c1f-0000-00000000a93e', error: MISSING_OBJECT_ERROR });
	ev('statement_failed', auditAt, MISSING_OBJECT_ERROR.split('\n').pop()!, { statementId: auditId });
	ev('audit_completed', auditAt + 20, `${SEED_AUDIT_CHECK}: query failed (severity error)`, {
		statementId: auditId,
		level: 'error',
		check: { name: SEED_AUDIT_CHECK, outcome: 'failed', rows: null, bindingKey: 'chk-end-orders-payments', resultId: null }
	});
	out('stderr', auditAt + 20, `  ✗ check ${SEED_AUDIT_CHECK}  ${MISSING_OBJECT_ERROR.replace('\n', ' ')}`, true);
	ev('invocation_failed', end - 400, '93 built · 0 failed · 0 skipped · 1 check failed');
	out('stderr', end - 400, '✗ Completed with errors  93 built · 1 check failed', true);
	// The worker's own housekeeping line, which today's page shows as the run's reason.
	out('stdout', end - 200, 'worker executor completed and closed its connections');
}

/** The SQL of the mock run's own statements: the audit's query and the inspection's schema lookups. */
export function seedAuditSql(st: RunStatement): string | null {
	if (st.runId !== SEED_AUDIT_RUN_ID) return null;
	if (st.kind === 'audit') return ORDERS_MATCH_PAYMENTS_SQL;
	if (st.kind === 'inspect') return `SHOW TERSE OBJECTS IN SCHEMA ${DATABASE}.${st.error?.match(/Schema '[^.]+\.(\w+)'/)?.[1] ?? ''}`;
	if (st.kind === 'load_seed') return `COPY INTO ${DATABASE}.REFERENCE.${st.nodeKey.toUpperCase()} FROM @sqlbuild_seeds/${st.nodeKey}.csv`;
	return null;
}
