// Neutral example data for the BUILD area: the shop project's node metadata,
// columns and column lineage, its audits, and the snapshot change feed. Build
// state itself (last builds, outcomes, staleness) is derived in mock/build.ts
// from the OPERATE run history plus a few builds made outside the scheduler.

import type {
	CheckAttachment,
	CheckKind,
	CheckOutcome,
	CheckSeverity,
	CheckThreshold,
	ColumnLens,
	EnvironmentChange,
	EvaluationMode,
	NodeChange,
	OrchestrationChange,
	Principal,
	RoleChange,
	RunScopePhase,
	SourceFreshness
} from '../types';
import { ALEX, CI_ACCOUNT, DEMO_USER, JORDAN, RILEY, SAM } from './principals';

const MIN = 60_000;
const DAY = 1440;

// ---- node metadata (graph and kinds come from mock/operate-graph.ts) ----

export type ColumnSpec = { name: string; type: string; description: string; audits?: string[]; expr?: string };

export type NodeSpec = {
	materialization: string | null;
	owner: string[];
	description: string;
	schema: string | null;
	columns: ColumnSpec[];
	/** FROM clause for nodes that read more than one parent. */
	from?: string;
	where?: string;
	groupBy?: boolean;
	/** Microbatch models: cursor column and batch size. */
	microbatch?: { cursorColumn: string; batchSize: string };
};

const c = (name: string, type: string, description: string, audits: string[] = [], expr?: string): ColumnSpec => ({ name, type, description, audits, expr });

