<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Loading from '$lib/components/loading.svelte';
	import { reportsCost } from '$lib/operate/cost';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { EVALUATE_DELAY_MS, createSelection } from '$lib/state/selection.svelte';
	import { untrack } from 'svelte';
	import CostView from './cost-view.svelte';
	import { placeOf } from './utils/selector';

	// Estimated query cost of a selection over 7 or 30 days, for locations whose adapter reports it.
	const PERIODS = [7, 30] as const;
	const days = $derived(page.url.searchParams.get('days') === '30' ? 30 : 7);
	const selection = createSelection({ url: true, follow: true });
	const reports = $derived(reportsCost(app.location?.target.adapter));
	/** The part of the selection the server resolves; the job, folder and model are looked up in the read. */
	const base = $derived(placeOf(selection.select, selection.exclude).base.join(' '));
	// Read once typing pauses, not on every key.
	let asked = $state(untrack(() => ({ select: base, exclude: selection.exclude })));
	$effect(() => {
		const next = { select: base, exclude: selection.exclude };
		if (next.select === untrack(() => asked.select) && next.exclude === untrack(() => asked.exclude)) return;
		const timer = setTimeout(() => (asked = next), EVALUATE_DELAY_MS);
		return () => clearTimeout(timer);
	});
	const cost = createResource(
		(client, scope) => (reports ? client.readCostSelection(scope, { days, ...asked }) : Promise.resolve(null)),
		['runs', 'warehouse_sync', 'snapshot'],
		{ raw: true }
	);
	function setDays(value: string) {
		const url = new URL(page.url);
		if (value === '30') url.searchParams.set('days', '30');
		else url.searchParams.delete('days');
		void goto(url, { noScroll: true, keepFocus: true });
	}
</script>

<AppTopbar title="Cost">
	{#if reports}
		<SegmentedFilter label="Period" value={String(days)} options={PERIODS.map((d) => ({ value: String(d), label: `${d} days` }))} onselect={setDays} />
		<RefreshButton onclick={() => cost.refresh()} />
	{/if}
</AppTopbar>

{#if !reports}
	<div class="empty">Query cost is reported for Snowflake only.</div>
{:else if cost.data}
	<div class="cost-page"><CostView read={cost.data} {selection} {days} /></div>
{:else if cost.error}
	<div class="empty" role="alert">{cost.error}</div>
{:else}
	<Loading />
{/if}

<style>
	.cost-page { display: flex; min-height: 0; flex: 1; flex-direction: column; }
	.empty { display: grid; flex: 1; place-items: center; padding: 32px; color: var(--muted-foreground); font-size: 13px; }
</style>
