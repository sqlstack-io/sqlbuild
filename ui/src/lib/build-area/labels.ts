// Labels, tones and small mappings for the BUILD area's vocabulary. Pure; no side effects.

import Database from '@lucide/svelte/icons/database';
import Download from '@lucide/svelte/icons/download';
import Eye from '@lucide/svelte/icons/eye';
import GitMerge from '@lucide/svelte/icons/git-merge';
import Layers from '@lucide/svelte/icons/layers';
import Sprout from '@lucide/svelte/icons/sprout';
import Table2 from '@lucide/svelte/icons/table-2';
import Workflow from '@lucide/svelte/icons/workflow';
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
	task: 'Task'
};

export const nodeKindPlural: Record<NodeKind, string> = {
	source: 'Sources',
	loader: 'Loaders',
	seed: 'Seeds',
	view: 'Views',
	table: 'Tables',
	incremental: 'Incremental',
	task: 'Tasks'
};

export const NODE_KINDS: NodeKind[] = ['source', 'loader', 'seed', 'view', 'table', 'incremental', 'task'];

/** Icon key used by the lineage cards, from the node kind and materialization. */
export function nodeIconKey(node: Pick<CatalogNode, 'kind' | 'materialization'>): string {
	if (node.kind === 'incremental') return node.materialization?.includes('microbatch') ? 'microbatch' : node.materialization?.includes('merge') ? 'merge' : 'delete_insert';
	if (node.kind === 'task') return 'task';
	return node.kind;
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
	task: Workflow
};

/** `incremental · merge` style kind line, falling back to the node kind. */
export function kindLine(node: Pick<CatalogNode, 'kind' | 'materialization'>): string {
	return node.materialization ?? nodeKindLabel[node.kind].toLowerCase();
}

export const runScopePhaseLabel: Record<RunScopePhase, string> = {
	delta_and_final: 'Each new batch before it is written, then the whole table',
	final: 'Whole table, after writing'
};

export const evaluationModeLabel: Record<EvaluationMode, string> = {
	violations: 'Fails if any rows break the rule',
	measurement: 'Measures a value'
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

/** Short ID for a fingerprint run, which is a UUID or an invocation ID. */
export function shortRunId(id: string): string {
	return id.length > 12 ? id.slice(0, 8) : id;
}

/** "run-4f2a" for scheduler builds, "from the command line · 0f9a3c27" otherwise. */
export function buildSourceLabel(build: Pick<NodeBuild, 'runId' | 'fingerprintRunId'>): string {
	return build.runId ?? `from the command line · ${shortRunId(build.fingerprintRunId)}`;
}

/** Versions for the SQL viewer's drift mode, newest first. */
export function sqlVersions(builds: BuiltDefinition[], now: number): SqlVersion[] {
	return builds.map((b, i) => ({
		id: i === 0 ? 'built' : b.build.fingerprintRunId,
		label: i === 0 ? 'Last build' : b.build.runId ? `Build ${b.build.runId}` : 'Build from the command line',
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
