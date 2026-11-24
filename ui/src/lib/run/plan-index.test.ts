import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanModel } from '$lib/api';
import { cursorWords, diffPreview, diffStats, sectionNote, groupWarnings, planGroups, selectedGroup, sharedNote, whitespaceOnly, type PlanRow } from './plan-index';

const empty: PlanDocument = {
	selected_count: 0,
	models: [],
	seeds: [],
	source_loads: [],
	functions: [],
	python_nodes: [],
	warnings: [],
	migrations: [],
	column_migrations: [],
	table_type_conversions: [],
	retention: [],
	synced_copy: null,
	changed: null
};
const model = (name: string, reason: string, extra: Partial<PlanModel> = {}): PlanModel => ({
	name,
	materialization_type: 'table',
	action: 'create_table',
	reason,
	backfill: { action: 'forward', duration: null },
	...extra
});
const head = ['--- previous', '+++ current', '@@ -1,3 +1,3 @@'];

describe('whitespace-only query changes', () => {
	it('labels a diff that only moves whitespace', () => {
		expect(whitespaceOnly([...head, '-select id,  name', '-from orders', '+select id,', '+    name from orders'])).toBe(true);
	});

	it("doesn't label a change of keyword case: it's a change", () => {
		expect(whitespaceOnly([...head, '-select id from orders', '+SELECT id FROM orders'])).toBe(false);
	});

	it("doesn't label a change of what the query reads", () => {
		expect(whitespaceOnly([...head, '-select id from orders', '+select id from orders_v2'])).toBe(false);
	});

	it('counts the changed lines, not the diff headers', () => {
		expect(diffStats([...head, ' select', '-  a', '+  b', '+  c'])).toEqual({ added: 2, removed: 1 });
	});
});

describe('cursor ranges in words', () => {
	it("words the executor's bound placeholders", () => {
		expect(cursorWords('range: __SQB_CURSOR_START__ → __SQB_CURSOR_END__')).toBe('range: from the last run → the latest data when the run starts');
		expect(cursorWords('range: 2026-11-01 → __SQB_CURSOR_END__')).toBe('range: 2026-11-01 → the latest data when the run starts');
	});
});

describe('notes every row shares', () => {
	const row = (name: string, detail: string, notes: string[] = []): PlanRow => ({ name, action: 'table', detail, notes, diff: null, diffOf: null, warnings: [], linked: true, fetch: null });

	it('finds the note every row carries, and none when one differs', () => {
		expect(sharedNote([row('a', 'no completed record'), row('b', 'no completed record')])).toBe('no completed record');
		expect(sharedNote([row('a', 'no completed record'), row('b', 'reads orders')])).toBeNull();
		expect(sharedNote([row('a', 'no completed record')])).toBeNull();
	});

	it('moves it to the group subtitle, off the rows', () => {
		const plan: PlanDocument = {
			...empty,
			models: ['orders', 'customers'].map((n) => model(n, 'no_change')),
			changed: { counts: ['definition'], reach: 'all', runs: ['orders', 'customers'].map((n) => ({ node: `model:${n}`, reason: 'no completed record', downstream_of: null })), held_back: [], unchanged: [] }
		};
		const [unchanged] = planGroups(plan);
		expect(unchanged.key).toBe('unchanged');
		expect(unchanged.subtitle).toBe('no completed record');
		expect(unchanged.rows.map((r) => r.detail)).toEqual(['', '']);
	});
});

describe('warnings grouped by message', () => {
	it('groups one message about different names into one row, counting the names it opens with', () => {
		const warnings = [
			...['a', 'b'].flatMap((s) => ['x', 'y', 'z'].map((m) => ({ severity: 'warning', message: `Scenario '${s}' provides fixture for unknown model '${m}'.` }))),
			{ severity: 'warning', message: 'orders: schema change detected but on_schema_change is set to ignore', model_name: 'orders' },
			...[1, 2].map((k) => ({ severity: 'warning', message: `Scenario 't${k}' requires source 'raw.r${k}', but no fixture was provided. Add a CTE named __source__raw__r${k} AS (...).` }))
		];
		const groups = groupWarnings(warnings);
		expect(groups.map((g) => [g.template, g.label, g.warnings.length])).toEqual([
			['Scenario … provides fixture for unknown model …', '6 in 2 scenarios', 6],
			['Scenario … requires source …, but no fixture was provided. Add a CTE named … AS (...).', '2 warnings', 2],
			['schema change detected but on_schema_change is set to ignore', '1 warning', 1]
		]);
	});
});

