// The plan laid out as `sqb plan` prints it (crates/sqlbuild-core/src/cli_text), so the page and the
// terminal read the same: work that runs first, one section per change reason with each model's detail,
// then the routine models, functions, seeds and warnings.

import type { PlanDocument, PlanModel } from '$lib/api';

export type PlanEntry = {
	name: string;
	/** What happens to it: an action for a changed model, its materialization otherwise. */
	value: string;
	/** Detail lines under it, such as its cursor or the cause of the change. */
	detail: string[];
	queryDiff?: string[];
	configDiff?: string[];
	schema?: { sign: '+' | '-' | '~'; text: string }[];
};

export type PlanSection = { title: string; entries: PlanEntry[]; changed: boolean };

/** The change reasons that get their own section, in the CLI's order. */
const REASON_GROUPS: [string, string][] = [
	['full_refresh', 'Full refresh'],
	['query_changed', 'Query changed'],
	['function_changed', 'Function changed'],
	['config_changed', 'Config changed'],
	['schema_changed', 'Schema changed'],
	['first_run', 'First run'],
	['renamed', 'Renamed']
];
const ROUTINE = new Set(['no_change', 'normal_incremental']);

/** The model's materialization as the CLI labels it, such as `view` or `delete_insert (timestamp)`. */
export function materializationLabel(m: PlanModel): string {
	if (m.materialization_type !== 'incremental') return m.materialization_type;
	const strategy = m.incremental_strategy ?? 'incremental';
	return m.cursor_type ? `${strategy} (${m.cursor_type})` : strategy;
}

/** What a changed model's build does, as the CLI's `action_text` words it. */
export function actionText(m: PlanModel): string {
	if (m.action === 'create_view') return 'recreate view';
	if (m.action === 'custom') return 'run custom materialization';
	if (m.backfill.action === 'bounded' && m.backfill.duration) return `rebuild last ${m.backfill.duration}`;
	if (m.backfill.action === 'full' && m.reason !== 'first_run') return 'full rebuild';
	return 'continue forward';
}

function cursorLines(m: PlanModel, showRange: boolean): string[] {
	const column = m.cursor_column;
	if (!column) return [];
	const lines = [`cursor: ${column}${m.cursor_type ? ` (${m.cursor_type})` : ''}`];
	const requested = m.cursor?.requested_bounds;
	if (requested && (requested.start || requested.end)) lines.push(`requested: ${requested.start ?? 'earliest available'} → ${requested.end ?? 'latest available'}`);
	const range = m.cursor?.resolved_bounds ?? m.cursor_bounds;
	if (showRange && range && (range.start || range.end)) lines.push(`range: ${range.start ?? '…'} → ${range.end ?? '…'}`);
	return lines;
}

function policyLine(m: PlanModel): string[] {
	if (m.backfill.action === 'forward' || !(m.reason === 'query_changed' || m.reason === 'schema_changed')) return [];
	const value = m.backfill.action === 'bounded' ? `bounded-${m.backfill.duration ?? 'full'}` : m.backfill.action;
	return [`policy: replay_on_change=${value}`];
}

const SCHEMA_SIGNS: Record<string, { sign: '+' | '-' | '~'; label: string }> = {
	column_added: { sign: '+', label: 'added' },
	column_removed: { sign: '-', label: 'removed' },
	column_type_changed: { sign: '~', label: 'type changed' }
};

function changedEntry(m: PlanModel, reason: string): PlanEntry {
	if (reason === 'first_run') return { name: m.name, value: materializationLabel(m), detail: cursorLines(m, true) };
	return {
		name: m.name,
		value: actionText(m),
		detail: [
			...cursorLines(m, reason !== 'full_refresh'),
			...policyLine(m),
			...(m.changed_functions?.length ? [`cause: function ${m.changed_functions.join(', ')} changed`] : [])
		],
		queryDiff: m.query_diff,
		configDiff: m.config_diff,
		schema: m.schema_findings?.map((f) => {
			const kind = SCHEMA_SIGNS[f.kind] ?? { sign: '~' as const, label: f.kind };
			const types = f.kind === 'column_type_changed' ? `  ${f.expected_type ?? 'None'} → ${f.actual_type ?? 'None'}` : f.expected_type ? `  ${f.expected_type}` : '';
			return { sign: kind.sign, text: `${f.column}${types}   (${kind.label})` };
		})
	};
}

