<script lang="ts">
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import PageBody from '$lib/components/page-body.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Loading from '$lib/components/loading.svelte';
	import { createCostState } from './state.svelte';
	import type { Period } from './types';
	import VariantA from './variant-a.svelte';
	import VariantB from './variant-b.svelte';
	import VariantC from './variant-c.svelte';
	import VariantD from './variant-d.svelte';

	// Estimated query cost of the target's runs. Mockup: `?variant=a|b|c|d` picks a layout (CHI-1650).
	const s = createCostState();
	const cost = $derived(s.cost.data);
</script>

<AppTopbar title="Cost">
	<SegmentedFilter label="Period" value={String(s.days)} options={[{ value: '7', label: '7 days' }, { value: '30', label: '30 days' }]} onselect={(v) => (s.days = Number(v) as Period)} />
	<RefreshButton onclick={() => s.cost.refresh()} />
</AppTopbar>

<PageBody>
	{#if s.cost.error && !cost}
		<p class="err">Couldn't load cost: {s.cost.error}</p>
	{:else if !cost}
		<Loading />
	{:else if s.variant === 'b'}
		<VariantB {cost} />
	{:else if s.variant === 'c'}
		<VariantC {cost} />
	{:else if s.variant === 'd'}
		<VariantD {cost} />
	{:else}
		<VariantA {cost} />
	{/if}
</PageBody>

<style>
	.err { color: var(--sb-error); font-size: 12.5px; }
</style>