export const NODE_SPECS: Record<string, NodeSpec> = {
	raw__customers: {
		materialization: null,
		owner: ['platform'],
		description: 'Customer records replicated from the shop database.',
		schema: 'RAW',
		columns: [c('id', 'bigint', 'Customer ID'), c('first_name', 'varchar', 'Given name'), c('last_name', 'varchar', 'Family name'), c('email', 'varchar', 'Contact email'), c('created_at', 'timestamp', 'Sign-up time (UTC)')]
	},
	raw__payments: {
		materialization: null,
		owner: ['platform'],
		description: 'Payment events replicated from the payments service.',
		schema: 'RAW',
		columns: [c('id', 'bigint', 'Payment ID'), c('order_id', 'bigint', 'Order paid for'), c('payment_method', 'varchar', 'card | cash | voucher', ['accepted_values']), c('amount_cents', 'integer', 'Amount in cents'), c('status', 'varchar', 'success | failed'), c('created_at', 'timestamp', 'Payment time (UTC)')]
	},
	raw__orders: {
		materialization: 'loader · delete_insert',
		owner: ['ingest'],
		description: 'Partner order files, loaded incrementally by the order loader.',
		schema: 'RAW',
		columns: [c('id', 'bigint', 'Order ID'), c('user_id', 'bigint', 'Customer ID'), c('order_date', 'timestamp', 'Order time (UTC)'), c('status', 'varchar', 'Order status'), c('total_cents', 'integer', 'Order total in cents'), c('_loaded_at', 'timestamp', 'Load time')]
	},
	country_codes: {
		materialization: 'seed',
		owner: ['analytics'],
		description: 'ISO country codes and sales regions.',
		schema: 'STAGING',
		columns: [c('code', 'varchar', 'ISO 3166 alpha-2 code', ['unique']), c('name', 'varchar', 'Country name'), c('region', 'varchar', 'Sales region')]
	},
	stg_customers: {
		materialization: 'view',
		owner: ['analytics'],
		description: 'Cleaned customers from the raw replica.',
		schema: 'STAGING',
		columns: [c('customer_id', 'bigint', 'Customer ID', ['not_null'], 'id AS customer_id'), c('first_name', 'varchar', 'Given name'), c('last_name', 'varchar', 'Family name'), c('email_domain', 'varchar', 'Domain of the contact email', [], "split_part(email, '@', 2) AS email_domain"), c('country_code', 'varchar', 'ISO country code', [], "'GB' AS country_code"), c('first_order_at', 'timestamp', 'Sign-up time (UTC)', [], 'created_at AS first_order_at')]
	},
	stg_orders: {
		materialization: 'view',
		owner: ['analytics'],
		description: 'Cleaned orders from the order loader.',
		schema: 'STAGING',
		columns: [c('order_id', 'bigint', 'Order ID', ['not_null'], 'id AS order_id'), c('customer_id', 'bigint', 'Customer ID', [], 'user_id AS customer_id'), c('order_status', 'varchar', 'completed | pending | cancelled', ['accepted_values'], 'status AS order_status'), c('total_cents', 'integer', 'Order total in cents'), c('ordered_at', 'timestamp', 'Order time (UTC)', [], 'order_date AS ordered_at')]
	},
	stg_payments: {
		materialization: 'view',
		owner: ['analytics'],
		description: 'Cleaned payments from the payments replica.',
		schema: 'STAGING',
		columns: [c('payment_id', 'bigint', 'Payment ID', ['not_null'], 'id AS payment_id'), c('order_id', 'bigint', 'Order paid for'), c('payment_method', 'varchar', 'card | cash | voucher'), c('amount_cents', 'integer', 'Amount in cents'), c('status', 'varchar', 'success | failed')]
	},
	fact_orders: {
		materialization: 'table',
		owner: ['analytics', 'finance-data'],
		description: 'One row per order with payment and customer attributes.',
		schema: 'MARTS',
		columns: [
			c('order_id', 'bigint', 'Primary key, one row per order', ['not_null', 'unique'], 'o.order_id'),
			c('customer_id', 'bigint', 'Customer who placed the order', ['relationships'], 'o.customer_id'),
			c('order_status', 'varchar', 'completed | pending | cancelled', [], 'o.order_status'),
			c('total_cents', 'integer', 'Order total in cents', [], 'o.total_cents'),
			c('payment_method', 'varchar', 'card | cash | voucher', [], 'p.payment_method'),
			c('ordered_at', 'timestamp', 'Order time (UTC)', [], 'o.ordered_at'),
			c('is_first_order', 'boolean', 'First order for this customer', [], 'o.ordered_at = c.first_order_at AS is_first_order')
		]
	},
	dim_customers: {
		materialization: 'table',
		owner: ['analytics'],
		description: 'Customer dimension with first and last order and lifetime value.',
		schema: 'MARTS',
		from: '__ref("stg_customers")\nLEFT JOIN __ref("fact_orders") USING (customer_id)\nLEFT JOIN __ref("country_codes") ON country_code = code',
		groupBy: true,
		columns: [
			c('customer_id', 'bigint', 'Primary key', ['unique']),
			c('full_name', 'varchar', 'Given and family name', [], "first_name || ' ' || last_name AS full_name"),
			c('country', 'varchar', 'Country name', [], 'name AS country'),
			c('first_order_at', 'timestamp', 'First order (UTC)'),
			c('last_order_at', 'timestamp', 'Latest order (UTC)', [], 'max(ordered_at) AS last_order_at'),
			c('order_count', 'bigint', 'Orders placed', [], 'count(order_id) AS order_count'),
			c('lifetime_value', 'decimal(18,2)', 'Total spend in dollars', [], 'cents_to_dollars(sum(total_cents)) AS lifetime_value')
		]
	},
	daily_revenue: {
		materialization: 'table',
		owner: ['finance-data'],
		description: 'Daily revenue from completed orders.',
		schema: 'MARTS',
		where: "order_status = 'completed'",
		groupBy: true,
		columns: [c('revenue_date', 'date', 'Order date', ['unique'], "date_trunc('day', ordered_at) AS revenue_date"), c('total_revenue_cents', 'bigint', 'Revenue in cents', ['expression_is_true'], 'sum(total_cents) AS total_revenue_cents'), c('order_count', 'bigint', 'Completed orders', [], 'count(order_id) AS order_count')]
	},
	customer_status: {
		materialization: 'incremental · merge',
		owner: ['growth'],
		description: 'Active or lapsed status per customer, merged on change.',
		schema: 'MARTS',
		columns: [c('customer_id', 'bigint', 'Customer ID'), c('status', 'varchar', 'active | lapsed', ['not_null'], "CASE WHEN last_order_at > now() - INTERVAL 90 DAY THEN 'active' ELSE 'lapsed' END AS status"), c('status_changed_at', 'timestamp', 'When the status last changed', [], 'last_order_at AS status_changed_at')]
	},
	hourly_order_activity: {
		materialization: 'incremental · microbatch',
		owner: ['analytics'],
		description: 'Orders and revenue per hour, built in hourly microbatches.',
		schema: 'ANALYTICS',
		groupBy: true,
		microbatch: { cursorColumn: 'activity_hour', batchSize: '1h' },
		columns: [c('activity_hour', 'timestamp', 'Hour (UTC)', [], "date_trunc('hour', ordered_at) AS activity_hour"), c('order_count', 'bigint', 'Orders in the hour', [], 'count(order_id) AS order_count'), c('revenue_cents', 'bigint', 'Revenue in the hour', [], 'sum(total_cents) AS revenue_cents')]
	},
	hourly_activity_ctx: {
		materialization: 'incremental · microbatch',
		owner: ['analytics'],
		description: 'Hourly activity with the day’s revenue for context.',
		schema: 'ANALYTICS',
		from: `__ref("hourly_order_activity")\nJOIN __ref("daily_revenue") ON date_trunc('day', activity_hour) = revenue_date`,
		microbatch: { cursorColumn: 'activity_hour', batchSize: '1h' },
		columns: [c('activity_hour', 'timestamp', 'Hour (UTC)', ['expression_is_true']), c('order_count', 'bigint', 'Orders in the hour'), c('revenue_cents', 'bigint', 'Revenue in the hour'), c('day_revenue_cents', 'bigint', 'Revenue for the whole day', [], 'total_revenue_cents AS day_revenue_cents'), c('share_of_day', 'double', 'Share of the day’s revenue', [], 'revenue_cents / nullif(total_revenue_cents, 0) AS share_of_day')]
	},
	daily_activity_rollup: {
		materialization: 'incremental · microbatch',
		owner: ['analytics'],
		description: 'Daily rollup of hourly activity with the peak hour.',
		schema: 'ANALYTICS',
		groupBy: true,
		microbatch: { cursorColumn: 'activity_date', batchSize: '1d' },
		columns: [c('activity_date', 'date', 'Day (UTC)', [], "date_trunc('day', activity_hour) AS activity_date"), c('order_count', 'bigint', 'Orders in the day', [], 'sum(order_count) AS order_count'), c('peak_hour', 'timestamp', 'Busiest hour', [], 'arg_max(activity_hour, order_count) AS peak_hour')]
	},
	revenue_report: {
		materialization: 'table',
		owner: ['finance-data'],
		description: 'Revenue report for the finance dashboard.',
		schema: 'REPORTING',
		columns: [c('revenue_date', 'date', 'Order date'), c('total_revenue', 'decimal(18,2)', 'Revenue in dollars', [], 'cents_to_dollars(total_revenue_cents) AS total_revenue'), c('order_count', 'bigint', 'Completed orders'), c('avg_order_value', 'decimal(18,2)', 'Average order value in dollars', [], 'cents_to_dollars(total_revenue_cents / nullif(order_count, 0)) AS avg_order_value')]
	},
	activity_report: {
		materialization: 'view',
		owner: ['analytics'],
		description: 'Hourly activity for the operations dashboard.',
		schema: 'REPORTING',
		columns: [c('activity_hour', 'timestamp', 'Hour (UTC)'), c('order_count', 'bigint', 'Orders in the hour'), c('share_of_day', 'double', 'Share of the day’s revenue, in percent', [], 'pct(share_of_day) AS share_of_day')]
	},
	export_revenue_csv: {
		materialization: null,
		owner: ['finance-data'],
		description: 'Exports the revenue report as CSV to the finance file drop.',
		schema: null,
		columns: []
	}
};

