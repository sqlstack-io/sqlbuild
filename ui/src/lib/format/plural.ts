// Counts in words, with the right singular or plural. Pure; no side effects.

/** `n` and its noun, singular for exactly one: `1 run`, `3 runs`, `1,204 nodes`; irregular plurals pass `many`. */
export function plural(n: number, one: string, many = `${one}s`): string {
	return `${n.toLocaleString('en')} ${n === 1 ? one : many}`;
}

/** Just the noun, for a count drawn apart from it, e.g. a bold number: `node` or `nodes`. */
export const noun = (n: number, one: string, many = `${one}s`) => (n === 1 ? one : many);

/**
 * Nouns for node kinds and node key prefixes, singular and plural. SQL audits and Python `@check`
 * nodes are both checks, so they are told apart as audits and Python checks, as the Checks page's
 * kinds name them.
 */
export const NODE_NOUNS: Record<string, readonly [string, string]> = {
	model: ['model', 'models'],
	source: ['source', 'sources'],
	seed: ['seed', 'seeds'],
	view: ['view', 'views'],
	table: ['table', 'tables'],
	incremental: ['incremental model', 'incremental models'],
	snapshot: ['snapshot', 'snapshots'],
	loader: ['loader', 'loaders'],
	asset: ['asset', 'assets'],
	check: ['Python check', 'Python checks'],
	python_check: ['Python check', 'Python checks'],
	audit: ['audit', 'audits'],
	sql_test: ['SQL test', 'SQL tests'],
	sql_scenario: ['scenario', 'scenarios'],
	macro: ['macro', 'macros'],
	udf: ['function', 'functions'],
	table_fn: ['table function', 'table functions'],
	sql_hook: ['SQL hook', 'SQL hooks'],
	hook: ['Python hook', 'Python hooks'],
	materialization: ['materialization', 'materializations'],
	event_exporter: ['event exporter', 'event exporters'],
	sink: ['sink', 'sinks']
};

/** The noun for a node kind or key prefix, singular or plural; an unknown kind is pluralised regularly. */
export function nodeNoun(kind: string, n: number): string {
	const [one, many] = NODE_NOUNS[kind] ?? [kind, `${kind}s`];
	return n === 1 ? one : many;
}

/** `n` nodes of a kind: `1 asset`, `407 assets`, `35 Python checks`, `4,668 audits`. */
export const nodeCount = (n: number, kind: string) => `${n.toLocaleString('en')} ${nodeNoun(kind, n)}`;