describe('the plan index', () => {
	const plan: PlanDocument = {
		...empty,
		models: [
			model('orders', 'query_changed', { query_diff: [...head, '-select 1', '+select 2'] }),
			model('payments', 'query_changed', { query_diff: [...head, '-select  1', '+select 1'] }),
			model('refunds', 'config_changed', { config_diff: ['-  "full_refresh": true,', '+  "full_refresh": false,'], config_change: 'full_refresh true → false' }),
			model('customers', 'first_run'),
			model('regions', 'no_change')
		],
		python_nodes: [{ name: 'export_orders', kind: 'asset', phase: 'after', identity_status: 'known' }],
		warnings: [{ severity: 'warning', message: "Scenario 'a' provides fixture for unknown model 'x'." }]
	};
	const groups = planGroups(plan);

	it('lists the non-empty groups in order, with the bulky ones after the warnings', () => {
		expect(groups.map((g) => [g.key, g.count])).toEqual([
			['changed', 3],
			['first_run', 1],
			['python', 1],
			['warnings', 1],
			['unchanged', 1]
		]);
	});

	it('breaks the changes down by kind, with line counts, whitespace-only and inline config changes', () => {
		const changed = groups[0];
		expect(changed.subtitle).toBe('2 query · 1 config');
		expect(Object.fromEntries(changed.rows.map((r) => [r.name, r.detail]))).toEqual({ orders: 'query +1 −1', payments: 'whitespace only', refunds: 'config · full_refresh true → false' });
		expect(changed.rows.find((r) => r.name === 'refunds')!.diffOf).toBeNull();
		expect(changed.rows.find((r) => r.name === 'orders')!.diffOf).toBe('query');
	});

	it('opens on the group the address names, else the first', () => {
		expect(selectedGroup(groups, 'python')?.key).toBe('python');
		expect(selectedGroup(groups, 'nonsense')?.key).toBe('changed');
		expect(selectedGroup(groups, null)?.key).toBe('changed');
	});
});

describe('a model that can\'t be planned', () => {
	const plan: PlanDocument = {
		...empty,
		has_errors: true,
		models: [
			model('orders', 'planning_error', { action: 'error', error: { code: 'S203', message: "model 'orders' sets full_refresh false", help: 'allow the rebuild' } }),
			model('totals', 'query_changed', { query_diff: [...head, '-select 1', '+select 2'] })
		]
	};
	const groups = planGroups(plan);

	it('is its own group first, with its error, and the rest still plans', () => {
		expect(groups.map((g) => [g.key, g.count])).toEqual([
			['errors', 1],
			['changed', 1]
		]);
		expect(groups[0].title).toBe('Planning failed');
		expect(groups[0].rows.map((r) => [r.name, r.action, r.detail, r.notes])).toEqual([['orders', 'error', "error[S203]: model 'orders' sets full_refresh false", ['help: allow the rebuild']]]);
	});
});

describe('an opened diff', () => {
	it('shows the first changed lines, then says the rest is cut off', () => {
		const diff = [{ text: '@@', cls: 'hunk' as const }, ...Array.from({ length: 20 }, (_, i) => ({ text: `+${i}`, cls: 'add' as const }))];
		const preview = diffPreview(diff, 12);
		expect(preview.lines).toHaveLength(13);
		expect(preview.cut).toBe(true);
		expect(preview.more).toBe(8);
		expect(diffPreview(diff.slice(0, 5), 12)).toEqual({ lines: diff.slice(0, 5), cut: false, more: 0 });
	});
});

