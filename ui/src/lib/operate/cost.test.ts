import { describe, expect, it } from 'vitest';
import type { RunCostRow } from '$lib/api';
import { combinedSource, costSummary, costTitle, formatQueryCost, runCostOf } from './cost';

const row = (overrides: Partial<RunCostRow>): RunCostRow => ({
	priceSource: overrides.usd === null ? null : 'config',
	nodeKey: `${overrides.resourceType ?? 'model'}:${overrides.resourceName ?? 'orders'}`,
	buildRunId: 'b1',
	resourceType: 'model',
	resourceName: 'orders',
	warehouseName: 'LOAD_WH',
	warehouseSize: 'X-Small',
	queryCount: 1,
	seconds: 10,
	bytesScanned: 0,
	credits: 0.1,
	usd: 0.3,
	status: 'complete',
	statementsExpected: 4,
	statementsObserved: 4,
	...overrides
});
const run = (overrides: Partial<RunCostRow>): RunCostRow => row({ resourceType: '_total', resourceName: 'b1', nodeKey: null, warehouseName: '', warehouseSize: '', ...overrides });

describe("a run's estimated query cost", () => {
	it('takes totals from the run row, which also counts queries no node owns', () => {
		const cost = runCostOf([run({ credits: 1, usd: 3 }), row({ credits: 0.5, usd: 1.5 }), row({ resourceName: 'customers', credits: 0.25, usd: 0.75 })]);
		expect([cost.credits, cost.usd]).toEqual([1, 3]);
		expect(cost.resources.map((r) => [r.nodeKey, r.credits])).toEqual([
			['model:orders', 0.5],
			['model:customers', 0.25]
		]);
	});

	it('folds one node across warehouses and ranks warehouses by cost', () => {
		const cost = runCostOf([
			run({ credits: 0.7 }),
			row({ credits: 0.1, warehouseName: 'LOAD_WH', warehouseSize: 'X-Small' }),
			row({ credits: 0.4, warehouseName: 'TRANSFORM_WH', warehouseSize: 'Small' }),
			row({ resourceName: 'customers', credits: 0.2, warehouseName: 'LOAD_WH', warehouseSize: 'X-Small' })
		]);
		expect(cost.resources.map((r) => [r.nodeKey, r.credits, r.warehouses])).toEqual([
			['model:orders', 0.5, ['LOAD_WH', 'TRANSFORM_WH']],
			['model:customers', 0.2, ['LOAD_WH']]
		]);
		expect(cost.warehouses.map((w) => [w.name, w.size])).toEqual([
			['TRANSFORM_WH', 'Small'],
			['LOAD_WH', 'X-Small']
		]);
		expect(cost.warehouses[1].credits).toBeCloseTo(0.3);
	});

	it("shows a group run's weakest collection and drops dollars when any job run has no price", () => {
		const cost = runCostOf([run({ buildRunId: 'b1', status: 'complete' }), run({ buildRunId: 'b2', status: 'partial', usd: null, statementsExpected: 5, statementsObserved: 2 })]);
		expect([cost.status, cost.usd, cost.statementsObserved, cost.statementsExpected]).toEqual(['partial', null, 6, 9]);
	});

	it('is none until a collection synced', () => {
		expect(runCostOf([]).status).toBe('none');
	});
});

describe('what the cost figure says', () => {
	it('never shows missing cost as zero', () => {
		expect(costSummary(runCostOf([run({ status: 'pending', credits: 0, usd: 0 })])).text).toBe('Cost pending');
		expect(costSummary(runCostOf([run({ status: 'collection_failed', credits: 0, usd: 0 })])).text).toBe('Cost unavailable');
	});

	it('names how much of a partial run was counted', () => {
		expect(costSummary(runCostOf([run({ status: 'partial', usd: 1.234, statementsExpected: 4, statementsObserved: 3 })]))).toEqual({ text: '$1.23', note: 'Partial: 3 of 4 statements' });
		expect(costSummary(runCostOf([run({ status: 'partial', statementsExpected: 4, statementsObserved: 4 })])).note).toBe('Partial: the warehouse left out some query details');
	});

	it('shows credits without a configured price and a floor for tiny amounts', () => {
		expect(formatQueryCost({ credits: 0.0125, usd: null })).toBe('0.013 credits');
		expect(formatQueryCost({ credits: 0.001, usd: 0.003 })).toBe('< $0.01');
		expect(formatQueryCost({ credits: 0, usd: 0 })).toBe('$0.00');
	});
});

describe('prices and their sources', () => {
	it('marks dollars at a list price as an estimate, and names the source in the tooltip', () => {
		expect(formatQueryCost({ credits: 1, usd: 3.9, priceSource: 'list_price' })).toBe('$3.90 est.');
		expect(formatQueryCost({ credits: 1, usd: 2.73, priceSource: 'rate_sheet' })).toBe('$2.73');
		expect(costTitle({ credits: 2, usd: 7.8, priceSource: 'list_price' })).toBe("2.000 credits × $3.90 per credit (Snowflake's list price for the account's region (an estimate))");
		expect(costTitle({ credits: 2, usd: null, priceSource: null })).toBeUndefined();
	});
	it('reads several sources as one: theirs, the list price when any is one, else mixed', () => {
		expect(combinedSource(['config', 'config', null])).toBe('config');
		expect(combinedSource(['config', 'list_price'])).toBe('list_price');
		expect(combinedSource(['config', 'rate_sheet'])).toBe('mixed');
		expect(combinedSource([null])).toBeNull();
	});
	it("gives a run's breakdown its rows' source", () => {
		expect(runCostOf([row({ resourceType: '_total', resourceName: 'b1', nodeKey: null, priceSource: 'list_price' }), row({ priceSource: 'list_price' })]).priceSource).toBe('list_price');
	});
});
