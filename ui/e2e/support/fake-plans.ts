// The control server's plan endpoints, faked inside a browser test over `FakeControl`, so the plan page
// reads a plan through the real server client: the dry run, the plan request, its summary and its
// section pages. The summary and rows are shaped as the server's `plan_summary` and plan rows read
// write them. Lives only in the tests.

import type { Page, Route } from '@playwright/test';

/** A plan the fake serves: the summary document, and each section's rows. */
export type FakePlan = { summary: Record<string, unknown>; rows: Record<string, Record<string, unknown>[]>; selection: string[] };

/** One section page read the page asked for. */
export type RowsRead = { section: string; offset: number; limit: number; name: string | null };

const json = (route: Route, body: unknown, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

/** The name a row is found by, as the server finds it. */
function rowName(section: string, row: Record<string, unknown>): string {
	if (section === 'renamed') return `${row.name} ${row.origin_model ?? ''}`;
	if (section === 'retention') return [...((row.models as string[] | undefined) ?? []), row.name].join(' ');
	return String(row.name ?? row.model ?? '');
}

export class FakePlans {
	readonly reads: RowsRead[] = [];
	constructor(readonly plan: FakePlan) {}

	/** Answers the plan endpoints; install after `FakeControl.install`, so these routes come first. */
	async install(page: Page) {
		await page.route('**/api/control/runs/adhoc/check', (route) =>
			json(route, { verb: 'build', selection: this.plan.selection, write_set: this.plan.selection, settings: {}, permissions: [], error: null, launchable: true })
		);
		await page.route(/\/api\/control\/locations\/[^/]+\/targets\/dev\/plans(\/.*)?(\?.*)?$/, (route) => this.#answer(route));
	}

	#answer(route: Route) {
		const url = new URL(route.request().url());
		if (route.request().method() === 'POST') return json(route, { applied: true });
		if (url.pathname.endsWith('/summary')) return json(route, { expires_at: new Date(Date.now() + 600_000).toISOString(), outcome: { kind: 'plan', document: this.plan.summary }, timings: null });
		if (url.pathname.endsWith('/diff')) return json(route, { error: 'no diff' }, 404);
		if (url.pathname.endsWith('/rows')) {
			const section = url.searchParams.get('section') ?? '';
			const offset = Number(url.searchParams.get('offset') ?? 0);
			const limit = Number(url.searchParams.get('limit') ?? 50);
			const name = url.searchParams.get('name');
			this.reads.push({ section, offset, limit, name });
			const all = this.plan.rows[section] ?? [];
			const found = name ? all.filter((row) => rowName(section, row).toLowerCase().includes(name.toLowerCase())) : all;
			return json(route, { total: found.length, section_total: all.length, rows: found.slice(offset, offset + limit), runs: [], warnings: [], cursor_window_starts: {} });
		}
		return json(route, { error: 'not found' }, 404);
	}
}

/**
 * A small plan with renames, as the server summarises it: one changed model, two renamed (one keeping a
 * view at its old name), their two rename migrations, one migration into a model that exists and one
 * column rename, so two migration steps.
 */
export function renamesPlan(): FakePlan {
	const model = (name: string, reason: string, extra: Record<string, unknown> = {}) => ({ name, materialization_type: 'table', action: 'create_table', reason, backfill: { action: 'forward', duration: null }, ...extra });
	const rows = {
		query_changed: [model('orders', 'query_changed', { diff_stats: { query: { added: 2, removed: 1, whitespace_only: false } } })],
		renamed: [
			model('customers_v2', 'renamed', { origin_model: 'customers', old_name: { action: 'archive_and_view', view: 'dev.customers', reads: 'dev.customers_v2', expires_at: '2026-12-18T00:00:00Z', column_aliases: {}, grants_copied: false, reason: null } }),
			model('payments_v2', 'renamed', { origin_model: 'payments' })
		],
		migrations: [
			{ kind: 'model_migration', model: 'customers_v2', origin_model: 'customers', decision: 'renamed', completed_at: null, is_rename: true, old_name: null },
			{ kind: 'model_migration', model: 'payments_v2', origin_model: 'payments', decision: 'renamed', completed_at: null, is_rename: true, old_name: null },
			{ kind: 'model_migration', model: 'orders', origin_model: 'orders_legacy', decision: 'forced_replace', completed_at: null, is_rename: false, old_name: null }
		],
		column_migrations: [{ kind: 'column_migration', model: 'orders', origin_column: 'amt', destination_column: 'amount', decision: 'rename', completed_at: null }]
	};
	const sections = Object.entries(rows).map(([section, list]) => ({ section, total: list.length, notes: [] }));
	const empty = { models: [], seeds: [], source_loads: [], functions: [], python_nodes: [], warnings: [], migrations: [], column_migrations: [], table_type_conversions: [], retention: [], old_names: [] };
	return {
		selection: ['model:orders', 'model:customers_v2', 'model:payments_v2'],
		rows,
		summary: {
			selected_count: 3,
			source_load_count: 0,
			python_node_count: 0,
			...empty,
			synced_copy: null,
			changed: null,
			sections,
			pages: Object.fromEntries(sections.map((s) => [s.section, s.total])),
			warning_groups: [],
			migration_steps: 2
		}
	};
}