/** Whether the plan reads as one full refresh: every model being built is one, as in the CLI. */
export function isFullRefresh(plan: PlanDocument): boolean {
	const active = plan.models.filter((m) => m.action !== 'skip');
	return active.length > 0 && active.every((m) => m.reason === 'full_refresh');
}

/** The plan's sections in the order `sqb plan` prints them. Skipped models are left out, as there. */
export function planSections(plan: PlanDocument): PlanSection[] {
	const active = plan.models.filter((m) => m.action !== 'skip');
	const sections: PlanSection[] = [];
	const add = (title: string, entries: PlanEntry[], changed = false) => {
		if (entries.length) sections.push({ title, entries, changed });
	};
	// Work that runs before any model builds comes first, as in the CLI.
	add('Migrations', plan.migrations.map((r) => ({ name: r.model, value: [r.origin_model && `from ${r.origin_model}`, r.decision, r.completed_at ? 'completed' : 'pending'].filter(Boolean).join(' · '), detail: [] })), true);
	add('Column migrations', plan.column_migrations.map((r) => ({ name: r.model, value: `${r.origin_column} → ${r.destination_column} · ${r.decision}`, detail: [] })), true);
	add('Table type changes', plan.table_type_conversions.map((r) => ({ name: r.model, value: [r.actual_type, r.desired_type].filter(Boolean).join(' → '), detail: [] })), true);
	add('Retention', plan.retention.map((r) => ({ name: r.model, value: r.action ?? '', detail: [] })), true);
	for (const kind of ['loader', 'source'] as const) {
		const loads = plan.source_loads.filter((s) => s.kind === kind);
		const label = kind === 'loader' ? 'Loaders' : 'Sources';
		add(`${label} to reload`, loads.filter((s) => s.is_reload).map((s) => ({ name: s.name, value: s.write_strategy ?? '', detail: [] })));
		add(`${label} to load`, loads.filter((s) => !s.is_reload).map((s) => ({ name: s.name, value: s.write_strategy ?? '', detail: s.cursor_column ? [`cursor: ${s.cursor_column}`] : [] })));
	}
	const functionEntry = (f: PlanDocument['functions'][number]) => ({ name: f.name, value: f.language, detail: [] });
	add('Changed functions', plan.functions.filter((f) => f.reason && f.reason !== 'no_change').map(functionEntry), true);
	if (isFullRefresh(plan)) {
		const counts = new Map<string, number>();
		for (const m of active) counts.set(m.materialization_type, (counts.get(m.materialization_type) ?? 0) + 1);
		add('Full refresh', [...counts].map(([kind, n]) => ({ name: kind, value: `${n} ${n === 1 ? 'model' : 'models'}`, detail: [] })), true);
	} else {
		for (const [reason, title] of REASON_GROUPS) add(title, active.filter((m) => m.reason === reason).map((m) => changedEntry(m, reason)), true);
		add('Models', active.filter((m) => ROUTINE.has(m.reason)).map((m) => ({ name: m.name, value: materializationLabel(m), detail: cursorLines(m, true) })));
	}
	add('Functions', plan.functions.filter((f) => !f.reason || f.reason === 'no_change').map(functionEntry));
	const seedLabel: Record<string, string> = { first_run: 'first_run', config_changed: 'seed_changed', no_change: 'current' };
	add('Seeds', plan.seeds.map((s) => ({ name: s.name, value: seedLabel[s.reason] ?? '', detail: [] })));
	add('Python', plan.python_nodes.map((p) => ({ name: p.name, value: `${p.kind} · ${p.identity_status}`, detail: [] })));
	return sections;
}

/** `N selected`, as the CLI's `Plan ready` line counts it. */
export function readySummary(plan: PlanDocument): string {
	const parts = [isFullRefresh(plan) ? 'full refresh' : '', `${plan.selected_count} selected`];
	const loads = plan.source_loads.length;
	if (loads) parts.push(`${loads} source${loads === 1 ? '' : 's'} to load`);
	return parts.filter(Boolean).join(', ');
}

/** The models a plan changes, as catalog keys, for drawing just the part of a large selection that matters. */
export function changedModelKeys(plan: PlanDocument): string[] {
	return plan.models.filter((m) => m.action !== 'skip' && !ROUTINE.has(m.reason)).map((m) => `model:${m.name}`);
}
