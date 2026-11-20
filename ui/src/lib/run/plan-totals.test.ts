import { describe, expect, it } from 'vitest';
import { planSummary } from '$lib/api/mock/large-plan';
import { summaryPlanDocument } from '$lib/api/mock/plan-summary-mockup';
import { planGroups } from './plan-index';
import { migrationSteps } from './plan-steps';
import { planSectionCounts, planTotals } from './plan-totals';

describe('planTotals', () => {
	const doc = summaryPlanDocument(true);
	const summary = planSummary(doc);

	it('counts each node once: renames, migrations and column migrations of the same models are one node each', () => {
		const totals = planTotals(planSectionCounts(summary), migrationSteps(summary));
		expect(totals.run).toBe(134 + 4 + 58 + 2184 + 79 + 418);
		expect(totals.wont).toBe(97);
		expect(totals.steps).toBe(63);
		expect(totals.warnings).toBe(83);
		// sqb plan counts models, seeds and functions as selected; the totals count loads and Python nodes,
		// which run, and leave out functions and disabled models, which aren't built.
		const disabled = doc.models.filter((m) => m.action === 'skip').length;
		expect(totals.run + totals.wont).toBe(doc.selected_count - doc.functions.length - disabled + doc.source_loads.length + doc.python_nodes.length);
		expect(doc.selected_count).toBe(doc.models.length + doc.seeds.length + doc.functions.length);
		expect(totals.runParts.map((p) => [p.key, p.count])).toEqual([
			['changed', 134],
			['renamed', 58],
			['first_run', 4],
			['loads', 79],
			['python', 418],
			['unchanged', 2184]
		]);
		expect(totals.wontParts.map((p) => [p.key, p.count])).toEqual([['skipped', 97]]);
	});

	it('agrees with the plan index, whose Renames group lists the renames’ own migrations as rows too', () => {
		const groups = planGroups(summary);
		const n = (key: string) => groups.find((g) => g.key === key)?.count;
		expect([n('changed'), n('first_run'), n('renames'), n('loads'), n('python'), n('unchanged'), n('skipped'), n('warnings')]).toEqual([134, 4, 58 + 58 + 63, 79, 418, 2184, 97, 83]);
	});

	it('counts a whole plan, with every row, the same as its server summary', () => {
		expect(planTotals(planSectionCounts(doc), migrationSteps(doc))).toEqual(planTotals(planSectionCounts(summary), migrationSteps(summary)));
	});

	it('takes the steps from the server summary, which leaves the renames’ own migrations out', () => {
		expect(summary.migration_steps).toBe(63);
		expect(summary.sections?.find((c) => c.section === 'migrations')?.total).toBe(60);
	});

	it('without --changed, nothing is skipped', () => {
		const plain = planSummary(summaryPlanDocument(false));
		const totals = planTotals(planSectionCounts(plain), migrationSteps(plain));
		expect(totals.wont).toBe(0);
		expect(totals.wontParts).toEqual([]);
	});
});
