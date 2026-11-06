// Model SQL for the mock project, standing in for the snapshot's raw and
// compiled SQL and for the definitions stored with each fingerprint.

import type { SnapshotNode } from '../types';
import { NODE_SPECS, type NodeSpec } from './build-fixtures';

const FACT_ORDERS_SQL = `MODEL (
  materialized table,
  unique_key order_id,
  owner [analytics, finance-data],
  tags [finance, core],
  description "One row per order with payment and customer attributes."
);

WITH orders AS (
  SELECT order_id, customer_id, order_status, total_cents, ordered_at
  FROM __ref("stg_orders")
),
payments AS (
  SELECT order_id, payment_method, amount_cents
  FROM __ref("stg_payments")
  WHERE status = 'success'
),
customers AS (
  SELECT customer_id, first_order_at
  FROM __ref("stg_customers")
)

SELECT
  o.order_id,
  o.customer_id,
  o.order_status,
  o.total_cents,
  p.payment_method,
  o.ordered_at,
  o.ordered_at = c.first_order_at AS is_first_order
FROM orders o
LEFT JOIN payments p USING (order_id)
LEFT JOIN customers c USING (customer_id)`;

/** Before generation 41, `is_first_order` was derived with a window function. */
const FACT_ORDERS_BEFORE_41 = FACT_ORDERS_SQL.replace(',\ncustomers AS (\n  SELECT customer_id, first_order_at\n  FROM __ref("stg_customers")\n)', '')
	.replace('  o.ordered_at = c.first_order_at AS is_first_order', '  row_number() OVER (PARTITION BY o.customer_id ORDER BY o.ordered_at) = 1 AS is_first_order')
	.replace('\nLEFT JOIN customers c USING (customer_id)', '');

function header(spec: NodeSpec, sn: SnapshotNode): string {
	const [materialized, strategy] = (spec.materialization ?? 'table').split(' · ');
	const lines = [`materialized ${materialized}`];
	if (strategy) lines.push(`strategy ${strategy}`);
	if (spec.microbatch) lines.push(`cursor ${spec.microbatch.cursorColumn}`, `batch_size ${spec.microbatch.batchSize}`);
	if (spec.owner.length) lines.push(`owner [${spec.owner.join(', ')}]`);
	if (sn.tags.length) lines.push(`tags [${sn.tags.join(', ')}]`);
	lines.push(`description "${spec.description}"`);
	return `MODEL (\n  ${lines.join(',\n  ')}\n);`;
}

function generated(spec: NodeSpec, sn: SnapshotNode, columns = spec.columns): string {
	const refOf = (key: string) => (NODE_SPECS[key] && key.startsWith('raw__') && key !== 'raw__orders' ? `__source("${key}")` : `__ref("${key}")`);
	const from = spec.from ?? sn.upstream.map(refOf).join('\nCROSS JOIN ');
	const select = columns.map((c) => `  ${c.expr ?? c.name}`).join(',\n');
	const parts = [header(spec, sn), '', `SELECT\n${select}`, `FROM ${from}`];
	if (spec.where) parts.push(`WHERE ${spec.where}`);
	if (spec.groupBy) parts.push('GROUP BY ALL');
	return parts.join('\n');
}

/** Current raw SQL in the snapshot; null for sources, seeds and Python nodes. */
export function rawSql(sn: SnapshotNode): string | null {
	if (sn.kind === 'source' || sn.kind === 'seed' || sn.kind === 'loader' || sn.kind === 'task') return null;
	if (sn.key === 'fact_orders') return FACT_ORDERS_SQL;
	const spec = NODE_SPECS[sn.key];
	return spec ? generated(spec, sn) : null;
}

/** Raw SQL as it was before `generation` took effect. */
export function rawSqlBefore(sn: SnapshotNode, generation: number): string | null {
	const current = rawSql(sn);
	const spec = NODE_SPECS[sn.key];
	if (!current || !spec) return current;
	if (sn.key === 'fact_orders' && generation <= 41) return FACT_ORDERS_BEFORE_41;
	if (generation <= 42 && sn.key === 'dim_customers') return generated(spec, sn, spec.columns.filter((c) => c.name !== 'lifetime_value'));
	if (generation <= 42 && sn.key === 'revenue_report') return generated(spec, sn, spec.columns.filter((c) => c.name !== 'avg_order_value'));
	return current;
}

const MACROS = (pctDecimals: number): [RegExp, string][] => [
	[/cents_to_dollars\(([^()]*(?:\([^()]*\))?[^()]*)\)/g, '($1) / 100.0'],
	[/pct\(([^()]*)\)/g, `round(100 * $1, ${pctDecimals})`]
];

/** Compiles refs to this target's relations and expands the project's macros. */
export function compileSql(raw: string, options: { pctDecimals?: number } = {}): string {
	let sql = raw.replace(/^MODEL\s*\([\s\S]*?\n\);\s*\n+/, '');
	sql = sql.replace(/__(?:ref|source)\("(\w+)"\)/g, (_m, key: string) => {
		const schema = NODE_SPECS[key]?.schema ?? 'ANALYTICS';
		return `"WAFFLE_SHOP"."${schema}"."${key.toUpperCase()}"`;
	});
	for (const [re, replacement] of MACROS(options.pctDecimals ?? 1)) sql = sql.replace(re, replacement);
	return sql;
}

/** Compiled SQL as built before `generation` (the `pct` macro changed in 42). */
export function compileSqlBefore(raw: string, generation: number): string {
	return compileSql(raw, { pctDecimals: generation <= 42 ? 2 : 1 });
}
