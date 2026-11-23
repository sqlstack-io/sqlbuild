// Mock data for the Cost page mockups (CHI-1650): a 2,281-model Snowflake project with 30 jobs.
// Deterministic, so screenshots compare; the timeline follows the clock.
import type { RunStatus } from '$lib/api';
import type { CostDay, CostModel, CostPage, CostRow, CostRun, CreditPrice, PriceSource } from '../../../routes/cost/types';

const FOLDERS: [string, number, string[]][] = [
	['staging', 640, ['stg']],
	['intermediate', 420, ['int']],
	['marts', 380, ['fct', 'dim']],
	['finance', 160, ['fin']],
	['marketing', 190, ['mkt']],
	['product', 170, ['prd']],
	['ml', 95, ['ml']],
	['reporting', 140, ['rpt']],
	['snapshots', 56, ['snap']],
	['ops', 30, ['ops']]
];
const ENTITIES = ['orders', 'order_lines', 'customers', 'sessions', 'events', 'payments', 'refunds', 'invoices', 'products', 'inventory', 'shipments', 'returns', 'campaigns', 'clicks', 'impressions', 'subscriptions', 'accounts', 'users', 'devices', 'pageviews', 'carts', 'promotions', 'suppliers', 'stores', 'warehouses', 'forecasts', 'features', 'experiments', 'ledger', 'fx_rates', 'tickets', 'reviews'];
const SUFFIXES = ['', '_daily', '_hourly', '_enriched', '_latest', '_history', '_by_region', '_by_channel', '_scd', '_rollup', '_v2', '_agg', '_dedup', '_flat', '_wide', '_weekly'];

/** Job, runs a day (fractions run every few days), credits a run, warehouse size, folders it builds. */
const JOBS: [string, number, number, string, string[]][] = [
	['nightly_full_refresh', 1, 96, 'Large', ['staging', 'intermediate', 'marts', 'finance', 'marketing', 'product', 'reporting']],
	['events_hourly', 24, 2.4, 'Medium', ['staging', 'intermediate']],
	['sessions_hourly', 24, 1.6, 'Medium', ['staging', 'marts']],
	['orders_15min', 96, 0.11, 'X-Small', ['staging']],
	['finance_close', 1, 18.5, 'Medium', ['finance']],
	['ml_features_daily', 1, 24, 'Large', ['ml']],
	['ml_training_weekly', 1 / 7, 140, 'Large', ['ml']],
	['marketing_attribution', 2, 6.2, 'Medium', ['marketing']],
	['product_metrics', 4, 2.1, 'Small', ['product']],
	['reporting_refresh', 6, 0.9, 'Small', ['reporting']],
	['snapshots_daily', 1, 4.4, 'Small', ['snapshots']],
	['inventory_hourly', 24, 0.32, 'X-Small', ['staging', 'intermediate']],
	['payments_hourly', 24, 0.28, 'X-Small', ['staging', 'finance']],
	['customers_daily', 1, 3.6, 'Small', ['marts']],
	['campaigns_daily', 1, 2.9, 'Small', ['marketing']],
	['exports_daily', 1, 1.4, 'X-Small', ['reporting']],
	['fx_rates_hourly', 24, 0.04, 'X-Small', ['finance']],
	['ops_monitoring', 12, 0.12, 'X-Small', ['ops']],
	['shipments_4h', 6, 0.7, 'Small', ['staging', 'intermediate']],
	['returns_daily', 1, 1.1, 'X-Small', ['intermediate', 'marts']],
	['experiments_daily', 1, 3.3, 'Small', ['product', 'ml']],
	['subscriptions_daily', 1, 2.2, 'Small', ['marts', 'finance']],
	['devices_daily', 1, 0.8, 'X-Small', ['staging']],
	['tickets_hourly', 24, 0.05, 'X-Small', ['staging']],
	['reviews_daily', 1, 0.4, 'X-Small', ['staging', 'marts']],
	['ledger_backfill', 1 / 10, 58, 'Large', ['finance']],
	['marts_weekly_rebuild', 1 / 7, 72, 'Large', ['marts', 'intermediate']],
	['forecasts_daily', 1, 7.5, 'Medium', ['ml', 'finance']],
	['suppliers_weekly', 1 / 7, 3.1, 'Small', ['staging', 'marts']],
	['data_quality', 4, 0.6, 'Small', ['ops', 'marts']]
];
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

