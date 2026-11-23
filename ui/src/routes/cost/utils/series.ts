// Stacks for the daily chart: the top groups by cost, the rest as "Other". Pure; no side effects.
import type { CostDay } from '../types';

export type Stack = { keys: string[]; days: { date: string; total: number; parts: { key: string; credits: number }[] }[]; max: number };
export const OTHER = 'Other';

export function stacks(daily: CostDay[], by: 'byJob' | 'bySize', top = 6): Stack {
	const totals = new Map<string, number>();
	for (const d of daily) for (const [k, v] of Object.entries(d[by])) totals.set(k, (totals.get(k) ?? 0) + v);
	const ranked = [...totals].sort((a, b) => b[1] - a[1]).map(([k]) => k);
	const keys = ranked.length > top + 1 ? [...ranked.slice(0, top), OTHER] : ranked;
	const days = daily.map((d) => {
		const parts = keys.map((key) => ({
			key,
			credits: key === OTHER ? Object.entries(d[by]).filter(([k]) => !keys.includes(k)).reduce((t, [, v]) => t + v, 0) : (d[by][key] ?? 0)
		}));
		return { date: d.date, total: d.credits, parts };
	});
	return { keys, days, max: Math.max(1, ...days.map((d) => d.total)) };
}

/** One series per key, for a row's sparkline. */
export function seriesOf(daily: CostDay[], key: string): number[] {
	return daily.map((d) => d.byJob[key] ?? 0);
}
