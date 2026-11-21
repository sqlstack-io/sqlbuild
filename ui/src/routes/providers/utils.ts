import type { ProviderConnection, ProviderConsumer } from '$lib/api';
import { plural } from '$lib/format/plural';


export const consumerKindLabel: Record<ProviderConsumer['kind'], string> = {
	loader: 'loader',
	asset: 'asset',
	check: 'check',
	hook: 'hook',
	materialization: 'materialization',
	event_exporter: 'event exporter',
	sink: 'sink'
};

/** e.g. `1 loader · 1 sink · 2 jobs`, or `Not used yet`. */
export function usedBySummary(p: ProviderConnection): string {
	const kinds = new Map<ProviderConsumer['kind'], number>();
	for (const c of p.usedBy) kinds.set(c.kind, (kinds.get(c.kind) ?? 0) + 1);
	const parts = [...[...kinds].map(([kind, count]) => plural(count, consumerKindLabel[kind])), p.jobs.length ? plural(p.jobs.length, 'job') : null].filter(Boolean);
	return parts.join(' · ') || 'Not used yet';
}

/** The Used by tab's count: the code in its graph (nodes, and code without a node of its own) and the jobs. */
export function usedCount(p: ProviderConnection): number {
	const graph = usedByGraph(p);
	return graph.groups.reduce((n, g) => n + g.nodes.length, 0) + graph.others.length + graph.jobs.length;
}

// ---- the Used by graph: provider → its code grouped by kind → the jobs that run it ----

/** How a kind of node reads in a count, e.g. `1 loader`, `16 assets`. */
const KIND_WORDS: Record<string, [singular: string, plural: string]> = {
	source: ['source', 'sources'],
	loader: ['loader', 'loaders'],
	seed: ['seed', 'seeds'],
	view: ['view', 'views'],
	table: ['table', 'tables'],
	incremental: ['incremental model', 'incremental models'],
	model: ['model', 'models'],
	asset: ['asset', 'assets'],
	check: ['check', 'checks'],
	udf: ['function', 'functions'],
	table_fn: ['table function', 'table functions']
};
/** Groups in the order the Catalog lists kinds; any other kind follows, alphabetically. */
const KIND_ORDER = ['loader', 'source', 'seed', 'view', 'table', 'incremental', 'model', 'asset', 'check', 'udf', 'table_fn'];
/** Consumers that run inside other nodes or outside jobs, so they have no node of their own in the graph. */
const OTHER_CONSUMERS: ReadonlySet<ProviderConsumer['kind']> = new Set(['hook', 'materialization', 'sink', 'event_exporter']);

const kindWords = (kind: string): [string, string] => KIND_WORDS[kind] ?? [kind.replace(/_/g, ' '), `${kind.replace(/_/g, ' ')}s`];
const capitalise = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** e.g. `1 loader`, `16 assets`. */
export function kindCount(kind: string, count: number): string {
	const [one, many] = kindWords(kind);
	return `${count} ${count === 1 ? one : many}`;
}

/** The name after the key's kind, e.g. `orders` for `model:orders`. */
export const nodeName = (key: string) => key.slice(key.indexOf(':') + 1);

export interface UsedByGroup {
	kind: string;
	/** e.g. `Loaders`. */
	label: string;
	/** Node keys, sorted by name. */
	nodes: string[];
	/** Jobs that run any of these nodes. */
	jobs: string[];
}

export interface UsedByGraph {
	groups: UsedByGroup[];
	jobs: string[];
	/** Hooks, materializations, sinks and event exporters: code without a node of its own. */
	others: ProviderConsumer[];
}

/**
 * The provider's nodes grouped by kind, each with the jobs that run it. A node's kind comes from
 * `catalogKind` (the Catalog's kind, e.g. `loader`), then its key's prefix (`check:…`), then the
 * consumer of the same name.
 */
export function usedByGraph(p: Pick<ProviderConnection, 'nodes' | 'jobs' | 'jobNodes' | 'usedBy'>, catalogKind: (key: string) => string | null = () => null): UsedByGraph {
	const keys = [...new Set([...p.nodes, ...Object.values(p.jobNodes).flat()])];
	const consumerKind = new Map(p.usedBy.map((c) => [c.name, c.kind as string]));
	const kindOfKey = (key: string) => catalogKind(key) ?? (key.includes(':') ? key.slice(0, key.indexOf(':')) : (consumerKind.get(key) ?? 'node'));
	const byKind = new Map<string, string[]>();
	for (const key of keys) {
		const kind = kindOfKey(key);
		byKind.set(kind, [...(byKind.get(kind) ?? []), key]);
	}
	const rank = (kind: string) => (KIND_ORDER.includes(kind) ? KIND_ORDER.indexOf(kind) : KIND_ORDER.length);
	const groups = [...byKind]
		.sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b))
		.map(([kind, nodes]) => {
			const inGroup = new Set(nodes);
			return {
				kind,
				label: capitalise(kindWords(kind)[1]),
				nodes: [...nodes].sort((a, b) => nodeName(a).localeCompare(nodeName(b))),
				jobs: p.jobs.filter((job) => (p.jobNodes[job] ?? []).some((key) => inGroup.has(key)))
			};
		});
	return { groups, jobs: [...p.jobs], others: p.usedBy.filter((c) => OTHER_CONSUMERS.has(c.kind)) };
}

/** e.g. `orders_api is used by 2 loaders, 16 assets, run by 1 job.` */
export function usedBySentence(name: string, graph: UsedByGraph): string {
	const parts = graph.groups.map((g) => kindCount(g.kind, g.nodes.length));
	for (const c of graph.others) parts.push(`${consumerKindLabel[c.kind]} ${c.name}`);
	if (!parts.length) return `Nothing uses ${name} yet.`;
	const run = graph.jobs.length ? `, run by ${plural(graph.jobs.length, 'job')}` : ', not run by any job';
	return `${name} is used by ${parts.join(', ')}${run}.`;
}

/** A node's page: the Catalog's node page, or the Checks list searched for a check. */
export function nodeHref(key: string, kind: string): string {
	return kind === 'check' ? `/checks?q=${encodeURIComponent(nodeName(key))}` : `/catalog/${key}`;
}