type Model = { key: string; folder: string; job: string; weight: number; queries: number; bytes: number };
type Run = CostRun & { at: number };

let world: { models: Model[]; runs: Run[]; hour: number } | null = null;

function build(now: number) {
	const hour = Math.floor(now / 3_600_000);
	if (world?.hour === hour) return world;
	const rand = prng(1650);
	const models: Model[] = [];
	const used = new Set<string>();
	for (const [folder, count, prefixes] of FOLDERS) {
		const owners = JOBS.filter((j) => j[4].includes(folder)).map((j) => j[0]);
		for (let i = 0; i < count; i++) {
			let key = '';
			for (let n = 0; !key || used.has(key); n++) {
				const p = prefixes[Math.floor(rand() * prefixes.length)];
				key = `${p}_${ENTITIES[Math.floor(rand() * ENTITIES.length)]}${SUFFIXES[Math.floor(rand() * SUFFIXES.length)]}${n > 2 ? `_${n}` : ''}`;
			}
			used.add(key);
			// A few models take most of a job's time.
			const weight = rand() ** 6 * 40 + rand();
			models.push({ key, folder, job: owners[Math.floor(rand() * owners.length)], weight, queries: 1 + Math.floor(rand() * 4), bytes: weight * 2e9 * (0.5 + rand()) });
		}
	}
	const runs: Run[] = [];
	const start = (hour - 60 * 24) * 3_600_000;
	for (const [job, perDay, credits, size] of JOBS) {
		const every = 86_400_000 / perDay;
		const nodes = models.filter((m) => m.job === job).length;
		for (let at = start + rand() * every; at < now; at += every) {
			const noisy = credits * (0.7 + rand() * 0.6) * (rand() < 0.01 ? 5 + rand() * 4 : 1) * (at > now - 9 * 86_400_000 && job === 'events_hourly' ? 1.45 : 1);
			const roll = rand();
			const status: RunStatus = now - at < every && now - at < 1_200_000 ? 'running' : roll < 0.03 ? 'failed' : roll < 0.06 ? 'partial' : 'success';
			const spent = status === 'failed' ? noisy * 0.3 : noisy;
			runs.push({ id: (Math.floor(at / 1000) * 2654435761 >>> 0).toString(16).padStart(8, '0'), job, at, startedAt: new Date(at).toISOString(), status, credits: spent, seconds: (spent / RATE[size]) * 3600, queries: nodes * 2, nodes, size });
		}
	}
	// Runs started by hand or from the CLI: a few big backfills.
	for (let i = 0; i < 40; i++) {
		const at = start + rand() * (now - start - 3_600_000);
		const spent = rand() < 0.2 ? 30 + rand() * 60 : 0.5 + rand() * 6;
		runs.push({ id: (Math.floor(at / 1000) * 2246822519 >>> 0).toString(16).padStart(8, '0'), job: null, at, startedAt: new Date(at).toISOString(), status: 'success', credits: spent, seconds: (spent / 4) * 3600, queries: 40, nodes: 20 + Math.floor(rand() * 200), size: 'Medium' });
	}
	world = { models, runs: runs.sort((a, b) => b.at - a.at), hour };
	return world;
}

function group<T>(items: T[], key: (t: T) => string, credits: (t: T) => number): Map<string, number> {
	const out = new Map<string, number>();
	for (const t of items) out.set(key(t), (out.get(key(t)) ?? 0) + credits(t));
	return out;
}

