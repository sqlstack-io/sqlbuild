// Mock data for the Cost page mockups (CHI-1650): a 2,281-model Snowflake project with 30 jobs and
// nested folders. Deterministic, so screenshots compare; the timeline follows the clock.
import type { RunStatus } from '$lib/api';
import type { CostDay, CostFact, CostModel, CostPage, CostRow, CostRun, CreditPrice, PriceSource } from '../../../routes/cost/types';

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

export const MANUAL = 'Manual or CLI';

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

const PRICES: Record<PriceSource, CreditPrice> = {
	config: { usdPerCredit: 3, source: 'config', detail: '' },
	rate_sheet: { usdPerCredit: 2.73, source: 'rate_sheet', detail: 'Contract · 22 Nov 2026' },
	list_price: { usdPerCredit: 3.9, source: 'list_price', detail: 'Enterprise · AWS us-east-1' }
};

function prng(seed: number) {
	return () => {
		seed = (seed + 0x6d2b79f5) | 0;
		let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

type Model = { key: string; folder: string; weight: number; queries: number; bytes: number; rising: boolean };
type Run = CostRun & { at: number };
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
		const nodes = jobs.get(job)!.models.length;
		for (let at = start + rand() * every; at < now; at += every) {
			const noisy = credits * (0.7 + rand() * 0.6) * (rand() < 0.01 ? 5 + rand() * 4 : 1) * (at > now - 9 * 86_400_000 && job === 'events_hourly' ? 1.45 : 1);
			const roll = rand();
			const status: RunStatus = now - at < every && now - at < 1_200_000 ? 'running' : roll < 0.03 ? 'failed' : roll < 0.06 ? 'partial' : 'success';
			const spent = status === 'failed' ? noisy * 0.3 : noisy;
			runs.push({ id: id(at, 2654435761), job, at, startedAt: new Date(at).toISOString(), status, credits: spent, seconds: (spent / RATE[size]) * 3600, queries: nodes * 2, nodes, size });
		}
	}
	// Runs started by hand or from the CLI: a few big backfills.
	for (let i = 0; i < 40; i++) {
		const at = start + rand() * (now - start - 3_600_000);
		const spent = rand() < 0.2 ? 30 + rand() * 60 : 0.5 + rand() * 6;
		runs.push({ id: id(at, 2246822519), job: null, at, startedAt: new Date(at).toISOString(), status: 'success', credits: spent, seconds: (spent / 4) * 3600, queries: 40, nodes: jobs.get(MANUAL)!.models.length, size: 'Medium' });
	}
	world = { models, jobs, runs: runs.sort((a, b) => b.at - a.at), hour };
	return world;
}

function group<T>(items: T[], key: (t: T) => string, value: (t: T) => number): Map<string, number> {
	const out = new Map<string, number>();
	for (const t of items) out.set(key(t), (out.get(key(t)) ?? 0) + value(t));
	return out;
}

const sum = <T>(items: T[], value: (t: T) => number) => items.reduce((t, i) => t + value(i), 0);

