import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanModel } from '$lib/api';
import { configChange, cursorWords, diffPreview, diffStats, groupWarnings, planGroups, planSummaryLine, selectedGroup, sharedNote, whitespaceOnly, type PlanRow } from './plan-index';

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

describe('config changes in one line', () => {
	it('reads up to two changed keys of the config JSON', () => {
		expect(configChange([...head, '-  "full_refresh": true,', '+  "full_refresh": false,'])).toBe('full_refresh true → false');
		expect(configChange(['-  "cursor": "observed_at",', '+  "cursor": "event_date",', '+  "grain": "day"'])).toBe('cursor observed_at → event_date; grain (unset) → day');
	});

	it('leaves more keys, or a nested value, to the diff', () => {
		expect(configChange(['-  "a": 1,', '+  "a": 2,', '-  "b": 1,', '+  "b": 2,', '-  "c": 1,', '+  "c": 2,'])).toBeNull();
		expect(configChange(['+  "meta": {'])).toBeNull();
	});
});

describe('cursor ranges in words', () => {
	it("words the executor's bound placeholders", () => {
		expect(cursorWords('range: __SQB_CURSOR_START__ → __SQB_CURSOR_END__')).toBe('range: from the last run → the latest data when the run starts');
		expect(cursorWords('range: 2026-11-01 → __SQB_CURSOR_END__')).toBe('range: 2026-11-01 → the latest data when the run starts');
	});
});

describe('notes every row shares', () => {
	const row = (name: string, detail: string, notes: string[] = []): PlanRow => ({ name, action: 'table', detail, notes, diff: null, diffOf: null, warnings: [], linked: true });

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
			['Scenario … provides fixture for unknown model ….', '6 in 2 scenarios', 6],
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
			model('refunds', 'config_changed', { config_diff: ['-  "full_refresh": true,', '+  "full_refresh": false,'] }),
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
		expect(planSummaryLine(groups)).toBe('3 changed, 1 new, 1 Python · 1 unchanged');
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

describe('an opened diff', () => {
	it('shows the first changed lines, then says the rest is cut off', () => {
		const diff = [{ text: '@@', cls: 'hunk' as const }, ...Array.from({ length: 20 }, (_, i) => ({ text: `+${i}`, cls: 'add' as const }))];
		const preview = diffPreview(diff, 12);
		expect(preview.lines).toHaveLength(13);
		expect(preview.cut).toBe(true);
		expect(diffPreview(diff.slice(0, 5), 12)).toEqual({ lines: diff.slice(0, 5), cut: false });
	});
});
