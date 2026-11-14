// The plan laid out as `sqb plan` prints it (crates/sqlbuild-core/src/cli_text), so the page and the
// terminal read the same: work that runs first, one section per change reason with each model's detail,
// then the routine models, functions, seeds and warnings.

import type { PlanChanged, PlanDocument, PlanModel, PlanWarning, Trigger } from '$lib/api';
import { formatIn } from '$lib/format/time';

export type PlanEntry = {
	name: string;
	/** What happens to it: an action for a changed model, its materialization otherwise. */
	value: string;
	/** Detail lines under it, such as its cursor or the cause of the change. */
	detail: string[];
	queryDiff?: string[];
	configDiff?: string[];
	schema?: { sign: '+' | '-' | '~'; text: string }[];
	/** The plan's warnings about this model, such as the CLI's forward-only replay warning, shown with it. */
	warnings?: PlanWarning[];
};

/** `collapsed` sections show one line, such as "4 unchanged, skipped by --changed", until opened. */
export type PlanSection = { title: string; entries: PlanEntry[]; changed: boolean; collapsed?: boolean };

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

/** A node key such as `model:orders` by the name `sqb plan` prints. */
const keyName = (key: string) => key.slice(key.indexOf(':') + 1);

/** The models the run builds: none the planner skips, and under `--changed` only those it runs. */
export function builtModels(plan: PlanDocument): PlanModel[] {
	return plan.models.filter((m) => m.action !== 'skip' && runsUnderChanged(plan, `model:${m.name}`));
}

/** Without `--changed` every planned node runs; with it, only those its decision runs. */
function runsUnderChanged(plan: PlanDocument, key: string): boolean {
	return !plan.changed || plan.changed.runs.some((r) => r.node === key);
}

/** The seeds the run loads or reports on: under `--changed`, only those it runs. */
export const builtSeeds = (plan: PlanDocument) => plan.seeds.filter((s) => runsUnderChanged(plan, `seed:${s.name}`));

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

