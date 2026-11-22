<script lang="ts">
	// The nodes a plan can't run because a provider they use is missing secrets: one problem block per
	// provider (what it needs, what it stops, how to fix it), then a row per node with why it can't run.
	import type { PlanCantRun } from '$lib/api';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { to } from '$lib/paths';
	import { cantRunRows, keyName } from './cant-run';
	import CantRunProblem from './cant-run-problem.svelte';

	/** `kindOf` and `upstreamOf` read the catalog; `then` ends the fix, such as "plan again". */
	let {
		entries,
		kindOf = () => '',
		upstreamOf = () => [],
		then = 'plan again'
	}: {
		entries: PlanCantRun[];
		kindOf?: (key: string) => string;
		upstreamOf?: (key: string) => string[];
		then?: string;
	} = $props();

	const rows = $derived(cantRunRows(entries, upstreamOf));
	const href = (key: string) => to(`/catalog/${key}`);
	const columns: TableColumn[] = [
		{ id: 'state', label: '', width: '34px' },
		{ id: 'node', label: 'Node', minWidth: 160 },
		{ id: 'kind', label: 'Kind', width: '100px' },
		{ id: 'why', label: "Why it can't run", minWidth: 200, truncate: false }
	];
</script>

<div class="cant-run" data-testid="plan-cant-run">
	{#each entries as e (e.provider)}<CantRunProblem entry={e} {then} />{/each}
	<DataTable class="static compact" embedded {columns} {rows} key={(r) => r.key} empty="No nodes.">
		{#snippet row(r)}
			<tr class:uses={r.uses}>
				<td><span class="state" class:hollow={!r.uses} aria-label={r.uses ? 'uses the provider' : 'reads a node that can’t run'}>{r.uses ? '✕' : '!'}</span></td>
				<td><a href={href(r.key)} class="mono">{keyName(r.key)}</a></td>
				<td class="faint">{kindOf(r.key)}</td>
				<td>{r.why}</td>
			</tr>
		{/snippet}
	</DataTable>
</div>

<style>
	.cant-run {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.state {
		display: inline-grid;
		width: 16px;
		height: 16px;
		place-items: center;
		border-radius: 2px;
		background: var(--sb-error);
		color: var(--background);
		font-size: 10px;
		font-weight: 700;
	}
	.state.hollow {
		border: 1px solid var(--sb-error);
		background: transparent;
		color: var(--sb-error);
	}
	.mono {
		font-family: var(--font-mono);
	}
	.faint {
		color: var(--muted-foreground);
	}
</style>
