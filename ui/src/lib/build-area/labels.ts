// Labels, tones and small mappings for the BUILD area's vocabulary. Pure; no side effects.

import Database from '@lucide/svelte/icons/database';
import Download from '@lucide/svelte/icons/download';
import Eye from '@lucide/svelte/icons/eye';
import GitMerge from '@lucide/svelte/icons/git-merge';
import Layers from '@lucide/svelte/icons/layers';
import ChartGantt from '@lucide/svelte/icons/chart-gantt';
import Repeat from '@lucide/svelte/icons/repeat';
import Package from '@lucide/svelte/icons/package';
import SquareFunction from '@lucide/svelte/icons/square-function';
import TableProperties from '@lucide/svelte/icons/table-properties';
import Sprout from '@lucide/svelte/icons/sprout';
import Table2 from '@lucide/svelte/icons/table-2';
import type {
	BuiltDefinition,
	CatalogNode,
	ChangeOp,
	CheckAttachment,
	CheckThreshold,
	EvaluationMode,
	NodeBuild,
	NodeKind,
	RunScopePhase,
	WarehouseColumnState
} from '$lib/api';
import type { SqlVersion } from '$lib/sql/versions';
import { formatAgo, formatStamp } from '$lib/format/time';

export const nodeKindLabel: Record<NodeKind, string> = {
	source: 'Source',
	loader: 'Loader',
	seed: 'Seed',
	view: 'View',
	table: 'Table',
	incremental: 'Incremental',
	asset: 'Asset',
	udf: 'Function',
	table_fn: 'Table function'
};

export const nodeKindPlural: Record<NodeKind, string> = {
	source: 'Sources',
	loader: 'Loaders',
	seed: 'Seeds',
	view: 'Views',
	table: 'Tables',
	incremental: 'Incremental',
	asset: 'Assets',
	udf: 'Functions',
	table_fn: 'Table functions'
};

export const NODE_KINDS: NodeKind[] = ['source', 'loader', 'seed', 'view', 'table', 'incremental', 'asset', 'udf', 'table_fn'];

/** The Catalog's kind tabs: SQL kinds one by one, Python nodes together, and functions together. */
export type KindTab = 'source' | 'seed' | 'view' | 'table' | 'incremental' | 'python' | 'functions';

export const KIND_TABS: KindTab[] = ['source', 'seed', 'view', 'table', 'incremental', 'python', 'functions'];

export const PYTHON_KINDS: readonly NodeKind[] = ['asset', 'loader'];
export const FUNCTION_KINDS: readonly NodeKind[] = ['udf', 'table_fn'];

export function kindTabOf(kind: NodeKind): KindTab {
	if (PYTHON_KINDS.includes(kind)) return 'python';
	if (FUNCTION_KINDS.includes(kind)) return 'functions';
	return kind as KindTab;
}

export const kindTabLabel: Record<KindTab, string> = {
	source: 'Sources',
	seed: 'Seeds',
	view: 'Views',
	table: 'Tables',
	incremental: 'Incremental',
	python: 'Python',
	functions: 'Functions'
};

/** A tab from a link, which may name a kind, such as `loader`, or a tab, such as `python`. */
export function parseKindTab(value: string | null): KindTab | null {
	if (!value) return null;
	if ((KIND_TABS as string[]).includes(value)) return value as KindTab;
	return (NODE_KINDS as string[]).includes(value) ? kindTabOf(value as NodeKind) : null;
}

/** Icon key used by the lineage cards, from the node kind and materialization. */
export function nodeIconKey(node: Pick<CatalogNode, 'kind' | 'materialization'>): string {
	if (node.kind === 'incremental') return node.materialization?.includes('microbatch') ? 'microbatch' : node.materialization?.includes('merge') ? 'merge' : 'delete_insert';
	return node.kind;
}

/** Mockup (`?cards=new`): icons that tell every incremental strategy apart, as the graph's cards draw them. */
export function nodeIconsFor(cardsNew: boolean): Record<string, typeof Table2> {
	return cardsNew ? { ...nodeIcon, delete_insert: Repeat, microbatch: ChartGantt } : nodeIcon;
}

export const nodeIcon: Record<string, typeof Table2> = {
	source: Database,
	loader: Download,
	seed: Sprout,
	view: Eye,
	table: Table2,
	incremental: Layers,
	merge: GitMerge,
	delete_insert: Layers,
	microbatch: Layers,
	asset: Package,
	udf: SquareFunction,
	table_fn: TableProperties
};

/** `incremental · merge` style kind line, falling back to the node kind. */
export function kindLine(node: Pick<CatalogNode, 'kind' | 'materialization'>): string {
	return node.materialization ?? nodeKindLabel[node.kind].toLowerCase();
}