export const PYTHON_SOURCE: Record<string, { file: string; source: string }> = {
	raw__orders: {
		file: 'tasks/raw__orders.py',
		source: `from sqlbuild.loaders import loader
from sqlbuild.executor.load.models import LoaderContext

from shop.partner_files import fetch_since


@loader(cursor="order_date", strategy="delete_insert")
def raw__orders(ctx: LoaderContext):
    """Partner order files, loaded incrementally by order date."""
    return fetch_since(ctx.current_cursor_value)`
	},
	export_revenue_csv: {
		file: 'tasks/export_revenue_csv.py',
		source: `from sqlbuild.tasks import task

from shop.file_drop import upload_csv


@task(upstream=["revenue_report"])
def export_revenue_csv(ctx):
    """Exports the revenue report as CSV to the finance file drop."""
    rows = ctx.query('SELECT * FROM __ref("revenue_report") ORDER BY revenue_date')
    upload_csv("finance/revenue.csv", rows)`
	}
};

/** `model.column` → `model.column` edges from the snapshot's column lineage. */
export const COLUMN_EDGES: [string, string, ColumnLens][] = [
	['raw__customers.id', 'stg_customers.customer_id', 'rename'],
	['raw__customers.first_name', 'stg_customers.first_name', 'passthrough'],
	['raw__customers.last_name', 'stg_customers.last_name', 'passthrough'],
	['raw__customers.email', 'stg_customers.email_domain', 'transform'],
	['raw__customers.created_at', 'stg_customers.first_order_at', 'rename'],
	['raw__orders.id', 'stg_orders.order_id', 'rename'],
	['raw__orders.user_id', 'stg_orders.customer_id', 'rename'],
	['raw__orders.status', 'stg_orders.order_status', 'rename'],
	['raw__orders.total_cents', 'stg_orders.total_cents', 'passthrough'],
	['raw__orders.order_date', 'stg_orders.ordered_at', 'rename'],
	['raw__payments.id', 'stg_payments.payment_id', 'rename'],
	['raw__payments.order_id', 'stg_payments.order_id', 'passthrough'],
	['raw__payments.payment_method', 'stg_payments.payment_method', 'passthrough'],
	['raw__payments.amount_cents', 'stg_payments.amount_cents', 'passthrough'],
	['raw__payments.status', 'stg_payments.status', 'passthrough'],
	['stg_orders.order_id', 'fact_orders.order_id', 'passthrough'],
	['stg_orders.customer_id', 'fact_orders.customer_id', 'passthrough'],
	['stg_orders.order_status', 'fact_orders.order_status', 'passthrough'],
	['stg_orders.total_cents', 'fact_orders.total_cents', 'passthrough'],
	['stg_payments.payment_method', 'fact_orders.payment_method', 'passthrough'],
	['stg_orders.ordered_at', 'fact_orders.ordered_at', 'passthrough'],
	['stg_orders.ordered_at', 'fact_orders.is_first_order', 'transform'],
	['stg_customers.first_order_at', 'fact_orders.is_first_order', 'transform'],
	['stg_customers.customer_id', 'dim_customers.customer_id', 'passthrough'],
	['stg_customers.first_name', 'dim_customers.full_name', 'transform'],
	['stg_customers.last_name', 'dim_customers.full_name', 'transform'],
	['country_codes.name', 'dim_customers.country', 'rename'],
	['stg_customers.first_order_at', 'dim_customers.first_order_at', 'passthrough'],
	['fact_orders.ordered_at', 'dim_customers.last_order_at', 'transform'],
	['fact_orders.order_id', 'dim_customers.order_count', 'transform'],
	['fact_orders.total_cents', 'dim_customers.lifetime_value', 'transform'],
	['fact_orders.ordered_at', 'daily_revenue.revenue_date', 'transform'],
	['fact_orders.total_cents', 'daily_revenue.total_revenue_cents', 'transform'],
	['fact_orders.order_id', 'daily_revenue.order_count', 'transform'],
	['dim_customers.customer_id', 'customer_status.customer_id', 'passthrough'],
	['dim_customers.last_order_at', 'customer_status.status', 'transform'],
	['dim_customers.last_order_at', 'customer_status.status_changed_at', 'rename'],
	['fact_orders.ordered_at', 'hourly_order_activity.activity_hour', 'transform'],
	['fact_orders.order_id', 'hourly_order_activity.order_count', 'transform'],
	['fact_orders.total_cents', 'hourly_order_activity.revenue_cents', 'transform'],
	['hourly_order_activity.activity_hour', 'hourly_activity_ctx.activity_hour', 'passthrough'],
	['hourly_order_activity.order_count', 'hourly_activity_ctx.order_count', 'passthrough'],
	['hourly_order_activity.revenue_cents', 'hourly_activity_ctx.revenue_cents', 'passthrough'],
	['daily_revenue.total_revenue_cents', 'hourly_activity_ctx.day_revenue_cents', 'rename'],
	['hourly_order_activity.revenue_cents', 'hourly_activity_ctx.share_of_day', 'transform'],
	['daily_revenue.total_revenue_cents', 'hourly_activity_ctx.share_of_day', 'transform'],
	['hourly_activity_ctx.activity_hour', 'daily_activity_rollup.activity_date', 'transform'],
	['hourly_activity_ctx.order_count', 'daily_activity_rollup.order_count', 'transform'],
	['hourly_activity_ctx.activity_hour', 'daily_activity_rollup.peak_hour', 'transform'],
	['daily_revenue.revenue_date', 'revenue_report.revenue_date', 'passthrough'],
	['daily_revenue.total_revenue_cents', 'revenue_report.total_revenue', 'transform'],
	['daily_revenue.order_count', 'revenue_report.order_count', 'passthrough'],
	['daily_revenue.total_revenue_cents', 'revenue_report.avg_order_value', 'transform'],
	['daily_revenue.order_count', 'revenue_report.avg_order_value', 'transform'],
	['hourly_activity_ctx.activity_hour', 'activity_report.activity_hour', 'passthrough'],
	['hourly_activity_ctx.order_count', 'activity_report.order_count', 'passthrough'],
	['hourly_activity_ctx.share_of_day', 'activity_report.share_of_day', 'transform']
];

