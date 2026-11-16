<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { openRow, to } from '$lib/paths';
	import type { CatalogNodeDetail } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { app } from '$lib/state/app.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeLabel, checkOutcomeTone } from '$lib/status';
	import { evaluationModeLabel, runScopePhaseLabel } from '$lib/build-area/labels';
	import { checkValue } from '../../checks/utils';

	let { node }: { node: CatalogNodeDetail } = $props();
	const checkColumns: TableColumn[] = [
		{ id: 'check', label: 'Check', minWidth: 200 },
		{ id: 'outcome', label: 'Outcome', width: '120px' },
		{ id: 'severity', label: 'Severity', width: '100px' },
		{ id: 'phase', label: 'Checks', width: '120px' },
		{ id: 'mode', label: 'Result type', width: '130px' },
		{ id: 'result', label: 'Result', width: '100px', align: 'right' },
		{ id: 'checked', label: 'Last checked', width: '130px' }
	];
</script>

<DataTable embedded columns={checkColumns} rows={node.checkResults} key={(c) => c.id} empty="No audits or checks attach to this node.">
	{#snippet row(c)}
			{@const o = c.latest?.outcome ?? 'skipped'}
			<tr onclick={(e) => openRow(e, `/checks/${c.id}`)}>
				<td><a class="mono" href={to(`/checks/${c.id}`)}>{c.name}</a>{#if c.column}<div class="cell-desc">column {c.column}</div>{/if}</td>
				<td><StatusLabel tone={checkOutcomeTone[o]} label={checkOutcomeLabel[o]} /></td>
				<td class="muted capitalize">{c.severity}</td>
				<td class="muted">{runScopePhaseLabel[c.runScopePhase]}</td>
				<td class="muted">{evaluationModeLabel[c.evaluationMode]}</td>
				<td class="num">{checkValue(c.latest, c.evaluationMode)}</td>
				<td><RelativeTime value={formatAgo(c.latest?.evaluatedAt, app.now)} exact={formatStamp(c.latest?.evaluatedAt)} /></td>
			</tr>
	{/snippet}
</DataTable>

<style>
	.mono {
		color: var(--foreground);
		font: 12px var(--font-mono);
	}
	.muted {
		color: var(--muted-foreground);
	}
</style>