/** The Cost page's read for the last `days` days, priced from `source`. */
export function mockCostPage(days: number, source: PriceSource, now = Date.now()): CostPage {
	const { models, jobs, runs } = build(now);
	const since = now - days * 86_400_000;
	const current = runs.filter((r) => r.at >= since);
	const before = runs.filter((r) => r.at < since && r.at >= since - days * 86_400_000);
	const jobOf = (r: Run) => r.job ?? MANUAL;
	const curByJob = group(current, jobOf, (r) => r.credits);
	const prevByJob = group(before, jobOf, (r) => r.credits);
	const runsByJob = group(current, jobOf, () => 1);
	const secondsByJob = group(current, jobOf, (r) => r.seconds);

	// Each job's credits split over the models it builds, by weight.
	const facts: CostFact[] = [];
	for (const job of jobs.values()) {
		const n = runsByJob.get(job.key) ?? 0;
		if (!n) continue;
		for (const m of job.models) {
			const share = m.weight / job.weight;
			facts.push({
				job: job.key,
				model: m.key,
				folder: m.folder,
				share,
				credits: (curByJob.get(job.key) ?? 0) * share,
				previous: (prevByJob.get(job.key) ?? 0) * share * (m.rising ? 0.45 : 1),
				seconds: (secondsByJob.get(job.key) ?? 0) * share,
				runs: n,
				queries: m.queries * n,
				bytesScanned: m.bytes * n
			});
		}
	}

	const rows = (cur: Run[], prev: Run[], key: (r: Run) => string, nodes: (k: string) => number): CostRow[] => {
		const p = group(prev, key, (r) => r.credits);
		return [...new Set(cur.map(key))]
			.map((k) => {
				const mine = cur.filter((r) => key(r) === k);
				return { key: k, credits: sum(mine, (r) => r.credits), previous: p.get(k) ?? 0, seconds: sum(mine, (r) => r.seconds), runs: mine.length, nodes: nodes(k) };
			})
			.sort((a, b) => b.credits - a.credits);
	};
	const jobRows = rows(current, before, jobOf, (k) => jobs.get(k)?.models.length ?? 0);
	const sizes = rows(current, before, (r) => r.size, (k) => new Set(facts.filter((f) => jobs.get(f.job)?.size === k).map((f) => f.model)).size);

	const byModel = new Map<string, CostFact[]>();
	for (const f of facts) byModel.set(f.model, [...(byModel.get(f.model) ?? []), f]);
	const folderOf = new Map(models.map((m) => [m.key, m.folder]));
	const costModels: CostModel[] = [...byModel]
		.map(([key, fs]) => {
			const top = [...fs].sort((a, b) => b.credits - a.credits)[0];
			return { key, folder: folderOf.get(key)!.split('/')[0], job: top.job, credits: sum(fs, (f) => f.credits), previous: sum(fs, (f) => f.previous), seconds: sum(fs, (f) => f.seconds), runs: sum(fs, (f) => f.runs), nodes: 1, queries: sum(fs, (f) => f.queries), bytesScanned: sum(fs, (f) => f.bytesScanned) };
		})
		.sort((a, b) => b.credits - a.credits);
	const folders: CostRow[] = [...new Set(costModels.map((m) => m.folder))]
		.map((f) => {
			const mine = costModels.filter((m) => m.folder === f);
			return { key: f, credits: sum(mine, (m) => m.credits), previous: sum(mine, (m) => m.previous), seconds: sum(mine, (m) => m.seconds), runs: Math.max(...mine.map((m) => m.runs)), nodes: mine.length };
		})
		.sort((a, b) => b.credits - a.credits);

	const daily: CostDay[] = [];
	const firstDay = Math.floor(since / 86_400_000) + 1;
	const jobDaily: Record<string, number[]> = {};
	for (let d = firstDay; d <= Math.floor(now / 86_400_000); d++) {
		const mine = current.filter((r) => Math.floor(r.at / 86_400_000) === d);
		const byJob = Object.fromEntries(group(mine, jobOf, (r) => r.credits));
		daily.push({ date: new Date(d * 86_400_000).toISOString().slice(0, 10), credits: sum(mine, (r) => r.credits), byJob, bySize: Object.fromEntries(group(mine, (r) => r.size, (r) => r.credits)) });
	}
	for (const job of jobs.keys()) jobDaily[job] = daily.map((d) => d.byJob[job] ?? 0);

	return {
		days,
		since: new Date(since).toISOString(),
		price: PRICES[source],
		credits: sum(current, (r) => r.credits),
		previous: sum(before, (r) => r.credits),
		seconds: sum(current, (r) => r.seconds),
		runCount: current.length,
		incompleteRuns: current.filter((r) => r.status === 'running').length,
		jobs: jobRows,
		folders,
		sizes,
		models: costModels,
		runs: [...current].sort((a, b) => b.credits - a.credits).map(({ at: _at, ...r }) => r),
		daily,
		facts,
		jobDaily,
		jobSize: Object.fromEntries([...jobs.values()].map((j) => [j.key, j.size]))
	};
}

/** A model's statements and their share of its cost; the cost API has no statement breakdown yet. */
export function mockStatements(model: string, credits: number, seconds: number, queries: number) {
	const kinds: [string, number][] = model.includes('_scd') || model.includes('_history')
		? [[`merge into ${model.toUpperCase()}`, 0.68], [`delete from ${model.toUpperCase()}`, 0.17], [`select count(*) from ${model.toUpperCase()}`, 0.09], [`alter table ${model.toUpperCase()} cluster by`, 0.06]]
		: [[`create or replace table ${model.toUpperCase()} as select`, 0.81], [`select count(*) from ${model.toUpperCase()}`, 0.11], [`grant select on ${model.toUpperCase()}`, 0.08]];
	return kinds.map(([text, share]) => ({ text, credits: credits * share, seconds: seconds * share, queries: Math.max(1, Math.round((queries * share) / 2)) }));
}
