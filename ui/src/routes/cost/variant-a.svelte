<script lang="ts">
	import { to } from '$lib/paths';
	import CostHead from './cost-head.svelte';
	import RankPanel from './rank-panel.svelte';
	import RunsPanel from './runs-panel.svelte';
	import type { CostPage } from './types';

	// A: totals, then every breakdown as a ranked table, three to a row on a wide screen.
	let { cost }: { cost: CostPage } = $props();
	const jobHref = (k: string) => (k === 'Manual or CLI' ? undefined : to(`/jobs/${k}`));
</script>

<CostHead {cost} />
<div class="grid">
	<RankPanel title="By job" rows={cost.jobs} total={cost.credits} price={cost.price} name="Job" count={{ label: 'Runs', value: (r) => r.runs }} href={(r) => jobHref(r.key)} />
	<RankPanel title="By folder" rows={cost.folders} total={cost.credits} price={cost.price} name="Folder" count={{ label: 'Models', value: (r) => r.nodes }} href={(r) => to(`/catalog?folder=${r.key}`)} />
	<RankPanel title="By warehouse size" rows={cost.sizes} total={cost.credits} price={cost.price} name="Size" count={{ label: 'Runs', value: (r) => r.runs }} />
	<RunsPanel runs={cost.runs} price={cost.price} />
	<RankPanel title="Most expensive models" rows={cost.models} total={cost.credits} price={cost.price} name="Model" count={{ label: 'Builds', value: (r) => r.runs }} sub={(r) => r.folder} href={(r) => to(`/catalog/${r.key}`)} />
</div>

<style>
	.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 470px), 1fr)); gap: 0 28px; }
	/* One column on a narrow screen: keep names near their numbers. */
	.grid > :global(.panel) { max-width: 760px; max-width: 2000px; }
</style>
