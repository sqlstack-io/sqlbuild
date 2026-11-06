<script lang="ts">
	import { goto } from '$app/navigation';
	import type { CatalogNodeDetail } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeLabel, checkOutcomeTone } from '$lib/status';
	import { evaluationModeLabel, runScopePhaseLabel } from '$lib/build-area/labels';
	import { checkValue } from '../../checks/utils';

	let { node }: { node: CatalogNodeDetail } = $props();
</script>

<table class="sb-list">
	<thead><tr><th>Check</th><th>Outcome</th><th>Severity</th><th>Checks</th><th>Result type</th><th class="num">Result</th><th>Last checked</th></tr></thead>
	<tbody>
		{#each node.checkResults as c (c.id)}
			{@const o = c.latest?.outcome ?? 'skipped'}
			<tr onclick={() => goto(`/checks/${c.id}`)}>
				<td><a class="mono" href="/checks/{c.id}">{c.name}</a>{#if c.column}<div class="cell-desc">column {c.column}</div>{/if}</td>
				<td><StatusLabel tone={checkOutcomeTone[o]} label={checkOutcomeLabel[o]} /></td>
				<td class="muted capitalize">{c.severity}</td>
				<td class="muted">{runScopePhaseLabel[c.runScopePhase]}</td>
				<td class="muted">{evaluationModeLabel[c.evaluationMode]}</td>
				<td class="num">{checkValue(c.latest, c.evaluationMode)}</td>
				<td><RelativeTime value={formatAgo(c.latest?.evaluatedAt, app.now)} exact={formatStamp(c.latest?.evaluatedAt)} /></td>
			</tr>
		{:else}
			<tr><td colspan="7" class="empty">No audits or checks attach to this node.</td></tr>
		{/each}
	</tbody>
</table>

<style>
	.mono {
		color: var(--foreground);
		font: 12px var(--font-mono);
	}
	.muted {
		color: var(--muted-foreground);
	}
</style>
