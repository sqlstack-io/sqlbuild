// Resolves selectors over the Cost mock's 2,281 models, in the browser. The mock server's
// evaluateSelection knows only the small demo catalog; the real page would ask the server.
// Mirrors evaluateMockSelection (operate-graph.ts): tokens unioned, comma parts intersected.
import type { SelectionEvaluation, SelectionProblem, SelectionVocabulary, VocabularyEntry } from '$lib/api';
import { MANUAL, mockCostJobs, mockCostModels } from './cost-page';
import { pathOfFolder } from '../../../routes/cost/utils/selector';

type Meta = { key: string; folder: string; path: string; tags: string[]; layer: number; entity: string };

const ENTITY = /(order_lines|orders|customers|sessions|events|payments|refunds|invoices|products|inventory|shipments|returns|campaigns|clicks|impressions|subscriptions|accounts|users|devices|pageviews|carts|promotions|suppliers|stores|warehouses|forecasts|features|experiments|ledger|fx_rates|tickets|reviews)/;
const LAYER: Record<string, number> = { staging: 0, python: 0, intermediate: 1, snapshots: 3 };


function tagsOf(key: string, folder: string): string[] {
	const tags: string[] = [];
	if (folder.startsWith('finance') || folder === 'marts/finance' || /ledger|invoices|payments|refunds/.test(key)) tags.push('finance');
	if (/customers|users|accounts|devices/.test(key)) tags.push('pii');
	if (key.includes('_hourly')) tags.push('hourly');
	if (folder.startsWith('ml')) tags.push('ml');
	if (folder.startsWith('reporting') || folder.startsWith('marts')) tags.push('reporting');
	return tags;
}

let held: { at: number; metas: Meta[]; byKey: Map<string, Meta>; jobs: Map<string, string[]> } | null = null;
function catalog() {
	const hour = Math.floor(Date.now() / 3_600_000);
	if (held?.at === hour) return held;
	const metas = mockCostModels().map((m) => ({ ...m, path: pathOfFolder(m.folder), tags: tagsOf(m.key, m.folder), layer: LAYER[m.folder.split('/')[0]] ?? 2, entity: ENTITY.exec(m.key)?.[1] ?? '' }));
	held = { at: hour, metas, byKey: new Map(metas.map((m) => [m.key, m])), jobs: mockCostJobs() };
	return held;
}

const glob = (pattern: string) => new RegExp(`^${pattern.replace(/[.^$()|\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.')}$`);

/** Mock lineage: a model's upstream is the earlier layers' models of the same entity; downstream the later ones. */
function expand(keys: Set<string>, up: boolean, down: boolean): Set<string> {
	if (!up && !down) return keys;
	const { metas, byKey } = catalog();
	const out = new Set(keys);
	for (const k of keys) {
		const m = byKey.get(k);
		if (!m?.entity) continue;
		for (const o of metas) if (o.entity === m.entity && ((up && o.layer < m.layer) || (down && o.layer > m.layer))) out.add(o.key);
	}
	return out;
}

function matchPart(raw: string): Set<string> | null {
	const { metas, byKey, jobs } = catalog();
	const up = raw.startsWith('+');
	const down = raw.length > 1 && raw.endsWith('+');
	const core = raw.replace(/^\+/, '').replace(/\+$/, '');
	const colon = core.indexOf(':');
	const type = colon > 0 ? core.slice(0, colon) : null;
	const value = colon > 0 ? core.slice(colon + 1) : core;
	let hits: Set<string>;
	if (type === 'job') hits = new Set(jobs.get(value) ?? []);
	else if (type === 'path' || (type === null && value.includes('/'))) {
		const p = value.replace(/\/$/, '');
		hits = new Set(metas.filter((m) => m.path === p || m.path.startsWith(`${p}/`)).map((m) => m.key));
	} else if (type === 'tag') hits = new Set(metas.filter((m) => m.tags.some((t) => glob(value).test(t))).map((m) => m.key));
	else if (type === 'kind') hits = value === 'model' || value === 'table' ? new Set(metas.map((m) => m.key)) : new Set();
	else if (type === null) hits = /[*?]/.test(value) ? new Set(metas.filter((m) => glob(value).test(m.key)).map((m) => m.key)) : byKey.has(value) ? new Set([value]) : new Set();
	else return null;
	if (!hits.size) return null;
	return expand(hits, up, down);
}