// ---- builds outside the scheduler (CLI, CI), from synced fingerprints ----

export type ExternalBuild = { node: string; fingerprintRunId: string; minutesAgo: number; outcome: 'success' | 'warn'; rows: number | null; durationMs: number };

export const EXTERNAL_BUILDS: ExternalBuild[] = [
	{ node: 'raw__orders', fingerprintRunId: '0f9a3c27-5d61-4e0b-9a88-2c4d7e1b6f30', minutesAgo: 95, outcome: 'success', rows: 1_312, durationMs: 41_000 },
	{ node: 'country_codes', fingerprintRunId: '6c2e8b14-07fa-4d39-b5c1-9e3a2f7d4b85', minutesAgo: 9 * DAY, outcome: 'success', rows: 249, durationMs: 1_800 },
	{ node: 'revenue_report', fingerprintRunId: '3b7d1e90-c4a2-4f68-8e15-a0d9c6b2f741', minutesAgo: 2 * DAY + 50, outcome: 'success', rows: 365, durationMs: 2_900 },
	{ node: 'dim_customers', fingerprintRunId: 'a41f6d08-2b93-4c7e-b0d5-7f1e9c3a6b22', minutesAgo: 3 * DAY, outcome: 'success', rows: 8_410, durationMs: 6_100 },
	{ node: 'activity_report', fingerprintRunId: 'e8c05b3a-91d7-4a26-8f4e-b6a2d0c7e913', minutesAgo: 4 * DAY, outcome: 'success', rows: null, durationMs: 700 }
];

