import { describe, expect, it } from 'vitest';
import type { RunEdge, RunEvent, RunNode } from '$lib/api';
import { rowsText } from './events';
import { quickPickText, withNameToggled } from './picks';
import { skipCause } from './skips';
import { summaryOf } from './summary';

const node = (nodeKey: string, status: RunNode['status'], extra: Partial<RunNode> = {}) => ({ nodeKey, status, runId: 'r', kind: 'table', rowsAffected: null, skippedBecauseOf: null, ...extra }) as RunNode;
const check = (level: RunEvent['level']) => ({ type: 'audit_completed', level }) as RunEvent;

describe('a run’s summary line', () => {
	it('counts what it built, what incrementals added, and its problems and checks', () => {
		const nodes = [
			node('model:orders', 'success'),
			node('model:events', 'success', { kind: 'incremental', rowsAffected: 48_000 }),
			node('model:margin', 'warn'),
			node('model:stock', 'failed'),
			node('model:report', 'skipped'),
			node('model:late', 'not_run')
		];
		expect(summaryOf(nodes, [check('info'), check('info'), check('warn')], false)).toEqual({
			built: 3,
			incremental: 1,
			incrementalRows: 48_000,
			failed: 1,
			warned: 1,
			skipped: 1,
			notRun: 1,
			running: 0,
			waiting: 0,
			checksPassed: 2,
			checksWarned: 1,
			checksFailed: 0
		});
	});

	it('counts nodes that have not started as waiting while the run goes on, and as not run once it ended', () => {
		const nodes = [node('model:orders', 'running'), node('model:report', 'pending')];
		expect([summaryOf(nodes, [], true).waiting, summaryOf(nodes, [], false).notRun]).toEqual([1, 1]);
	});
});

describe('why a node was skipped', () => {
	const failed = node('source:raw_orders', 'failed');
	const loader = node('loader:orders', 'skipped', { skippedBecauseOf: { nodeKey: 'source:raw_orders', runId: 'r', jobKey: null } });
	const model = node('model:orders', 'skipped', { skippedBecauseOf: { nodeKey: 'source:raw_orders', runId: 'r', jobKey: null } });
	const edges: RunEdge[] = [
		{ from: 'source:raw_orders', to: 'loader:orders' },
		{ from: 'loader:orders', to: 'model:orders' }
	];

	it('names the failure under it, and the skipped node between it and a failure further up', () => {
		const all = [failed, loader, model];
		expect([skipCause(loader, all, edges), skipCause(model, all, edges), skipCause(failed, all, edges)]).toEqual([
			'skipped: raw_orders failed',
			'skipped: orders was skipped (after raw_orders failed)',
			null
		]);
	});
});

describe('quick picks', () => {
	const nodes = [node('model:orders', 'failed'), node('model:report', 'skipped'), node('model:customers', 'success')];

	it('fill the selection with the failed, their downstream, the skipped, or the whole job', () => {
		expect([quickPickText('failed', nodes, 'shop_daily'), quickPickText('downstream', nodes, 'shop_daily'), quickPickText('skipped', nodes, 'shop_daily'), quickPickText('all', nodes, 'shop_daily'), quickPickText('all', nodes, null)]).toEqual([
			'orders',
			'orders+',
			'report',
			'job:shop_daily',
			'orders report customers'
		]);
	});

	it('toggle one node’s plain name in the selection', () => {
		expect([withNameToggled('orders report', 'report'), withNameToggled('orders', 'report'), withNameToggled('', 'orders')]).toEqual(['orders', 'orders report', 'orders']);
	});
});

describe('a node’s rows', () => {
	it('are only what it can report: rows added, a rebuilt table’s count, or rows so far', () => {
		expect([
			rowsText(node('model:events', 'success', { kind: 'incremental', rowsAffected: 1200 })),
			rowsText(node('loader:raw_events', 'success', { kind: 'loader', rowsAffected: 1 })),
			rowsText(node('model:orders', 'success', { tableRows: 5000, rowsAffected: 5000 })),
			rowsText(node('model:orders', 'running', { rowsSoFar: 320 })),
			rowsText(node('model:orders_view', 'success', { kind: 'view' })),
			rowsText(node('model:orders', 'running'))
		]).toEqual(['+1,200 rows', '+1 row', '5,000 rows', '320 rows so far', null, null]);
	});
});
