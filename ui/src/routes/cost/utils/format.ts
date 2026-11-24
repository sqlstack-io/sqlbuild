// Cost figures: dollars when the read has a price, else credits, so no price is ever guessed; the
// credits and the price in tooltips. Number formats are made once. Pure; no side effects.

import type { CostPrice, PriceSource } from '$lib/api';
import { PRICE_SOURCE_TEXT } from '$lib/operate/cost';

const whole = new Intl.NumberFormat('en', { maximumFractionDigits: 0 });
const cents = new Intl.NumberFormat('en', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const tenths = new Intl.NumberFormat('en', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const shortDay = new Intl.DateTimeFormat('en-GB', { day: 'numeric', timeZone: 'UTC' });
const longDay = new Intl.DateTimeFormat('en-GB', { weekday: 'short', day: 'numeric', month: 'short', timeZone: 'UTC' });

/** A count, such as runs or queries. */
export const count = (n: number) => whole.format(n);

export function creditsText(credits: number): string {
	if (credits > 0 && credits < 0.01) return '< 0.01 credits';
	return `${credits < 10 ? cents.format(credits) : credits < 1000 ? tenths.format(credits) : whole.format(credits)} credits`;
}

/** The figure: dollars (cents below $100, whole dollars above) with a price, else credits. */
export function money(credits: number, price: CostPrice | null): string {
	if (!price) return creditsText(credits);
	const v = credits * price.usdPerCredit;
	if (v > 0 && v < 0.01) return '< $0.01';
	return `$${v < 100 ? cents.format(v) : whole.format(v)}`;
}

/** A figure's tooltip: its credits and the price that made it dollars. */
export function costTitle(credits: number, price: CostPrice | null): string {
	return price ? `${creditsText(credits)} × $${cents.format(price.usdPerCredit)} per credit (${PRICE_SOURCE_TEXT[price.source]})` : creditsText(credits);
}

/** Where a price came from, for its help tip. */
export const priceHelp = (price: CostPrice): string => PRICE_SOURCE_HELP[price.source];

const PRICE_SOURCE_HELP: Record<PriceSource, string> = {
	config: 'Set by usd_per_credit under [cost] in sqlbuild_project.toml.',
	rate_sheet: "Your contract's price per credit, from Snowflake's rate sheet (ORGANIZATION_USAGE.RATE_SHEET_DAILY).",
	list_price:
		"An estimate: Snowflake's public on-demand price for the account's region, at Enterprise edition. Set usd_per_credit under [cost] in sqlbuild_project.toml, or let the connection's role read ORGANIZATION_USAGE, for your contract's price.",
	mixed: 'Builds in this period were priced from more than one source.'
};

/** Change against the period before, or null without a previous figure. */
export function change(credits: number, previous: number): { text: string; tone: 'up' | 'down' | 'flat' } | null {
	if (previous <= 0) return null;
	const pct = Math.round(((credits - previous) / previous) * 100);
	if (Math.abs(pct) < 3) return { text: '±0%', tone: 'flat' };
	return { text: `${pct > 0 ? '+' : '−'}${whole.format(Math.abs(pct))}%`, tone: pct > 0 ? 'up' : 'down' };
}

export const sharePct = (part: number, total: number) => (total <= 0 ? '—' : part > 0 && part / total < 0.005 ? '<1%' : `${Math.round((part / total) * 100)}%`);

/** Warehouse time: minutes below an hour, then hours. */
export function hours(seconds: number): string {
	if (seconds <= 0) return '—';
	if (seconds < 3600) return `${Math.max(1, Math.round(seconds / 60))}m`;
	return seconds < 36_000 ? `${tenths.format(seconds / 3600)}h` : `${whole.format(seconds / 3600)}h`;
}

export function bytes(n: number): string {
	if (n >= 1e12) return `${tenths.format(n / 1e12)} TB`;
	if (n >= 1e9) return `${tenths.format(n / 1e9)} GB`;
	return `${whole.format(n / 1e6)} MB`;
}

/** "17" for a chart's day axis, or "Mon, 17 Nov" in full. */
export function dayLabel(date: string, long = false): string {
	if (!date) return '';
	return (long ? longDay : shortDay).format(new Date(`${date}T00:00:00Z`));
}