// ---- source freshness (`_sqlbuild_source_freshness`) ----

export function sourceFreshness(now: number): Record<string, SourceFreshness> {
	const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();
	return {
		raw__customers: { status: 'pass', loadedAt: ago(140), checkedAt: ago(2), warnAfterSeconds: 6 * 3600, errorAfterSeconds: 24 * 3600 },
		raw__payments: { status: 'warn', loadedAt: ago(34), checkedAt: ago(2), warnAfterSeconds: 1800, errorAfterSeconds: 2 * 3600 }
	};
}

// ---- audits and checks (`_sqlbuild_audit_results`) ----

export type CheckSpec = {
	id: string;
	name: string;
	definition: string | null;
	kind: CheckKind;
	attachment: CheckAttachment;
	node: string | null;
	column: string | null;
	severity: CheckSeverity;
	runScopePhase: RunScopePhase;
	evaluationMode: EvaluationMode;
	thresholds: CheckThreshold[];
	description: string | null;
	/** The latest outcome, then the shape of the older history. */
	latest: { outcome: CheckOutcome; violations?: number; measured?: number };
	history: { failEvery?: number; warnEvery?: number; measuredBase?: number };
	/** A failed build of the node failed in this audit's query (an execution error), not before audits ran. */
	errorOnFailure?: boolean;
	/** Compiled audit query, with `{relation}` for the attached object. */
	sql: string;
	evidenceColumns?: string[];
};