/** The cursor lines; a preview's `windowStart` stands in for a range it could not bound. */
function cursorLines(m: PlanModel, showRange: boolean, windowStart?: string): string[] {
	const column = m.cursor_column;
	if (!column) return [];
	const lines = [`cursor: ${column}${m.cursor_type ? ` (${m.cursor_type})` : ''}`];
	const requested = m.cursor?.requested_bounds;
	if (requested && (requested.start || requested.end)) lines.push(`requested: ${requested.start ?? 'earliest available'} → ${requested.end ?? 'latest available'}`);
	const range = m.cursor?.resolved_bounds ?? m.cursor_bounds;
	if (showRange && range && (range.start || range.end)) lines.push(`range: ${range.start ?? '…'} → ${range.end ?? '…'}`);
	else if (showRange && windowStart) lines.push(`range: ${windowStart} → latest data when the run starts`);
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

function changedEntry(m: PlanModel, reason: string, windowStart?: string): PlanEntry {
	if (reason === 'first_run') return { name: m.name, value: materializationLabel(m), detail: cursorLines(m, true, windowStart) };
	return {
		name: m.name,
		value: actionText(m),
		detail: [
			...cursorLines(m, reason !== 'full_refresh', windowStart),
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
	const active = builtModels(plan);
	return active.length > 0 && active.every((m) => m.reason === 'full_refresh');
}

/** The plan's sections in the order `sqb plan` prints them. Skipped models are left out, as there. */
export function planSections(plan: PlanDocument): PlanSection[] {
	const active = builtModels(plan);
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
		const starts = plan.synced_copy?.cursor_window_starts ?? {};
		for (const [reason, title] of REASON_GROUPS) add(title, active.filter((m) => m.reason === reason).map((m) => changedEntry(m, reason, starts[m.name])), true);
		// Under --changed a routine model runs because something it reads changed, or for its own reason, such as no completed record.
		const why = changedRuns(plan);
		add('Models', active.filter((m) => ROUTINE.has(m.reason)).map((m) => {
			const run = why.get(m.name);
			return { name: m.name, value: materializationLabel(m), detail: [...cursorLines(m, true, starts[m.name]), ...(run ? [run.downstream_of ? `reads ${keyName(run.downstream_of)}` : run.reason] : [])] };
		}));
	}
	add('Functions', plan.functions.filter((f) => !f.reason || f.reason === 'no_change').map(functionEntry));
	const seedLabel: Record<string, string> = { first_run: 'first_run', config_changed: 'seed_changed', no_change: 'current' };
	add('Seeds', builtSeeds(plan).map((s) => ({ name: s.name, value: seedLabel[s.reason] ?? '', detail: [] })));
	add('Python', plan.python_nodes.map((p) => ({ name: p.name, value: `${p.kind} · ${p.identity_status}`, detail: [] })));
	for (const entry of sections.filter((s) => s.changed).flatMap((s) => s.entries)) {
		// A planner warning names its model in `model_name`, or opens its message with it as the CLI prints it.
		const mine = plan.warnings.filter((w) => (w.model_name ?? w.message.split(':', 1)[0]) === entry.name);
		if (mine.length) entry.warnings = mine;
	}
	return sections;
}

/** The forward-only replay warning, `sqb plan`'s long advice in a sentence and the ways to reprocess. */
const FORWARD_ONLY = /already-built dates keep their old results/;
export const REPROCESS_OPTIONS = [
	'replay_on_change full: rebuild every date with the new SQL',
	'replay_on_change bounded-<duration>, such as bounded-14d: reprocess just that window',
	'Leave it forward-only, the default: existing dates stay as they are'
];

/** A warning shown under its model: one sentence, and for the forward-only warning, how to reprocess. */
export function warningView(w: PlanWarning): { text: string; reprocess: boolean } {
	if (FORWARD_ONLY.test(w.message)) return { text: 'Already-built dates keep their old results; only new dates use the new query.', reprocess: true };
	const text = w.model_name ? w.message : w.message.slice(w.message.indexOf(':') + 1).trim();
	return { text: text.charAt(0).toUpperCase() + text.slice(1), reprocess: false };
}

/** The warnings no changed model shows with it, listed on their own after the plan. */
export function looseWarnings(plan: PlanDocument, sections: PlanSection[]): PlanWarning[] {
	const shown = new Set(sections.flatMap((s) => s.entries.flatMap((e) => e.warnings ?? [])));
	return plan.warnings.filter((w) => !shown.has(w));
}

/** The `--changed` run of each model, by name. */
const changedRuns = (plan: PlanDocument) => new Map((plan.changed?.runs ?? []).map((r) => [keyName(r.node), r]));

/** The models a plan changes, as catalog keys, for drawing just the part of a large selection that matters. */
export function changedModelKeys(plan: PlanDocument): string[] {
	return builtModels(plan).filter((m) => !ROUTINE.has(m.reason)).map((m) => `model:${m.name}`);
}

/** What `--changed` counted as a change and how far a change reaches, the CLI's `Counting: … · reach: …` line in words. */
export function changedRule(changed: PlanChanged): string {
	const counted = changed.counts.includes('source_data')
		? changed.counts.includes('definition')
			? 'definition and source data changes'
			: 'source data changes, not definition changes'
		: 'definition changes, not source data';
	const reach = changed.reach === 'all' ? 'builds everything downstream of a change' : 'stops at models another scheduled job refreshes';
	return `--changed counts ${counted}, and ${reach}.`;
}

/** What a preview couldn't know, in words; the run checks each against the warehouse before it builds. */
const UNKNOWN_WORDS: Record<string, string> = {
	'source freshness': 'whether sources are fresh enough',
	retention: 'what retention removes',
	'column rename support': 'whether the warehouse can rename columns in place',
	'sources outside mirrored schemas': 'sources in schemas the server doesn’t sync',
	'table types': 'what type each existing table is'
};
export const unknownWords = (item: string) => UNKNOWN_WORDS[item] ?? item;

/** The Held back and Unchanged sections of a `--changed` plan; `refresh` words what refreshes a job's nodes. */
export function changedSections(changed: PlanChanged, refresh: (job: string) => string): PlanSection[] {
	const sections: PlanSection[] = [];
	const held = changed.held_back.map((h) => ({ name: keyName(h.node), value: 'refreshed_by' in h ? h.refreshed_by.map(refresh).join(' · ') : `reads ${keyName(h.reads)}, which is held back`, detail: [] }));
	if (held.length) sections.push({ title: 'Held back', entries: held, changed: false });
	if (changed.unchanged.length) sections.push({ title: 'Unchanged', entries: changed.unchanged.map((key) => ({ name: keyName(key), value: 'skipped by --changed', detail: [] })), changed: false, collapsed: true });
	return sections;
}

/** What refreshes a node held back for `job` (a job or group the plan names): its next scheduled run, its sensor, or that it is paused. */
export function jobRefreshText(job: string, triggers: Pick<Trigger, 'jobKey' | 'type' | 'state' | 'nextDueAt'>[], now: number): string {
	const mine = triggers.filter((t) => t.jobKey === job);
	const live = mine.filter((t) => t.state === 'running');
	if (mine.length && !live.length) return `job ${job} is paused`;
	const next = live.flatMap((t) => (t.nextDueAt ? [t.nextDueAt] : [])).sort()[0];
	if (next) return `job ${job} · next run ${formatIn(next, now)}`;
	return live.some((t) => t.type === 'sensor') ? `job ${job} · when its sensor fires` : `job ${job}`;
}
