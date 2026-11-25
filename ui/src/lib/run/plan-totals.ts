// What a build plan runs and what it doesn't, in nodes, for the plan index's Will run and Won't run
// headings (CHI-1563). Counted from the same sections the index shows, without counting a node twice:
// - each model is in exactly one model section (by its plan reason), each seed in one seed section;
// - "Unchanged" models (routine_models) still build: a table is rebuilt, an incremental runs, and under
//   `--changed` only those `--changed` runs are listed there; what it skips is `unchanged`;
// - migrations, column migrations, table type conversions and retention are steps on models that are
//   already counted in a model section, so the "Renames and migrations" group counts renamed models as
//   its nodes and the rest as migration steps; a rename's own migration is not a step, as the renamed
//   model's row says it;
// - functions are not nodes the build runs, so their sections count no nodes;
// - loads and Python nodes run; `--changed` decides only models and seeds.
// Pure.

import type { PlanDocument, PlanPageSection, PlanSectionCount } from '$lib/api';
import { sectionCounts } from './plan-index';
import { changedSections, planSections } from './plan-text';

/** Sections whose rows are nodes the build runs. */
const RUNS = new Set<PlanPageSection>([
	'full_refresh',
	'query_changed',
	'function_changed',
	'config_changed',
	'schema_changed',
	'first_run',
	'renamed',
	'routine_models',
	'first_run_seeds',
	'changed_seeds',
	'unchanged_seeds',
	'loader_reloads',
	'loader_loads',
	'source_reloads',
	'source_loads',
	'python_nodes'
]);
/** Sections whose rows are selected nodes the build leaves alone. */
const WONT = new Set<PlanPageSection>(['planning_error', 'window_before_cursor_start', 'held_back', 'unchanged']);

/** One part of a total: a label, its nodes, and the plan index group it opens. */
export type TotalPart = { key: string; group: string; label: string; count: number };

export type PlanTotals = {
	/** Nodes the build runs. */
	run: number;
	/** Selected nodes it doesn't: models it can't plan, held back and skipped by `--changed`. */
	wont: number;
	runParts: TotalPart[];
	wontParts: TotalPart[];
	/** Migration steps on nodes counted above, which the Renames group adds to its count. */
	steps: number;
	warnings: number;
};

const PARTS: { key: string; group: string; label: string; sections: PlanPageSection[] }[] = [
	{ key: 'full_refresh', group: 'full_refresh', label: 'full refresh', sections: ['full_refresh'] },
	{ key: 'changed', group: 'changed', label: 'changed', sections: ['query_changed', 'function_changed', 'config_changed', 'schema_changed', 'changed_seeds'] },
	{ key: 'renamed', group: 'renames', label: 'renamed', sections: ['renamed'] },
	{ key: 'first_run', group: 'first_run', label: 'first run', sections: ['first_run', 'first_run_seeds'] },
	{ key: 'loads', group: 'loads', label: 'loads', sections: ['loader_reloads', 'loader_loads', 'source_reloads', 'source_loads'] },
	{ key: 'python', group: 'python', label: 'Python', sections: ['python_nodes'] },
	{ key: 'unchanged', group: 'unchanged', label: 'unchanged', sections: ['routine_models', 'unchanged_seeds'] },
	{ key: 'errors', group: 'errors', label: 'planning failed', sections: ['planning_error'] },
	{ key: 'window', group: 'window', label: 'before cursor_start', sections: ['window_before_cursor_start'] },
	{ key: 'held_back', group: 'held_back', label: 'held back', sections: ['held_back'] },
	{ key: 'skipped', group: 'skipped', label: 'skipped by --changed', sections: ['unchanged'] }
];

/** The plan's section counts: the server summary's, else counted from the mock's whole plan's rows. */
export function planSectionCounts(plan: PlanDocument): PlanSectionCount[] {
	if (plan.sections) return plan.sections;
	return sectionCounts(plan, [...planSections(plan), ...(plan.changed ? changedSections(plan.changed, (job) => job, plan.pages) : [])]);
}

/** The plan's totals in nodes, from its section counts, and its migration steps. */
export function planTotals(counts: PlanSectionCount[], steps: number): PlanTotals {
	const of = (sections: PlanPageSection[]) => counts.filter((c) => sections.includes(c.section)).reduce((n, c) => n + c.total, 0);
	const sum = (keep: Set<PlanPageSection>) => counts.filter((c) => keep.has(c.section)).reduce((n, c) => n + c.total, 0);
	const parts = PARTS.map((p) => ({ key: p.key, group: p.group, label: p.label, count: of(p.sections) })).filter((p) => p.count > 0);
	const wontKeys = new Set(['errors', 'window', 'held_back', 'skipped']);
	return {
		run: sum(RUNS),
		wont: sum(WONT),
		runParts: parts.filter((p) => !wontKeys.has(p.key)),
		wontParts: parts.filter((p) => wontKeys.has(p.key)),
		steps,
		warnings: of(['warnings'])
	};
}