export const CHECK_SPECS: CheckSpec[] = [
	{ id: 'chk-fo-not-null', name: 'not_null(order_id)', definition: 'not_null', kind: 'audit', attachment: 'model', node: 'fact_orders', column: 'order_id', severity: 'error', runScopePhase: 'delta_and_final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: {}, sql: 'SELECT order_id FROM {relation} WHERE order_id IS NULL' },
	{ id: 'chk-fo-unique', name: 'unique(order_id)', definition: 'unique', kind: 'audit', attachment: 'model', node: 'fact_orders', column: 'order_id', severity: 'error', runScopePhase: 'delta_and_final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: { failEvery: 17 }, sql: 'SELECT order_id, count(*) AS n FROM {relation} GROUP BY order_id HAVING count(*) > 1', evidenceColumns: ['order_id', 'n'] },
	{ id: 'chk-fo-rel', name: 'relationships(customer_id → dim_customers)', definition: 'relationships', kind: 'audit', attachment: 'model', node: 'fact_orders', column: 'customer_id', severity: 'warn', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: 'Every order belongs to a known customer.', latest: { outcome: 'pass', violations: 0 }, history: { warnEvery: 9 }, sql: 'SELECT f.customer_id FROM {relation} f LEFT JOIN "WAFFLE_SHOP"."MARTS"."DIM_CUSTOMERS" d USING (customer_id) WHERE d.customer_id IS NULL', evidenceColumns: ['customer_id'] },
	{ id: 'chk-sc-not-null', name: 'not_null(customer_id)', definition: 'not_null', kind: 'audit', attachment: 'model', node: 'stg_customers', column: 'customer_id', severity: 'error', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: {}, sql: 'SELECT customer_id FROM {relation} WHERE customer_id IS NULL' },
	{ id: 'chk-sp-not-null', name: 'not_null(payment_id)', definition: 'not_null', kind: 'audit', attachment: 'model', node: 'stg_payments', column: 'payment_id', severity: 'warn', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'warn', violations: 2 }, history: { warnEvery: 3 }, sql: 'SELECT order_id, payment_method, amount_cents FROM {relation} WHERE payment_id IS NULL', evidenceColumns: ['order_id', 'payment_method', 'amount_cents'] },
	{ id: 'chk-so-values', name: 'accepted_values(order_status)', definition: 'accepted_values', kind: 'audit', attachment: 'model', node: 'stg_orders', column: 'order_status', severity: 'error', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: {}, sql: "SELECT order_status, count(*) AS n FROM {relation} WHERE order_status NOT IN ('completed', 'pending', 'cancelled') GROUP BY 1" },
	{ id: 'chk-dc-unique', name: 'unique(customer_id)', definition: 'unique', kind: 'audit', attachment: 'model', node: 'dim_customers', column: 'customer_id', severity: 'error', runScopePhase: 'delta_and_final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: {}, sql: 'SELECT customer_id, count(*) AS n FROM {relation} GROUP BY customer_id HAVING count(*) > 1' },
	{ id: 'chk-dr-positive', name: 'expression_is_true(total_revenue_cents >= 0)', definition: 'expression_is_true', kind: 'audit', attachment: 'model', node: 'daily_revenue', column: 'total_revenue_cents', severity: 'error', runScopePhase: 'delta_and_final', evaluationMode: 'violations', thresholds: [], description: 'Revenue is never negative.', latest: { outcome: 'pass', violations: 0 }, history: {}, errorOnFailure: true, sql: 'SELECT revenue_date, total_revenue_cents FROM {relation} WHERE NOT (total_revenue_cents >= 0)', evidenceColumns: ['revenue_date', 'total_revenue_cents'] },
	{ id: 'chk-dr-unique', name: 'unique(revenue_date)', definition: 'unique', kind: 'audit', attachment: 'model', node: 'daily_revenue', column: 'revenue_date', severity: 'error', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: {}, sql: 'SELECT revenue_date, count(*) AS n FROM {relation} GROUP BY 1 HAVING count(*) > 1' },
	{ id: 'chk-hoa-rows', name: 'row_count(activity_hour)', definition: 'row_count', kind: 'audit', attachment: 'model', node: 'hourly_order_activity', column: null, severity: 'warn', runScopePhase: 'delta_and_final', evaluationMode: 'measurement', thresholds: [{ level: 'warn', operator: 'below', value: 500 }, { level: 'error', operator: 'below', value: 100 }], description: 'Each batch should hold a normal hour of orders.', latest: { outcome: 'warn', measured: 412 }, history: { measuredBase: 640, warnEvery: 6 }, sql: 'SELECT count(*) AS measured_value FROM {relation} WHERE activity_hour >= :batch_start AND activity_hour < :batch_end' },
	{ id: 'chk-hac-hour', name: 'expression_is_true(activity_hour IS NOT NULL)', definition: 'expression_is_true', kind: 'audit', attachment: 'model', node: 'hourly_activity_ctx', column: 'activity_hour', severity: 'error', runScopePhase: 'delta_and_final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: { failEvery: 23 }, sql: 'SELECT * FROM {relation} WHERE NOT (activity_hour IS NOT NULL)' },
	{ id: 'chk-cs-not-null', name: 'not_null(status)', definition: 'not_null', kind: 'audit', attachment: 'model', node: 'customer_status', column: 'status', severity: 'error', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: {}, sql: 'SELECT customer_id FROM {relation} WHERE status IS NULL' },
	{ id: 'chk-rp-values', name: 'accepted_values(payment_method)', definition: 'accepted_values', kind: 'audit', attachment: 'source', node: 'raw__payments', column: 'payment_method', severity: 'warn', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: { warnEvery: 11 }, sql: "SELECT payment_method, count(*) AS n FROM {relation} WHERE payment_method NOT IN ('card', 'cash', 'voucher') GROUP BY 1", evidenceColumns: ['payment_method', 'n'] },
	{ id: 'chk-cc-unique', name: 'unique(code)', definition: 'unique', kind: 'audit', attachment: 'seed', node: 'country_codes', column: 'code', severity: 'error', runScopePhase: 'final', evaluationMode: 'violations', thresholds: [], description: null, latest: { outcome: 'pass', violations: 0 }, history: {}, sql: 'SELECT code, count(*) AS n FROM {relation} GROUP BY 1 HAVING count(*) > 1' },
	{ id: 'chk-end-reconcile', name: 'revenue_reconciles', definition: null, kind: 'sql_test', attachment: 'end', node: null, column: null, severity: 'error', runScopePhase: 'final', evaluationMode: 'measurement', thresholds: [{ level: 'warn', operator: 'above', value: 0.05 }, { level: 'error', operator: 'above', value: 0.1 }], description: 'Mart revenue matches successful payments within 0.1%.', latest: { outcome: 'pass', measured: 0.02 }, history: { measuredBase: 0.03, warnEvery: 8 }, sql: `SELECT 100 * abs(m.total - p.total) / nullif(p.total, 0) AS measured_value
FROM (SELECT sum(total_revenue_cents) AS total FROM "WAFFLE_SHOP"."MARTS"."DAILY_REVENUE") m,
     (SELECT sum(amount_cents) AS total FROM "WAFFLE_SHOP"."STAGING"."STG_PAYMENTS" WHERE status = 'success') p` }
];

// ---- snapshot change feed (one entry per deploy) ----

export type DeploySpec = {
	generation: number;
	gitRevision: string;
	gitDirty: boolean;
	gitAuthor: string;
	minutesAgo: number;
	publishedBy: Principal;
	nodes: NodeChange[];
	orchestration: Omit<OrchestrationChange, 'triggerId'>[];
	roles: RoleChange[];
	environment: EnvironmentChange | null;
};

const node = (key: string, op: NodeChange['op'], detail: string, extra: Partial<NodeChange> = {}): NodeChange => ({
	key,
	kind: null,
	op,
	formerKey: null,
	compiledOnly: false,
	detail,
	columns: [],
	...extra
});

export const DEPLOYS: DeploySpec[] = [
	{
		generation: 42,
		gitRevision: '7d2e41a',
		gitDirty: false,
		gitAuthor: ALEX.displayName,
		minutesAgo: 190,
		publishedBy: CI_ACCOUNT,
		nodes: [
			node('dim_customers', 'changed', 'Column added', { columns: [{ name: 'lifetime_value', op: 'added', detail: 'decimal(18,2)' }] }),
			node('revenue_report', 'changed', 'SQL changed', { columns: [{ name: 'avg_order_value', op: 'added', detail: 'decimal(18,2)' }] }),
			node('activity_report', 'changed', 'Compiled SQL changed: macro pct now rounds to one decimal place', { compiledOnly: true })
		],
		orchestration: [{ kind: 'trigger', key: 'revenue_refresh', op: 'changed', detail: 'Cron moved from 06:00 to 05:30' }],
		roles: [
			{ role: 'finance_operator', op: 'changed', detail: 'Adds run.rewrite_history and widens view.evidence to tag:finance; drops run.adhoc', coverage: { before: 3, after: 3 } },
			{ role: 'reporting_operator', op: 'changed', detail: 'Adds trigger.toggle on group:reporting', coverage: { before: 2, after: 2 } }
		],
		environment: null
	},
	{
		generation: 41,
		gitRevision: '3a9c0f1',
		gitDirty: false,
		gitAuthor: SAM.displayName,
		minutesAgo: 2 * DAY + 20,
		publishedBy: CI_ACCOUNT,
		nodes: [
			node('hourly_activity_ctx', 'renamed', 'Renamed from activity_hourly', { formerKey: 'activity_hourly' }),
			node('fact_orders', 'changed', 'SQL changed: is_first_order now uses the customer’s first order', { columns: [{ name: 'is_first_order', op: 'changed', detail: 'expression changed' }] })
		],
		orchestration: [{ kind: 'sensor', key: 'on_fact_orders_rebuilt', op: 'added', detail: 'Node sensor: requests activity_microbatch when fact_orders is rebuilt' }],
		roles: [],
		environment: { before: 'env-3f1c9a', after: 'env-8b2d47', detail: 'Dependencies changed: uv.lock updated 2 packages' }
	},
	{
		generation: 40,
		gitRevision: '5b1e7d3',
		gitDirty: false,
		gitAuthor: JORDAN.displayName,
		minutesAgo: 5 * DAY + 130,
		publishedBy: DEMO_USER,
		nodes: [
			node('daily_activity_rollup', 'added', 'New incremental microbatch model'),
			node('customer_status', 'changed', 'Config changed: materialization table → incremental · merge')
		],
		orchestration: [
			{ kind: 'job', key: 'activity_microbatch', op: 'changed', detail: 'Settings: --max-microbatches 4 → 8' },
			{ kind: 'group', key: 'nightly_all', op: 'changed', detail: 'Member added: reporting_exports' }
		],
		roles: [{ role: 'reporting_operator', op: 'added', detail: 'Operates the reporting group', coverage: { before: 0, after: 2 } }],
		environment: null
	},
	{
		generation: 39,
		gitRevision: 'e04f9b2',
		gitDirty: false,
		gitAuthor: RILEY.displayName,
		minutesAgo: 9 * DAY + 15,
		publishedBy: CI_ACCOUNT,
		nodes: [
			node('daily_revenue', 'renamed', 'Renamed from revenue_by_day', {
				formerKey: 'revenue_by_day',
				columns: [
					{ name: 'revenue_date', op: 'renamed', detail: 'from day' },
					{ name: 'total_revenue_cents', op: 'renamed', detail: 'from amount' }
				]
			}),
			node('weekly_orders', 'removed', 'Model removed')
		],
		orchestration: [],
		roles: [],
		environment: { before: 'env-1a7e02', after: 'env-3f1c9a', detail: 'sqb 0.184.2 → 0.185.0' }
	}
];
