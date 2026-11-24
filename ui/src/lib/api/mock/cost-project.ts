// A Snowflake project the size of a real one for the Cost page, turned on with `?mock&costs`: 2,281
// models in nested folders, 30 jobs and builds by hand, over 60 days. Deterministic, so screenshots
// compare; the timeline follows the clock. Its selections resolve over its own models, and each job's
// cost is split over the models it builds by weight. Pure apart from wrapping the client it is given.

import type { ApiClient } from '../client';
import type { CostFact, CostSelection, CostSelectionQuery, CostSeriesGroup, NodeBuildCost, RunStatus, Scope, SelectionEvaluation, SelectionProblem, SelectionVocabulary, VocabularyEntry } from '../types';
import { costPeriod, inCostGroup } from './cost-rows';

/** A folder as `path:` selects it: models under `sql/models/`, Python nodes under their own root. */
const pathOf = (folder: string) => (folder.startsWith('python/') ? folder : `sql/models/${folder}`);

/** Leaf folders, their model count and model name prefix: 2,281 models. */
const FOLDERS: [string, number, string][] = [
	['staging/orders', 90, 'stg'],
	['staging/customers', 80, 'stg'],
	['staging/payments', 70, 'stg'],
	['staging/events', 110, 'stg'],
	['staging/inventory', 70, 'stg'],
	['staging/partners/web', 60, 'stg'],
	['staging/partners/feeds', 50, 'stg'],
	['staging/support', 40, 'stg'],
	['staging/products', 70, 'stg'],
	['intermediate/orders', 90, 'int'],
	['intermediate/customers', 70, 'int'],
	['intermediate/inventory', 60, 'int'],
	['intermediate/sessions/web', 70, 'int'],
	['intermediate/sessions/app', 50, 'int'],
	['intermediate/pricing', 80, 'int'],
	['marts/core', 120, 'fct'],
	['marts/finance', 60, 'fct'],
	['marts/marketing', 80, 'dim'],
	['marts/product', 70, 'fct'],
	['marts/support', 50, 'dim'],
	['finance/ledger', 60, 'fin'],
	['finance/revenue', 60, 'fin'],
	['finance/fx', 40, 'fin'],
	['marketing/attribution', 80, 'mkt'],
	['marketing/campaigns', 60, 'mkt'],
	['marketing/audiences', 50, 'mkt'],
	['product/experiments', 70, 'prd'],
	['product/metrics', 100, 'prd'],
	['ml/features', 45, 'ml'],
	['ml/training', 25, 'ml'],
	['ml/scoring', 25, 'ml'],
	['reporting/daily', 80, 'rpt'],
	['reporting/exports', 60, 'rpt'],
	['snapshots', 56, 'snap'],
	['python/nodes/loaders', 18, 'py'],
	['python/nodes/checks', 12, 'py']
];
const ENTITIES = ['orders', 'order_lines', 'customers', 'sessions', 'events', 'payments', 'refunds', 'invoices', 'products', 'inventory', 'shipments', 'returns', 'campaigns', 'clicks', 'impressions', 'subscriptions', 'accounts', 'users', 'devices', 'pageviews', 'carts', 'promotions', 'suppliers', 'stores', 'warehouses', 'forecasts', 'features', 'experiments', 'ledger', 'fx_rates', 'tickets', 'reviews'];
const SUFFIXES = ['', '_daily', '_hourly', '_enriched', '_latest', '_history', '_by_region', '_by_channel', '_scd', '_rollup', '_v2', '_agg', '_dedup', '_flat', '_wide', '_weekly'];

/** The job key runs started by hand or from the CLI are kept under here; they have no job. */
const MANUAL = '';