describe('a plan from a server summary', () => {
	it('says the note the counts show every row shares as "all N", and a majority as "N of M"', () => {
		const notes = (count: number) => [{ kind: 'run_reason', value: 'no completed record', count }, { kind: 'materialization', value: 'table', count: 1507 }];
		expect(sectionNote({ section: 'routine_models', total: 1507, notes: notes(1507) })).toEqual({ row: 'no completed record', all: true, text: 'no completed record (all 1,507)' });
		expect(sectionNote({ section: 'routine_models', total: 1507, notes: notes(1200) })?.text).toBe('no completed record (1,200 of 1,507)');
		expect(sectionNote({ section: 'routine_models', total: 1507, notes: notes(700) })).toBeNull();
		expect(sectionNote({ section: 'routine_models', total: 3, notes: [{ kind: 'reads', value: 'orders', count: 3 }] })?.text).toBe('reads orders (all 3)');
	});

	const summary: PlanDocument = {
		...empty,
		models: [
			model('orders', 'query_changed', { diff_stats: { query: { added: 82, removed: 104, whitespace_only: false } } }),
			model('payments', 'query_changed', { diff_stats: { query: { added: 2, removed: 2, whitespace_only: true } } }),
			model('refunds', 'config_changed', { config_change: 'full_refresh true → false', diff_stats: { config: { added: 1, removed: 1, whitespace_only: false } } }),
			model('ledger', 'config_changed', { diff_stats: { config: { added: 3, removed: 3, whitespace_only: false } } }),
			model('regions', 'no_change')
		],
		sections: [
			{ section: 'query_changed', total: 2, notes: [] },
			{ section: 'config_changed', total: 2, notes: [] },
			{ section: 'routine_models', total: 1507, notes: [{ kind: 'run_reason', value: 'no completed record', count: 1507 }] },
			{ section: 'first_run_seeds', total: 2, notes: [] },
			{ section: 'unchanged_seeds', total: 10, notes: [] },
			{ section: 'unchanged', total: 300, notes: [] },
			{ section: 'warnings', total: 74, notes: [] }
		],
		migration_steps: 0,
		pages: { query_changed: 2, config_changed: 2, routine_models: 1507, first_run_seeds: 2, unchanged_seeds: 10, unchanged: 300, warnings: 74 },
		loaded: { query_changed: 2, config_changed: 2, routine_models: 1 },
		warning_groups: [
			{ template: 'Scenario … provides fixture for unknown model …', count: 71, subject: { noun: 'scenario', count: 24 } },
			{ template: 'Scenario … requires source …', count: 3, subject: null }
		],
		changed: { counts: ['definition'], reach: 'all', runs: ['orders', 'payments', 'refunds', 'ledger', 'regions'].map((n) => ({ node: `model:${n}`, reason: n === 'regions' ? 'no completed record' : 'definition changed', downstream_of: null })), held_back: [], unchanged: [] }
	};

	it("counts groups from the server's sections, and pages their rows in plan order", () => {
		const groups = planGroups(summary);
		expect(groups.map((g) => [g.key, g.count])).toEqual([
			['changed', 4],
			['first_run', 2],
			['warnings', 74],
			['unchanged', 1517],
			['skipped', 300]
		]);
		const unchanged = groups[3];
		expect(unchanged.subtitle).toBe('1,507 models · 10 seeds · models: no completed record (all 1,507)');
		expect(unchanged.rows.map((r) => [r.name, r.action, r.detail])).toEqual([['regions', 'table', '']]);
		expect(unchanged.pages).toEqual([
			{ section: 'routine_models', loaded: 1, total: 1507 },
			{ section: 'unchanged_seeds', loaded: 0, total: 10 }
		]);
		expect(groups[1].pages).toEqual([{ section: 'first_run_seeds', loaded: 0, total: 2 }]);
	});

	it("reads changed rows' counts from diff_stats, and reads a diff it doesn't carry on demand", () => {
		const rows = (diffs?: Map<string, null | { model: string; config_diff?: string[] }>) => Object.fromEntries(planGroups(summary, undefined, diffs)[0].rows.map((r) => [r.name, r]));
		const before = rows();
		expect(before.orders).toMatchObject({ detail: 'query +82 −104', diff: null, fetch: 'diff' });
		expect(before.payments.detail).toBe('whitespace only');
		// The server's short config change reads inline, with nothing to read; a longer one links to its diff.
		expect(before.refunds).toMatchObject({ detail: 'config · full_refresh true → false', fetch: null, diffOf: null });
		expect(before.ledger).toMatchObject({ detail: 'config', fetch: 'diff', diffOf: 'config' });
		const read = rows(new Map([['orders', null]]));
		// The server has no diff to give, as without permission to read the model's SQL.
		expect(read.orders).toMatchObject({ detail: 'query +82 −104', diff: null, fetch: 'unavailable' });
	});

	it("lists the server's warning groups, counted and each where its warnings start, before any is read", () => {
		const warnings = planGroups(summary).find((g) => g.key === 'warnings')!;
		expect(warnings.subtitle).toBe('2 kinds');
		expect(warnings.warningGroups?.map((g) => [g.template, g.label, g.count, g.offset, g.warnings.length])).toEqual([
			['Scenario … provides fixture for unknown model …', '71 in 24 scenarios', 71, 0, 0],
			['Scenario … requires source …', '3 warnings', 3, 71, 0]
		]);
	});

	it('pages the warnings past the largest groups the summary lists as one more group, after them', () => {
		const capped = { ...summary, sections: summary.sections!.map((s) => (s.section === 'warnings' ? { ...s, total: 80 } : s)) };
		const warnings = planGroups(capped).find((g) => g.key === 'warnings')!;
		expect(warnings.subtitle).toBe('2 kinds and more');
		expect(warnings.warningGroups?.map((g) => [g.template, g.label, g.count, g.offset])).toEqual([
			['Scenario … provides fixture for unknown model …', '71 in 24 scenarios', 71, 0],
			['Scenario … requires source …', '3 warnings', 3, 71],
			['Other warnings', '6 warnings of smaller kinds', 6, 74]
		]);
	});
});
