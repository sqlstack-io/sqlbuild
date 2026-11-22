import { describe, expect, it } from 'vitest';
import type { PlanCantRun } from '$lib/api';
import { cantRunKeys, cantRunReason, cantRunRows, fixText, needsText, runCantRun, sinkBlocks, sinkLines, willRunCount, withoutNodes } from './cant-run';

const payments: PlanCantRun = {
	reason: 'missing_secret',
	provider: 'payments_api',
	path: 'providers/payments_api.py',
	variables: ['PAYMENTS_API_TOKEN'],
	nodes: ['source:raw_payments'],
	readers: ['model:stg_payments', 'model:fct_payments'],
	sinks: [],
	env_file: '.env'
};
const chatSink: PlanCantRun = { ...payments, provider: 'chat', variables: ['CHAT_URL'], nodes: [], readers: [], sinks: ['run_log'] };

describe('a plan with nodes that cannot run', () => {
	it('lists the provider users before their readers, once each', () => {
		expect(cantRunKeys([payments, { ...payments, provider: 'other', nodes: ['model:stg_payments'], readers: [] }])).toEqual([
			'source:raw_payments',
			'model:stg_payments',
			'model:fct_payments'
		]);
	});

	it('says why Run is disabled and what leaves them out', () => {
		expect(cantRunReason([payments])).toBe('Run is disabled: 1 missing secret. Fix it and plan again, or leave those 3 nodes out.');
		expect(withoutNodes('sqb build --select job:shop_job --exclude old', cantRunKeys([payments]))).toBe(
			'sqb build --select job:shop_job --exclude old source:raw_payments stg_payments fct_payments'
		);
	});

	it('names what the provider stops and how to fix it', () => {
		expect(needsText(payments)).toBe('raw_payments and the 2 nodes that read it (stg_payments, fct_payments)');
		expect(fixText(payments, 'plan again')).toBe(
			'set PAYMENTS_API_TOKEN in .env, then restart the location (in sqb dev, press r), or in the environment the location (or sqb dev) was started with, then restart it; after that, plan again'
		);
		expect(fixText({ ...payments, env_file: null }, 'plan again')).toBe('set PAYMENTS_API_TOKEN in the environment the location (or sqb dev) was started with, then restart it; after that, plan again');
	});

	it('gives each node its reason, readers naming the blocked node they read', () => {
		const upstream: Record<string, string[]> = { 'model:stg_payments': ['source:raw_payments'], 'model:fct_payments': ['model:stg_payments'] };
		expect(cantRunRows([payments], (key) => upstream[key] ?? [])).toEqual([
			{ key: 'source:raw_payments', uses: true, why: 'provider payments_api: PAYMENTS_API_TOKEN not set' },
			{ key: 'model:stg_payments', uses: false, why: 'reads raw_payments' },
			{ key: 'model:fct_payments', uses: false, why: 'reads stg_payments' }
		]);
	});
});

describe('a run that hit missing secrets', () => {
	it('reads each provider, its failed nodes and what was skipped because of them', () => {
		const message = "provider payments_api: PAYMENTS_API_TOKEN not set (missing from the location's environment and from .env)";
		const at = { runId: 'r1', startedAt: null };
		expect(
			runCantRun(
				[
					{ ...at, nodeKey: 'source:raw_payments', status: 'failed', errorCode: 'D012', errorMessage: message, skippedBecauseOf: null },
					{ ...at, nodeKey: 'model:stg_payments', status: 'skipped', errorCode: null, errorMessage: null, skippedBecauseOf: { nodeKey: 'source:raw_payments' } },
					{ ...at, nodeKey: 'model:fct_payments', status: 'skipped', errorCode: null, errorMessage: null, skippedBecauseOf: { nodeKey: 'model:stg_payments' } },
					{ ...at, nodeKey: 'model:orders', status: 'failed', errorCode: 'Q001', errorMessage: 'syntax error', skippedBecauseOf: null }
				],
				[{ id: 'r1', statusReason: null }]
			)
		).toEqual([{ ...payments, path: '' }]);
	});
});

describe('what a plan can run when secrets are missing', () => {
	it('counts Will run without the nodes it can not run', () => {
		expect(willRunCount(90, [payments])).toBe(87);
		expect(willRunCount(2, [payments])).toBe(0);
	});

	it('says a sink stops the whole plan, counts it as no node, and offers no way around it', () => {
		expect(cantRunKeys([chatSink])).toEqual([]);
		expect(sinkBlocks([chatSink])).toBe(true);
		expect(sinkBlocks([payments])).toBe(false);
		expect(sinkLines([chatSink])).toEqual(['sink run_log needs CHAT_URL; every run uses sinks']);
		expect(willRunCount(90, [payments, chatSink])).toBe(0);
		expect(cantRunReason([payments, chatSink])).toBe('Run is disabled: sink run_log needs CHAT_URL; every run uses sinks. Fix it and plan again.');
	});
});