/** Job, runs a day (fractions run every few days), credits a run, warehouse size, folders it builds. */
const JOBS: [string, number, number, string, string[]][] = [
	['nightly_full_refresh', 1, 96, 'Large', ['staging', 'intermediate', 'marts', 'finance', 'marketing', 'product', 'reporting']],
	['events_hourly', 24, 2.4, 'Medium', ['staging/events', 'intermediate/sessions']],
	['sessions_hourly', 24, 1.6, 'Medium', ['intermediate/sessions', 'marts/product']],
	['orders_15min', 96, 0.11, 'X-Small', ['staging/orders']],
	['finance_close', 1, 18.5, 'Medium', ['finance']],
	['ml_features_daily', 1, 24, 'Large', ['ml/features', 'ml/scoring']],
	['ml_training_weekly', 1 / 7, 140, 'Large', ['ml/training']],
	['marketing_attribution', 2, 6.2, 'Medium', ['marketing/attribution', 'marts/marketing']],
	['product_metrics', 4, 2.1, 'Small', ['product/metrics']],
	['reporting_refresh', 6, 0.9, 'Small', ['reporting/daily']],
	['snapshots_daily', 1, 4.4, 'Small', ['snapshots']],
	['inventory_hourly', 24, 0.32, 'X-Small', ['staging/inventory', 'intermediate/inventory']],
	['payments_hourly', 24, 0.28, 'X-Small', ['staging/payments', 'finance/ledger']],
	['customers_daily', 1, 3.6, 'Small', ['marts/core', 'intermediate/customers']],
	['campaigns_daily', 1, 2.9, 'Small', ['marketing/campaigns', 'marketing/audiences']],
	['exports_daily', 1, 1.4, 'X-Small', ['reporting/exports']],
	['fx_rates_hourly', 24, 0.04, 'X-Small', ['finance/fx']],
	['partner_loaders', 12, 0.12, 'X-Small', ['python/nodes/loaders', 'staging/partners/feeds']],
	['shipments_4h', 6, 0.7, 'Small', ['staging/orders', 'intermediate/orders']],
	['returns_daily', 1, 1.1, 'X-Small', ['intermediate/orders', 'marts/core']],
	['experiments_daily', 1, 3.3, 'Small', ['product/experiments']],
	['subscriptions_daily', 1, 2.2, 'Small', ['marts/finance', 'finance/revenue']],
	['devices_daily', 1, 0.8, 'X-Small', ['staging/events']],
	['tickets_hourly', 24, 0.05, 'X-Small', ['staging/support', 'marts/support']],
	['reviews_daily', 1, 0.4, 'X-Small', ['staging/products']],
	['ledger_backfill', 1 / 10, 58, 'Large', ['finance/ledger']],
	['marts_weekly_rebuild', 1 / 7, 72, 'Large', ['marts', 'intermediate']],
	['forecasts_daily', 1, 7.5, 'Medium', ['ml/scoring', 'finance/revenue']],
	['suppliers_weekly', 1 / 7, 3.1, 'Small', ['staging/partners']],
	['data_quality', 4, 0.6, 'Small', ['python/nodes/checks', 'marts/core']]
];
/** What runs started by hand or from the CLI built. */
const MANUAL_FOLDERS = ['marts/core', 'intermediate/pricing'];
/** Credits an hour by size, for warehouse time. */
const RATE: Record<string, number> = { 'X-Small': 1, Small: 2, Medium: 4, Large: 8 };


