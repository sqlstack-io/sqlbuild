// The plan laid out as `sqb plan` prints it (crates/sqlbuild-core/src/cli_text), so the page and the
// terminal read the same: work that runs first, one section per change reason with each model's detail,
// then the routine models, functions, seeds and warnings.

import type { PlanChanged, PlanDocument, PlanModel, Trigger } from '$lib/api';
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
	return sections;
}

/** The `--changed` run of each model, by name. */
const changedRuns = (plan: PlanDocument) => new Map((plan.changed?.runs ?? []).map((r) => [keyName(r.node), r]));

const count = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** Names a few models, or counts them when there are more than two. */
const few = (models: PlanModel[], noun: string) => (models.length <= 2 ? models.map((m) => m.name).join(' and ') : count(models.length, noun));

/**
 * What the run does: how many models build, which changed since their last build, which build for the
 * first time, which take new rows and that the rest are recreated as usual, then what else happens.
 */
export function planSentence(plan: PlanDocument): string {
	const active = builtModels(plan);
	// The planner skips only disabled models; seeds it reports as current don't load.
	const skipped = plan.models.filter((m) => m.action === 'skip').length;
	const held = plan.changed?.held_back.length ?? 0;
	const unchanged = plan.changed?.unchanged.length ?? 0;
	const seeds = builtSeeds(plan).filter((s) => s.reason !== 'no_change').length;
	const functions = plan.functions.filter((f) => f.reason && f.reason !== 'no_change').length;
	const runs = [
		plan.source_loads.length && count(plan.source_loads.length, 'source loads first', 'sources load first'),
		seeds && count(seeds, 'seed loads', 'seeds load'),
		functions && count(functions, 'changed function is recreated', 'changed functions are recreated'),
		plan.python_nodes.length && count(plan.python_nodes.length, 'Python node runs', 'Python nodes run')
	].filter((p): p is string => !!p);
	const after = [
		...runs,
		...(skipped ? [count(skipped, 'disabled model is skipped', 'disabled models are skipped')] : []),
		...(held ? [`${count(held, 'model waits', 'models wait')} for ${held === 1 ? 'its' : 'their'} job's own run`] : []),
		...(unchanged ? [`${count(unchanged, 'unchanged node is', 'unchanged nodes are')} skipped by --changed`] : [])
	]
		.map((p) => ` ${p}.`)
		.join('');
	if (!active.length) return `${runs.length ? 'No models build.' : 'Nothing to build.'}${after}`;
	const builds = count(active.length, 'model builds', 'models build');
	if (isFullRefresh(plan)) return `${builds} from scratch, replacing what was built.${after}`;
	const changed = active.filter((m) => !ROUTINE.has(m.reason) && m.reason !== 'first_run');
	const fresh = active.filter((m) => m.reason === 'first_run');
	// Under --changed a routine model runs because something upstream changed or for its own reason, never just to take new rows.
	const why = changedRuns(plan);
	const own = plan.changed ? active.filter((m) => ROUTINE.has(m.reason) && !why.get(m.name)?.downstream_of) : [];
	const rows = plan.changed ? [] : active.filter((m) => m.reason === 'normal_incremental');
	const usual = active.filter((m) => (plan.changed ? ROUTINE.has(m.reason) && !!why.get(m.name)?.downstream_of : m.reason === 'no_change'));
	const parts = [
		changed.length && `${few(changed, 'model')} changed since ${changed.length === 1 ? 'its' : 'their'} last build`,
		fresh.length && `${few(fresh, 'model')} ${fresh.length === 1 ? 'builds' : 'build'} for the first time`,
		own.length && `${few(own, 'model')} ${own.length === 1 ? 'runs for its' : 'run for their'} own reason`,
		rows.length && `${count(rows.length, 'incremental model takes', 'incremental models take')} new rows`
	].filter((p): p is string => !!p);
	if (usual.length && plan.changed) parts.push(`${parts.length ? 'the other ' : ''}${count(usual.length, 'model reads', 'models read')} a change upstream`);
	else if (usual.length) parts.push(parts.length ? `the other ${usual.length} ${usual.length === 1 ? 'is' : 'are'} recreated as usual` : `none changed since ${usual.length === 1 ? 'its' : 'their'} last build`);
	const detail = parts.length > 1 ? `${parts.slice(0, -1).join(', ')} and ${parts.at(-1)}` : parts[0];
	return `${builds}: ${detail}.${after}`;
}

/** The models a plan changes, as catalog keys, for drawing just the part of a large selection that matters. */
export function changedModelKeys(plan: PlanDocument): string[] {
	return builtModels(plan).filter((m) => !ROUTINE.has(m.reason)).map((m) => `model:${m.name}`);
}

/** The line `sqb plan --changed` opens with: what counts as a change and how far a change reaches. */
export function changedPolicyLine(changed: PlanChanged): string {
	const label = { definition: 'definition', source_data: 'source data' } as const;
	const uncounted = (['definition', 'source_data'] as const).filter((kind) => !changed.counts.includes(kind)).map((kind) => ` (${label[kind]} not counted)`);
	return `Counting: ${changed.counts.map((kind) => label[kind]).join(', ')}${uncounted.join('')} · reach: ${changed.reach}`;
}

/** The Held back and Unchanged sections of a `--changed` plan; `refresh` words what refreshes a job's nodes. */
export function changedSections(changed: PlanChanged, refresh: (job: string) => string): PlanSection[] {
	const sections: PlanSection[] = [];
	const held = changed.held_back.map((h) => ({ name: keyName(h.node), value: 'refreshed_by' in h ? h.refreshed_by.map(refresh).join(' · ') : `reads ${keyName(h.reads)}, which is held back`, detail: [] }));
	if (held.length) sections.push({ title: 'Held back', entries: held, changed: false });
	if (changed.unchanged.length) sections.push({ title: 'Unchanged', entries: changed.unchanged.map((key) => ({ name: keyName(key), value: 'skipped by --changed', detail: [] })), changed: false });
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