/** The Cost page's read for the last `days` days, priced from `source`. */
export function mockCostPage(days: number, source: PriceSource, now = Date.now()): CostPage {
	const { models, runs } = build(now);
	const since = now - days * 86_400_000;
	const current = runs.filter((r) => r.at >= since);
	const before = runs.filter((r) => r.at < since && r.at >= since - days * 86_400_000);
	const MANUAL = 'Manual or CLI';
	const jobOf = (r: Run) => r.job ?? MANUAL;

	const rows = (cur: Run[], prev: Run[], key: (r: Run) => string, nodes: (k: string) => number): CostRow[] => {
		const p = group(prev, key, (r) => r.credits);
		const keys = [...new Set(cur.map(key))];
		return keys
			.map((k) => {
				const mine = cur.filter((r) => key(r) === k);
				return { key: k, credits: mine.reduce((t, r) => t + r.credits, 0), previous: p.get(k) ?? 0, seconds: mine.reduce((t, r) => t + r.seconds, 0), runs: mine.length, nodes: nodes(k) };
			})
			.sort((a, b) => b.credits - a.credits);
	};
	const jobs = rows(current, before, jobOf, (k) => models.filter((m) => m.job === k).length);
	const sizes = rows(current, before, (r) => r.size, (k) => models.filter((m) => JOBS.find((j) => j[0] === m.job)?.[3] === k).length);

	// A job's credits split over its models by weight.
	const jobTotals = (rs: Run[]) => group(rs.filter((r) => r.job), (r) => r.job!, (r) => r.credits);
	const jobRuns = group(current.filter((r) => r.job), (r) => r.job!, () => 1);
	const cur = jobTotals(current);
	const prev = jobTotals(before);
	const weights = group(models, (m) => m.job, (m) => m.weight);
	const costModels: CostModel[] = models
		.map((m) => {
			const share = m.weight / (weights.get(m.job) ?? 1);
			const credits = (cur.get(m.job) ?? 0) * share;
			const n = jobRuns.get(m.job) ?? 0;
			return { key: m.key, folder: m.folder, job: m.job, credits, previous: (prev.get(m.job) ?? 0) * share * (m.key.includes('_enriched') ? 0.6 : 1), seconds: (credits / RATE[JOBS.find((j) => j[0] === m.job)![3]]) * 3600, runs: n, nodes: 1, queries: m.queries * n, bytesScanned: m.bytes * n };
		})
		.filter((m) => m.runs > 0)
		.sort((a, b) => b.credits - a.credits);
	const folders: CostRow[] = [...group(costModels, (m) => m.folder, () => 0).keys()]
		.map((f) => {
			const mine = costModels.filter((m) => m.folder === f);
			return { key: f, credits: mine.reduce((t, m) => t + m.credits, 0), previous: mine.reduce((t, m) => t + m.previous, 0), seconds: mine.reduce((t, m) => t + m.seconds, 0), runs: Math.max(...mine.map((m) => m.runs)), nodes: mine.length };
		})
		.sort((a, b) => b.credits - a.credits);

	const daily: CostDay[] = [];
	const firstDay = Math.floor(since / 86_400_000) + 1;
	for (let d = firstDay; d <= Math.floor(now / 86_400_000); d++) {
		const mine = current.filter((r) => Math.floor(r.at / 86_400_000) === d);
		daily.push({ date: new Date(d * 86_400_000).toISOString().slice(0, 10), credits: mine.reduce((t, r) => t + r.credits, 0), byJob: Object.fromEntries(group(mine, jobOf, (r) => r.credits)), bySize: Object.fromEntries(group(mine, (r) => r.size, (r) => r.credits)) });
	}

	const credits = current.reduce((t, r) => t + r.credits, 0);
	return {
		days,
		since: new Date(since).toISOString(),
		price: PRICES[source],
		credits,
		previous: before.reduce((t, r) => t + r.credits, 0),
		seconds: current.reduce((t, r) => t + r.seconds, 0),
		runCount: current.length,
		incompleteRuns: current.filter((r) => r.status === 'running').length,
		jobs,
		folders,
		sizes,
		models: costModels,
		runs: [...current].sort((a, b) => b.credits - a.credits).map(({ at: _at, ...r }) => r),
		daily
	};
}