function prng(seed: number) {
	return () => {
		seed = (seed + 0x6d2b79f5) | 0;
		let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

type Model = { key: string; folder: string; weight: number; queries: number; bytes: number; rising: boolean };
type Run = { id: string; job: string | null; at: number; status: RunStatus; credits: number; seconds: number };
type Job = { key: string; size: string; models: Model[]; weight: number };

let world: { models: Model[]; jobs: Map<string, Job>; runs: Run[]; hour: number } | null = null;

const under = (folder: string, prefixes: string[]) => prefixes.some((p) => folder === p || folder.startsWith(`${p}/`));

function build(now: number) {
	const hour = Math.floor(now / 3_600_000);
	if (world?.hour === hour) return world;
	const rand = prng(1650);
	const models: Model[] = [];
	const used = new Set<string>();
	for (const [folder, count, prefix] of FOLDERS) {
		for (let i = 0; i < count; i++) {
			let key = '';
			for (let n = 0; !key || used.has(key); n++) key = `${prefix}_${ENTITIES[Math.floor(rand() * ENTITIES.length)]}${SUFFIXES[Math.floor(rand() * SUFFIXES.length)]}${n > 2 ? `_${n}` : ''}`;
			used.add(key);
			// A few models take most of a job's time.
			const weight = rand() ** 10 * 600 + rand();
			models.push({ key, folder, weight, queries: 1 + Math.floor(rand() * 4), bytes: weight * 2e9 * (0.5 + rand()), rising: rand() < 0.04 });
		}
	}
	const jobs = new Map<string, Job>();
	const addJob = (key: string, size: string, folders: string[]) => {
		const mine = models.filter((m) => under(m.folder, folders));
		jobs.set(key, { key, size, models: mine, weight: mine.reduce((t, m) => t + m.weight, 0) });
	};
	for (const [job, , , size, folders] of JOBS) addJob(job, size, folders);
	addJob(MANUAL, 'Medium', MANUAL_FOLDERS);

	const runs: Run[] = [];
	const start = (hour - 60 * 24) * 3_600_000;
	const id = (at: number, salt: number) => ((Math.floor(at / 1000) * salt) >>> 0).toString(16).padStart(8, '0');
	for (const [job, perDay, credits, size] of JOBS) {
		const every = 86_400_000 / perDay;
		for (let at = start + rand() * every; at < now; at += every) {
			const noisy = credits * (0.7 + rand() * 0.6) * (rand() < 0.01 ? 5 + rand() * 4 : 1) * (at > now - 9 * 86_400_000 && job === 'events_hourly' ? 1.45 : 1);
			const roll = rand();
			const status: RunStatus = now - at < every && now - at < 1_200_000 ? 'running' : roll < 0.03 ? 'failed' : roll < 0.06 ? 'partial' : 'success';
			const spent = status === 'failed' ? noisy * 0.3 : noisy;
			runs.push({ id: id(at, 2654435761), job, at, status, credits: spent, seconds: (spent / RATE[size]) * 3600 });
		}
	}
	// Runs started by hand or from the CLI: a few big backfills.
	for (let i = 0; i < 40; i++) {
		const at = start + rand() * (now - start - 3_600_000);
		const spent = rand() < 0.2 ? 30 + rand() * 60 : 0.5 + rand() * 6;
		runs.push({ id: id(at, 2246822519), job: null, at, status: 'success', credits: spent, seconds: (spent / 4) * 3600 });
	}
	world = { models, jobs, runs: runs.sort((a, b) => b.at - a.at), hour };
	return world;
}


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
	const metas = build(Date.now()).models.map((m) => ({ key: m.key, folder: m.folder, path: pathOf(m.folder), tags: tagsOf(m.key, m.folder), layer: LAYER[m.folder.split('/')[0]] ?? 2, entity: ENTITY.exec(m.key)?.[1] ?? '' }));
	held = { at: hour, metas, byKey: new Map(metas.map((m) => [m.key, m])), jobs: new Map([...build(Date.now()).jobs.values()].map((j) => [j.key, j.models.map((m) => m.key)])) };
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
function evaluateCostSelection(select: string, exclude: string): SelectionEvaluation {
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
	const reply = (keys: string[], error: SelectionProblem | null): SelectionEvaluation => ({ snapshotId: 'mock-snapshot', count: keys.length, keys, nextOffset: null, error });
	const selected = select.trim() ? resolve('select', select) : new Set(catalog().metas.map((m) => m.key));
	if (!(selected instanceof Set)) return reply([], selected);
	const excluded = exclude.trim() ? resolve('exclude', exclude) : new Set<string>();
	if (!(excluded instanceof Set)) return reply([], excluded);
	const keys = [...selected].filter((k) => !excluded.has(k)).sort();
	if (!keys.length) return reply([], { code: null, message: 'the selectors match no nodes in the current publication', help: null, location: null, suggestions: [], refused: null });
	return reply(keys.map((k) => `model:${k}`), null);
}

/** Tags, folders and jobs of the Cost mock, for the selection field's suggestions. */
function costVocabulary(): SelectionVocabulary {
	const { metas, jobs } = catalog();
	const counted = (values: string[]): VocabularyEntry[] => {
		const counts = new Map<string, number>();
		for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
		return [...counts].sort(([a], [b]) => a.localeCompare(b)).map(([value, count]) => ({ value, count }));
	};
	const folders = (path: string) => path.split('/').map((_, i, parts) => parts.slice(0, i + 1).join('/'));
	return {
		snapshotId: 'mock-snapshot',
		tags: counted(metas.flatMap((m) => m.tags)),
		paths: counted(metas.flatMap((m) => folders(m.path))),
		jobs: [...jobs].filter(([j]) => j !== MANUAL).map(([value, keys]) => ({ value, count: keys.length })).sort((a, b) => a.value.localeCompare(b.value)),
		sources: [],
		seeds: [],
		kinds: [{ value: 'model', count: metas.length }]
	};
}


/** Each job's share of a model's cost: the model's weight among the job's models. */
function shares(): Map<string, { model: Model; share: number }[]> {
	const out = new Map<string, { model: Model; share: number }[]>();
	for (const job of build(Date.now()).jobs.values()) out.set(job.key, job.models.map((model) => ({ model, share: model.weight / job.weight })));
	return out;
}

const DAY = 86_400_000;
/** The project's `usd_per_credit`. */
const PRICE = 3;

/** The last reads, by query and hour, so batches of series don't build them again. */
const reads = new Map<string, { read: CostSelection; byJob: Map<string | null, CostFact[]>; jobTotal: Map<string | null, number> }>();

function cached(query: CostSelectionQuery) {
	const key = `${Math.floor(Date.now() / 3_600_000)}\u0000${query.days}\u0000${query.select}\u0000${query.exclude}`;
	let hit = reads.get(key);
	if (!hit) {
		const read = costSelection(query);
		const byJob = new Map<string | null, CostFact[]>();
		const jobTotal = new Map<string | null, number>();
		for (const f of read.facts) {
			(byJob.get(f.job) ?? byJob.set(f.job, []).get(f.job)!).push(f);
			jobTotal.set(f.job, (jobTotal.get(f.job) ?? 0) + f.credits);
		}
		if (reads.size > 20) reads.clear();
		reads.set(key, (hit = { read, byJob, jobTotal }));
	}
	return hit;
}

function costSelection(query: CostSelectionQuery): CostSelection {
	const now = Date.now();
	const period = costPeriod(query.days, now);
	const evaluated = query.select.trim() || query.exclude.trim() ? evaluateCostSelection(query.select, query.exclude) : null;
	const chosen = evaluated && new Set(evaluated.keys);
	const { runs } = build(now);
	const byJob = new Map<string, { credits: number; previous: number; seconds: number; runs: number; daily: number[]; pending: number }>();
	for (const r of runs) {
		if (r.at < period.previous) continue;
		const key = r.job ?? MANUAL;
		const j = byJob.get(key) ?? byJob.set(key, { credits: 0, previous: 0, seconds: 0, runs: 0, daily: Array(query.days).fill(0), pending: 0 }).get(key)!;
		const day = period.day(r.at);
		if (day === null) j.previous += r.credits;
		else {
			j.credits += r.credits;
			j.seconds += r.seconds;
			j.runs += 1;
			j.daily[day] += r.credits;
			if (r.status === 'running' || r.status === 'partial') j.pending += 1;
		}
	}
	const facts: CostFact[] = [];
	const folderDaily = new Map<string, number[]>();
	const jobDaily = new Map<string | null, number[]>();
	let runCount = 0;
	let incomplete = 0;
	for (const [job, parts] of shares()) {
		const j = byJob.get(job);
		if (!j) continue;
		let any = false;
		for (const { model, share } of parts) {
			const key = `model:${model.key}`;
			if (chosen && !chosen.has(key)) continue;
			any = true;
			const credits = j.credits * share;
			facts.push({ job: job || null, nodeKey: key, folder: model.folder, credits, previousCredits: j.previous * share * (model.rising ? 0.45 : 1), usd: credits * PRICE, previousUsd: j.previous * share * (model.rising ? 0.45 : 1) * PRICE, seconds: j.seconds * share, runs: j.runs, queries: model.queries * j.runs, bytesScanned: Math.round(model.bytes * j.runs) });
			const top = model.folder.split('/')[0];
			const f = folderDaily.get(top) ?? folderDaily.set(top, Array(query.days).fill(0)).get(top)!;
			const d = jobDaily.get(job || null) ?? jobDaily.set(job || null, Array(query.days).fill(0)).get(job || null)!;
			j.daily.forEach((v, i) => {
				f[i] += v * share;
				d[i] += v * share;
			});
		}
		if (any) {
			runCount += j.runs;
			incomplete += j.pending;
		}
	}
	facts.sort((a, b) => b.credits - a.credits);
	const sum = (pick: (f: CostFact) => number) => facts.reduce((t, f) => t + pick(f), 0);
	const error: SelectionProblem | null = evaluated?.error ?? null;
	return {
		snapshotId: 'mock-snapshot',
		since: new Date(period.first).toISOString(),
		days: query.days,
		dates: period.dates,
		error,
		price: error || !facts.length ? null : { usdPerCredit: PRICE, source: 'configured' },
		credits: error ? 0 : sum((f) => f.credits),
		previousCredits: error ? 0 : sum((f) => f.previousCredits),
		usd: error ? null : sum((f) => f.credits) * PRICE,
		previousUsd: error ? null : sum((f) => f.previousCredits) * PRICE,
		runs: error ? 0 : runCount,
		incompleteRuns: error ? 0 : incomplete,
		queries: error ? 0 : sum((f) => f.queries),
		seconds: error ? 0 : sum((f) => f.seconds),
		bytesScanned: error ? 0 : sum((f) => f.bytesScanned),
		facts: error ? [] : facts,
		jobDaily: error ? [] : [...jobDaily].map(([key, credits]) => ({ key, credits })),
		folderDaily: error ? [] : [...folderDaily].map(([key, credits]) => ({ key, credits }))
	};
}

function costSeries(query: CostSelectionQuery, groups: CostSeriesGroup[]): number[][] {
	const { read, byJob, jobTotal } = cached(query);
	const daily = new Map(read.jobDaily.map((s) => [s.key, s.credits]));
	return groups.map((g) => {
		const out = Array<number>(query.days).fill(0);
		const facts = g.manual ? (byJob.get(null) ?? []) : g.job !== undefined ? (byJob.get(g.job) ?? []) : read.facts;
		for (const f of facts) {
			if (!inCostGroup(g, f)) continue;
			const total = jobTotal.get(f.job) || 1;
			(daily.get(f.job) ?? []).forEach((v, i) => (out[i] += (v * f.credits) / total));
		}
		return out;
	});
}

/** A model's cost in each of its jobs' builds over the period, newest first. */
function nodeCosts(nodeKey: string, days: number): NodeBuildCost[] {
	const now = Date.now();
	const name = nodeKey.slice(nodeKey.indexOf(':') + 1);
	const since = now - days * DAY;
	const mine = new Map<string, number>();
	for (const [job, parts] of shares()) for (const p of parts) if (p.model.key === name) mine.set(job, p.share);
	return build(now)
		.runs.filter((r) => r.at >= since && mine.has(r.job ?? MANUAL))
		.map((r) => {
			const share = mine.get(r.job ?? MANUAL)!;
			return { buildRunId: `build-${r.id}`, runId: r.job ? r.id : null, startedAt: new Date(r.at).toISOString(), status: r.status === 'running' ? 'pending' : 'complete', credits: r.credits * share, usd: r.credits * share * PRICE, seconds: r.seconds * share, queryCount: 2 };
		});
}

/** The client with the Cost page's reads, and the selections they take, answered by the large cost project. */
export function costProject(client: ApiClient): ApiClient {
	// Answered after a delay, worked out then, as a server would: not in the task that asked.
	const reply = <T>(make: () => T) => new Promise<T>((resolve) => setTimeout(() => resolve(make()), 60));
	return new Proxy(client, {
		get(target, prop, receiver) {
			if (prop === 'readCostSelection') return (_scope: Scope, query: CostSelectionQuery) => reply(() => cached(query).read);
			if (prop === 'readCostSeries') return (_scope: Scope, query: CostSelectionQuery, groups: CostSeriesGroup[]) => reply(() => costSeries(query, groups));
			if (prop === 'listNodeCosts') return (_scope: Scope, nodeKey: string, days: number) => reply(() => nodeCosts(nodeKey, days));
			if (prop === 'evaluateSelection')
				return (_scope: Scope, request: { select: string; exclude: string; offset?: number; limit?: number }) => {
					return reply(() => {
						const all = evaluateCostSelection(request.select, request.exclude);
						const offset = request.offset ?? 0;
						const limit = request.limit ?? 1000;
						return { ...all, keys: all.keys.slice(offset, offset + limit), nextOffset: offset + limit < all.keys.length ? offset + limit : null };
					});
				};
			if (prop === 'selectionVocabulary') return () => reply(() => costVocabulary());
			const value = Reflect.get(target, prop, receiver);
			return typeof value === 'function' ? value.bind(target) : value;
		}
	});
}
