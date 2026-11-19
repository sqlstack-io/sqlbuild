// The plan's renames and migration steps in words, for the Renames and migrations section: each renamed
// model as old name → new name, and each step (a model migration, column rename, table type change or
// retention change) naming the node it acts on and what it does. A row says only what its heading
// doesn't: the usual outcome of its kind goes unsaid, the exceptions are said. Pure.

import type { PlanColumnMigration, PlanDocument, PlanMigration, PlanModel, PlanPageSection, PlanRetention, PlanTableTypeConversion } from '$lib/api';

/** A renamed model: its old name when known, its new name, and anything unusual, such as what stays at the old name. */
export type RenamedRow = { from: string | null; to: string; detail: string[] };
/** One migration step: the node(s) it acts on, what it does, and any warning. */
export type StepRow = { node: string; what: string; detail: string[]; warning: string | null };

/** The migration step sections, in the order they run, with their headings. */
export const STEP_KINDS: { section: PlanPageSection; title: string }[] = [
	{ section: 'migrations', title: 'Data moved between models' },
	{ section: 'column_migrations', title: 'Column renames' },
	{ section: 'table_type_conversions', title: 'Table type changes' },
	{ section: 'retention', title: 'Time travel retention' }
];

const day = (n: number | null | undefined) => (n === null || n === undefined ? '?' : `${n} ${n === 1 ? 'day' : 'days'}`);
const when = (iso: string | null | undefined) => (iso ? iso.slice(0, 10) : null);

/** What happens at a moved model's old name. */
function oldNameWords(old: PlanMigration['old_name']): string | null {
	if (!old) return null;
	const until = when(old.expires_at);
	switch (old.action) {
		case 'archive_and_view':
			return `old table archived; a view at the old name${until ? ` until ${until}` : ''}`;
		case 'view_only':
			return `a view at the old name${until ? ` until ${until}` : ''}`;
		case 'live':
			return 'the old name stays live';
		default:
			return null;
	}
}

/** A renamed model as a row: its old name, and what stays at the old name, both from its own row. */
export function renamedRow(model: PlanModel): RenamedRow {
	const detail = [model.query_changed ? 'query changed too' : null, oldNameWords(model.old_name)].filter((d): d is string => !!d);
	return { from: model.origin_model ?? null, to: model.name, detail };
}

/** What a model migration does to its model, by its decision. */
function migrationWords(m: PlanMigration): string {
	const from = m.origin_model ?? 'its old name';
	switch (m.decision) {
		case 'migrate':
			return `takes the data of ${from}`;
		case 'redo':
			return `takes the data of ${from} again`;
		case 'superseded_replace':
			return `replaced by the data of ${from}, which was renamed back`;
		case 'forced_replace':
			return `replaced by the data of ${from} (forced)`;
		case 'conflict':
			return `not moved: ${from} and this model both hold data`;
		case 'origin_missing':
			return `nothing to move: ${from} is not in the warehouse`;
		case 'done':
			return `already took the data of ${from}`;
		case 'renamed':
			return `renamed from ${from}`;
		default:
			return `${m.decision} from ${from}`;
	}
}

/** A column rename's outcome when it is not the usual rename. */
const COLUMN: Record<string, string> = {
	record: 'already renamed; recorded',
	done: 'already done',
	conflict: 'not renamed: both columns exist',
	source_missing: 'not renamed: the old column is missing',
	still_produced: 'not renamed: the query still produces the old column',
	unsupported: 'not renamed: not supported here'
};

const DIRECTION: Record<string, string> = { increase: 'raised', decrease: 'lowered', mixed: 'changed', match: 'unchanged', apply_after_create: 'set after the table is created' };

/** A model migration step. */
export function migrationStep(m: PlanMigration): StepRow {
	return { node: m.model, what: migrationWords(m), detail: [m.compatibility === 'incompatible' ? 'incompatible columns' : null, m.completed_at ? 'completed' : null].filter((d): d is string => !!d), warning: null };
}

/** A column rename step. */
export function columnStep(c: PlanColumnMigration): StepRow {
	const unusual = c.decision === 'rename' ? null : (COLUMN[c.decision] ?? c.decision);
	return { node: c.model, what: `${c.origin_column} → ${c.destination_column}`, detail: unusual ? [unusual] : [], warning: null };
}

/** A table type change step. */
export function tableTypeStep(t: PlanTableTypeConversion): StepRow {
	// A downgrade's warning says what it loses; one without a warning is still said.
	return { node: t.model, what: `${t.actual_type ?? '?'} → ${t.desired_type ?? '?'}`, detail: t.downgrade && !t.irreversible_warning ? ['downgrade'] : [], warning: t.irreversible_warning ?? null };
}

/** A retention step: the models whose relation it changes, and the days before and after. */
export function retentionStep(r: PlanRetention): StepRow {
	const models = r.models ?? [];
	const node = r.scope === 'schema' ? `schema ${r.name ?? '?'}` : models.length ? models.join(', ') : (r.name ?? '?');
	const direction = r.direction ? (DIRECTION[r.direction] ?? r.direction) : 'changed';
	const what = r.direction === 'match' ? `stays ${day(r.desired_days)}` : `${direction}: ${day(r.actual_days)} → ${day(r.desired_days)}`;
	const detail = r.scope === 'schema' && models.length ? [`for ${models.length === 1 ? models[0] : `${models.length} models`}`] : [];
	return { node, what, detail, warning: r.irreversible_warning ?? null };
}

/** Whether a model migration is a step: a rename's own migration is said by its Renamed row instead. */
export const isStep = (m: PlanMigration) => !m.is_rename;

/** A step section's rows, as a page brings them, in words; a rename's own migration is not a step. */
export function stepRowsOf(section: PlanPageSection, rows: unknown[]): StepRow[] {
	switch (section) {
		case 'migrations':
			return (rows as PlanMigration[]).filter(isStep).map(migrationStep);
		case 'column_migrations':
			return (rows as PlanColumnMigration[]).map(columnStep);
		case 'table_type_conversions':
			return (rows as PlanTableTypeConversion[]).map(tableTypeStep);
		case 'retention':
			return (rows as PlanRetention[]).map(retentionStep);
		default:
			return [];
	}
}

/**
 * The plan's migration steps: the server summary's count (every summary has one), else counted from the
 * rows of a whole plan, which only the default mock serves.
 */
export function migrationSteps(plan: PlanDocument): number {
	if (plan.sections) return plan.migration_steps ?? 0;
	return plan.migrations.filter(isStep).length + plan.column_migrations.length + plan.table_type_conversions.length + plan.retention.length;
}

/** A step section's loaded rows, in words. */
export function stepRows(plan: PlanDocument, section: PlanPageSection): StepRow[] {
	const lists: Partial<Record<PlanPageSection, unknown[]>> = { migrations: plan.migrations, column_migrations: plan.column_migrations, table_type_conversions: plan.table_type_conversions, retention: plan.retention };
	return stepRowsOf(section, lists[section] ?? []);
}