function suggest(raw: string): string[] {
	const { metas, jobs } = catalog();
	const core = raw.replace(/^\+/, '').replace(/\+$/, '');
	const colon = core.indexOf(':');
	const type = colon > 0 ? core.slice(0, colon) : null;
	const value = (colon > 0 ? core.slice(colon + 1) : core).toLowerCase();
	const pool = type === 'job' ? [...jobs.keys()].filter((j) => j !== MANUAL) : type === 'tag' ? [...new Set(metas.flatMap((m) => m.tags))] : type === null ? metas.map((m) => m.key) : [];
	const close = pool.filter((v) => v.toLowerCase().startsWith(value.slice(0, 3))).slice(0, 1);
	return close.map((v) => raw.replace(core, `${type ? `${type}:` : ''}${v}`));
}

/** The models a selection selects, as `model:<name>` keys, or why `sqb` would refuse it. */
export function evaluateCostSelection(select: string, exclude: string): SelectionEvaluation {
	const resolve = (field: 'select' | 'exclude', text: string): Set<string> | SelectionProblem => {
		const chosen = new Set<string>();
		for (const match of text.matchAll(/\S+/g)) {
			const token = match[0];
			let hits = null as Set<string> | null;
			for (const [index, part] of token.split(',').entries()) {
				const found = matchPart(part);
				if (!found) {
					const bare = part.replace(/^\+/, '').replace(/\+$/, '');
					const colon = bare.indexOf(':');
					const refused = { part: index, type: colon > 0 ? bare.slice(0, colon) : null, value: colon > 0 ? bare.slice(colon + 1) : bare };
					const suggestions = suggest(part).map((s) => token.replace(part, s));
					return { code: 'S007', message: `unknown selector name '${part}'`, help: null, location: { field, token, start: match.index!, end: match.index! + token.length }, suggestions, refused };
				}
				const prev: Set<string> | null = hits;
				hits = prev ? new Set([...prev].filter((k) => found.has(k))) : found;
			}
			for (const k of hits ?? []) chosen.add(k);
		}
		return chosen;
	};
	const reply = (keys: string[], error: SelectionProblem | null): SelectionEvaluation => ({ snapshotId: 'mock-cost', count: keys.length, keys, nextOffset: null, error });
	const selected = select.trim() ? resolve('select', select) : new Set(catalog().metas.map((m) => m.key));
	if (!(selected instanceof Set)) return reply([], selected);
	const excluded = exclude.trim() ? resolve('exclude', exclude) : new Set<string>();
	if (!(excluded instanceof Set)) return reply([], excluded);
	const keys = [...selected].filter((k) => !excluded.has(k)).sort();
	if (!keys.length) return reply([], { code: null, message: 'the selectors match no nodes in the current publication', help: null, location: null, suggestions: [], refused: null });
	return reply(keys.map((k) => `model:${k}`), null);
}

/** Tags, folders and jobs of the Cost mock, for the selection field's suggestions. */
export function costVocabulary(): SelectionVocabulary {
	const { metas, jobs } = catalog();
	const counted = (values: string[]): VocabularyEntry[] => {
		const counts = new Map<string, number>();
		for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
		return [...counts].sort(([a], [b]) => a.localeCompare(b)).map(([value, count]) => ({ value, count }));
	};
	const folders = (path: string) => path.split('/').map((_, i, parts) => parts.slice(0, i + 1).join('/'));
	return {
		snapshotId: 'mock-cost',
		tags: counted(metas.flatMap((m) => m.tags)),
		paths: counted(metas.flatMap((m) => folders(m.path))),
		jobs: [...jobs].filter(([j]) => j !== MANUAL).map(([value, keys]) => ({ value, count: keys.length })).sort((a, b) => a.value.localeCompare(b.value)),
		sources: [],
		seeds: [],
		kinds: [{ value: 'model', count: metas.length }]
	};
}

/** Model names with their folders, for picking by name. */
export const costNames = () => catalog().metas.map((m) => ({ value: m.key, hint: m.folder }));
