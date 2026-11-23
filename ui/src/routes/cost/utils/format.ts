// Cost figures in dollars, with credits kept for tooltips. Pure; no side effects.
import type { CreditPrice, PriceSource } from '../types';

export const priceSourceLabel: Record<PriceSource, string> = {
	config: 'Project price',
	rate_sheet: 'Rate sheet',
	list_price: 'List price'
};

/** Where the price came from, for the price's tooltip (an explanation, not a label). */
export const priceSourceHelp: Record<PriceSource, string> = {
	config: 'Set by usd_per_credit under [cost] in sqlbuild_project.toml.',
	rate_sheet: "Your contract's price per credit, from Snowflake's rate sheet (ORGANIZATION_USAGE.RATE_SHEET_DAILY).",
	list_price: "Snowflake's public on-demand price for the account's edition and region. Set usd_per_credit under [cost] in sqlbuild_project.toml, or grant the connection's role ORGANIZATION_USAGE, for an exact price."
};

export const isEstimate = (price: CreditPrice) => price.source === 'list_price';

/** Dollars: cents below $100, whole dollars above. */
export function usd(credits: number, price: CreditPrice): string {
	const v = credits * price.usdPerCredit;
	if (v > 0 && v < 0.01) return '< $0.01';
	return v < 100 ? `$${v.toFixed(2)}` : `$${Math.round(v).toLocaleString('en')}`;
}

export function creditsText(credits: number): string {
	return `${credits < 10 ? credits.toFixed(2) : credits < 1000 ? credits.toFixed(1) : Math.round(credits).toLocaleString('en')} credits`;
}

/** A figure's tooltip: its credits and the price that made it dollars. */
export function costTitle(credits: number, price: CreditPrice): string {
	return `${creditsText(credits)} × $${price.usdPerCredit.toFixed(2)} (${priceSourceLabel[price.source].toLowerCase()}${isEstimate(price) ? ', estimate' : ''})`;
}

/** The price as a short fact: "$2.73 per credit", or "$3.90 est." for a list price. */
export const priceText = (price: CreditPrice) => `$${price.usdPerCredit.toFixed(2)} per credit${isEstimate(price) ? ' est.' : ''}`;

/** Change against the period before, or null without a previous figure. */
export function change(credits: number, previous: number): { text: string; tone: 'up' | 'down' | 'flat' } | null {
	if (previous <= 0) return null;
	const pct = Math.round(((credits - previous) / previous) * 100);
	if (Math.abs(pct) < 3) return { text: '±0%', tone: 'flat' };
	return { text: `${pct > 0 ? '+' : '−'}${Math.abs(pct)}%`, tone: pct > 0 ? 'up' : 'down' };
}

export const sharePct = (part: number, total: number) => (total <= 0 ? '—' : part > 0 && part / total < 0.005 ? '<1%' : `${Math.round((part / total) * 100)}%`);

export function hours(seconds: number): string {
	if (seconds <= 0) return '—';
	if (seconds < 3600) return `${Math.max(1, Math.round(seconds / 60))}m`;
	return seconds < 36_000 ? `${(seconds / 3600).toFixed(1)}h` : `${Math.round(seconds / 3600).toLocaleString('en')}h`;
}

export function bytes(n: number): string {
	if (n >= 1e12) return `${(n / 1e12).toFixed(1)} TB`;
	if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`;
	return `${Math.round(n / 1e6)} MB`;
}

/** "Mon 17" for a chart's day axis. */
export function dayLabel(date: string, long = false): string {
	const d = new Date(`${date}T00:00:00Z`);
	return d.toLocaleDateString('en-GB', long ? { weekday: 'short', day: 'numeric', month: 'short', timeZone: 'UTC' } : { day: 'numeric', timeZone: 'UTC' });
}
