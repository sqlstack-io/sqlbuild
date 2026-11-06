import type { Edge, Node } from '@xyflow/svelte';

// Synthetic warehouse-shaped lineage for scale testing (?models=N on /lineage).
// Deterministic: the same N always produces the same graph. Models carry the
// same fields as the catalog-backed lineage cards (kind, icon, checks, stale,
// owner, tags, ...) so the real cards render in full.

const LAYERS = ['raw', 'stg', 'int', 'int', 'fct', 'dim', 'mart', 'rpt'] as const;
// share of models per layer (sums to 1)
const LAYER_SHARE = [0.12, 0.2, 0.16, 0.12, 0.14, 0.08, 0.12, 0.06];
const MODELS_PER_DOMAIN = 120;
const DOMAIN_NAMES = [
	'orders', 'customers', 'payments', 'inventory', 'menu', 'stores', 'staff', 'suppliers', 'delivery',
	'loyalty', 'marketing', 'finance', 'pricing', 'reviews', 'support', 'ops', 'forecasting', 'logistics',
	'events', 'web', 'mobile', 'franchise', 'quality', 'audit'
];
const OWNERS = ['analytics', 'finance-data', 'growth', 'platform', 'ops'];
const TAGS = ['core', 'finance', 'pii', 'daily', 'hourly', 'reporting'];

export const domainName = (i: number) =>
	DOMAIN_NAMES[i % DOMAIN_NAMES.length] + (i >= DOMAIN_NAMES.length ? `_${Math.floor(i / DOMAIN_NAMES.length) + 1}` : '');

type Kind = { kind: string; icon: string };
const KINDS: Record<(typeof LAYERS)[number], Kind[]> = {
	raw: [
		{ kind: 'source', icon: 'source' },
		{ kind: 'source · merge', icon: 'loader' },
		{ kind: 'loader · table', icon: 'loader' },
		{ kind: 'seed', icon: 'seed' }
	],
	stg: [{ kind: 'view', icon: 'view' }],
	int: [
		{ kind: 'view', icon: 'view' },
		{ kind: 'table', icon: 'table' }
	],
	fct: [
		{ kind: 'table', icon: 'table' },
		{ kind: 'incremental · merge', icon: 'merge' },
		{ kind: 'incremental · delete_insert', icon: 'delete_insert' }
	],
	dim: [
		{ kind: 'table', icon: 'table' },
		{ kind: 'incremental · merge', icon: 'merge' }
	],
	mart: [
		{ kind: 'table', icon: 'table' },
		{ kind: 'incremental · delete_insert', icon: 'microbatch' }
	],
	rpt: [
		{ kind: 'view', icon: 'view' },
		{ kind: 'table', icon: 'table' }
	]
};

function rng(seed: number) {
	let s = seed >>> 0;
	return () => {
		s = (s + 0x6d2b79f5) >>> 0;
		let t = s;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

export function scaleGraph(total: number): { nodes: Node[]; edges: Edge[] } {
	const rand = rng(total);
	const pick = <T>(xs: readonly T[]) => xs[Math.floor(rand() * xs.length)];
	const domains = Math.max(1, Math.round(total / MODELS_PER_DOMAIN));
	const byLayer: string[][][] = LAYERS.map(() => Array.from({ length: domains }, () => []));
	const nodes: Node[] = [];
	const edges: Edge[] = [];
	const downstream = new Map<string, string[]>();

	let made = 0;
	LAYERS.forEach((layer, li) => {
		const count = li === LAYERS.length - 1 ? total - made : Math.round(total * LAYER_SHARE[li]);
		for (let i = 0; i < count; i++) {
			const domain = Math.floor(rand() * domains);
			const id = `${layer}_${li}_${i}`;
			const { kind, icon } = pick(KINDS[layer]);
			const name = `${layer}__${domainName(domain)}__m${i}`;
			const isSource = kind.startsWith('source');
			const fail = !isSource && rand() < 0.03;
			const checkCount = isSource ? 0 : Math.floor(rand() * 5);
			const warn = !fail && checkCount > 0 && rand() < 0.06;
			nodes.push({
				id,
				type: 'asset',
				position: { x: 0, y: 0 },
				data: {
					name,
					kind,
					icon,
					status: isSource ? 'src' : fail ? 'fail' : 'ok',
					lastBuiltAt: new Date(Date.now() - (1 + Math.floor(rand() * 59)) * 60_000).toISOString(),
					...(fail
						? { checks: '✕ 1', checksColor: 'var(--sb-error)' }
						: warn
							? { checks: '! 1', checksColor: 'var(--sb-warning)' }
							: checkCount
								? { checks: `✓ ${checkCount}` }
								: {}),
					...(isSource
						? {}
						: {
								columnCount: 3 + Math.floor(rand() * 20),
								owner: rand() < 0.6 ? [pick(OWNERS)] : undefined,
								tags: rand() < 0.4 ? [pick(TAGS)] : undefined,
								description: rand() < 0.5 ? `${layer} model for ${domainName(domain)}.` : undefined
							})
				}
			});
			byLayer[li][domain].push(id);

			if (li > 0) {
				const parents = 1 + Math.floor(rand() * (layer === 'stg' ? 1.5 : 3.5));
				const seen = new Set<string>();
				for (let p = 0; p < parents; p++) {
					// mostly the previous layer, sometimes further back; mostly same domain
					const back = 1 + (rand() < 0.7 ? 0 : Math.floor(rand() * Math.min(3, li)));
					const pl = li - back;
					const pd = rand() < 0.85 ? domain : Math.floor(rand() * domains);
					const pool = byLayer[pl][pd].length ? byLayer[pl][pd] : byLayer[pl].flat();
					if (!pool.length) continue;
					const src = pool[Math.floor(rand() * pool.length)];
					if (seen.has(src)) continue;
					seen.add(src);
					edges.push({ id: `${src}-${id}`, source: src, target: id, type: 'bezier' });
					(downstream.get(src) ?? downstream.set(src, []).get(src)!).push(id);
				}
			}
		}
		made += count;
	});

	// Staleness: a few models changed since the last build in this target (direct
	// SQL change, or compiled-only change); everything downstream of them is stale
	// with an upstream change. Staleness is shown separately from the last outcome.
	const byId = new Map(nodes.map((n) => [n.id, n]));
	const driftSources = nodes.filter((n) => n.data.status !== 'src' && rand() < 0.008);
	for (const n of driftSources) n.data = { ...n.data, stale: rand() < 0.75 ? 'changed' : 'compiled' };
	const stack = driftSources.flatMap((n) => downstream.get(n.id) ?? []);
	const seen = new Set<string>();
	while (stack.length) {
		const id = stack.pop()!;
		if (seen.has(id)) continue;
		seen.add(id);
		const n = byId.get(id)!;
		if (!n.data.stale) n.data = { ...n.data, stale: 'upstream' };
		stack.push(...(downstream.get(id) ?? []));
	}
	return { nodes, edges };
}