/** What a check looks at, as a short fact; `runScopePhaseHelp` says when. */
export const runScopePhaseLabel: Record<RunScopePhase, string> = {
	delta_and_final: 'Each batch + full table',
	final: 'Full table'
};

export const runScopePhaseHelp: Record<RunScopePhase, string> = {
	delta_and_final: "Each new batch is checked before it's written, then the whole table",
	final: 'Checked against the whole table after the build writes it'
};

/** How a check decides its result, as a short fact; `evaluationModeHelp` explains it. */
export const evaluationModeLabel: Record<EvaluationMode, string> = {
	violations: 'Any failing row',
	measurement: 'Measured value'
};

export const evaluationModeHelp: Record<EvaluationMode, string> = {
	violations: 'Fails when any row breaks the rule',
	measurement: 'Compares a measured value with thresholds'
};

export const attachmentLabel: Record<CheckAttachment, string> = {
	model: 'Model',
	source: 'Source',
	seed: 'Seed',
	end: 'End of run'
};

export function thresholdLabel(t: CheckThreshold): string {
	const value = Array.isArray(t.value) ? `${t.value[0]}–${t.value[1]}` : String(t.value);
	return `${t.level} ${t.operator} ${value}`;
}

export const changeOpLabel: Record<ChangeOp, string> = {
	added: 'Added',
	removed: 'Removed',
	changed: 'Changed',
	renamed: 'Renamed'
};

export const changeOpTone: Record<ChangeOp, string> = {
	added: 'var(--sb-success)',
	removed: 'var(--sb-error)',
	changed: 'var(--sb-stale)',
	renamed: 'var(--primary)'
};

export const warehouseColumnLabel: Record<WarehouseColumnState, string> = {
	match: 'Matches',
	type_differs: 'Type differs',
	missing: 'Missing in warehouse',
	undeclared: 'Not declared'
};

export const warehouseColumnTone: Record<WarehouseColumnState, string> = {
	match: 'var(--sb-success)',
	type_differs: 'var(--sb-warning)',
	missing: 'var(--sb-stale)',
	undeclared: 'var(--sb-text-faint)'
};

/** A UUID's first block, or the random suffix of an `sqb` run id such as `20261107T061043Z_ea9122fb7f42`. */
export function shortRunId(id: string): string {
	const suffix = id.slice(id.lastIndexOf('_') + 1);
	return suffix.length > 12 ? suffix.slice(0, 8) : suffix;
}

/** "run-4f2a" for scheduler builds, "from the command line · 0f9a3c27" otherwise. */
export function buildSourceLabel(build: Pick<NodeBuild, 'runId' | 'fingerprintRunId'>): string {
	return build.runId ?? `CLI · ${shortRunId(build.fingerprintRunId)}`;
}

/** Versions for the SQL viewer's drift mode, newest first. */
export function sqlVersions(builds: BuiltDefinition[], now: number): SqlVersion[] {
	return builds.map((b, i) => ({
		id: i === 0 ? 'built' : b.build.fingerprintRunId,
		label: i === 0 ? 'Last build' : b.build.runId ? `Build ${b.build.runId}` : 'CLI build',
		detail: `${i === 0 || !b.build.runId ? `${buildSourceLabel(b.build)} · ` : ''}${formatAgo(b.build.builtAt, now)} · ${formatStamp(b.build.builtAt)}`,
		raw: b.raw,
		compiled: b.compiled
	}));
}

export function formatCredits(value: number): string {
	return `${value < 0.01 ? value.toFixed(4) : value.toFixed(3)} cr`;
}

export function formatNumber(value: number | null | undefined): string {
	return value == null ? '—' : value.toLocaleString('en-US');
}

export function formatBytes(bytes: number | null | undefined): string {
	if (bytes == null) return '—';
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	let v = bytes;
	let i = 0;
	while (v >= 1024 && i < units.length - 1) {
		v /= 1024;
		i += 1;
	}
	return `${v < 10 && i ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function formatMs(ms: number | null | undefined): string {
	if (ms == null) return '—';
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60_000) return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)}s`;
	return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

/** Relation as `DATABASE.SCHEMA.NAME`. */
export function relationName(relation: { database: string | null; schema: string; name: string } | null): string {
	if (!relation) return '—';
	return [relation.database, relation.schema, relation.name].filter(Boolean).join('.');
}

/** A node key without its kind, e.g. `customer_summary` for `model:customer_summary`. */
export const shortName = (key: string) => key.slice(key.indexOf(':') + 1);
