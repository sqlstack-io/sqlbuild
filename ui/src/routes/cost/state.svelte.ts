import { page } from '$app/state';
import { createResource } from '$lib/state/resource.svelte';
import { loadCost } from './api';
import type { Period, PriceSource, Variant } from './types';

const VARIANTS: readonly Variant[] = ['a', 'b', 'c', 'd', 'e', 'f'];
/** Mockups show the rate sheet in some variants and the list-price estimate in others; `?price=` overrides. */
const DEFAULT_PRICE: Record<Variant, PriceSource> = { a: 'rate_sheet', b: 'list_price', c: 'rate_sheet', d: 'list_price', e: 'rate_sheet', f: 'list_price' };
const PRICE_PARAM: Record<string, PriceSource> = { rate: 'rate_sheet', est: 'list_price', config: 'config' };

export function createCostState() {
	// `?days=30` opens the 30-day period, for screenshots.
	let days = $state<Period>(page.url.searchParams.get('days') === '30' ? 30 : 7);
	const variant = $derived<Variant>(VARIANTS.find((v) => v === page.url.searchParams.get('variant')) ?? 'a');
	const source = $derived<PriceSource>(PRICE_PARAM[page.url.searchParams.get('price') ?? ''] ?? DEFAULT_PRICE[variant]);
	const cost = createResource((client, scope) => loadCost(client, scope, days, source), ['runs', 'warehouse_sync', 'snapshot']);
	return {
		get days() {
			return days;
		},
		set days(v: Period) {
			days = v;
		},
		get variant() {
			return variant;
		},
		cost
	};
}
