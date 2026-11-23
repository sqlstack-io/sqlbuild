<script lang="ts">
	import { to } from '$lib/paths';
	import CostLine from './cost-line.svelte';
	import DailyChart from './daily-chart.svelte';
	import RankPanel from './rank-panel.svelte';
	import RunsPanel from './runs-panel.svelte';
	import type { CostPage } from './types';

	// B: the trend first, then the top five of each breakdown with "Show all".
	let { cost }: { cost: CostPage } = $props();
	const jobHref = (k: string) => (k === 'Manual or CLI' ? undefined : to(`/jobs/${k}`));
</script>

<CostLine {cost} />
<DailyChart {cost} />
<div class="grid">
	<RankPanel title="By job" rows={cost.jobs} total={cost.credits} price={cost.price} name="Job" count={{ label: 'Runs', value: (r) => r.runs }} href={(r) => jobHref(r.key)} limit={5} bar={false} showChange />
	<RankPanel title="Most expensive models" rows={cost.models} total={cost.credits} price={cost.price} name="Model" count={{ label: 'Builds', value: (r) => r.runs }} sub={(r) => r.folder} href={(r) => to(`/catalog/${r.key}`)} limit={5} bar={false} />
	<RunsPanel runs={cost.runs} price={cost.price} limit={5} />
	<RankPanel title="By folder" rows={cost.folders} total={cost.credits} price={cost.price} name="Folder" count={{ label: 'Models', value: (r) => r.nodes }} limit={5} bar={false} showChange />
	<RankPanel title="By warehouse size" rows={cost.sizes} total={cost.credits} price={cost.price} name="Size" count={{ label: 'Runs', value: (r) => r.runs }} limit={5} bar={false} showChange />
</div>

<style>
	.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 470px), 1fr)); gap: 0 28px; }
	/* One column on a narrow screen: keep names near their numbers. */
	.grid > :global(.panel) { max-width: 760px; }
</style>
