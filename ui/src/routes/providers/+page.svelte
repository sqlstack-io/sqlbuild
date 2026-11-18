<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { openRow, to } from '$lib/paths';
	import PuzzleIcon from '@lucide/svelte/icons/puzzle';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { usedBySummary } from './utils';

	const providers = createResource((client, scope) => client.listProviders(scope));

	let query = $state('');
	const all = $derived(providers.data ?? []);
	const q = $derived(query.trim().toLowerCase());
	const shown = $derived(all.filter((p) => !q || `${p.name} ${p.className} ${p.description}`.toLowerCase().includes(q)));
	const tableColumns: TableColumn[] = [
		{ id: 'provider', label: 'Provider', minWidth: 220 },
		{ id: 'class', label: 'Class', width: '24%' },
		{ id: 'used', label: 'Used by', width: '22%' },
		{ id: 'file', label: 'Defined in', width: '22%' }
	];
</script>

<AppTopbar title="Providers">
	<RefreshButton onclick={() => providers.refresh()} />
</AppTopbar>

<ListToolbar loading={providers.data === undefined} bind:query placeholder="Search providers" total="{shown.length} of {all.length} providers">
</ListToolbar>

<DataTable columns={tableColumns} rows={shown} key={(p) => p.name} resource={providers} empty={all.length ? 'No providers match the search.' : 'No providers.'}>
	{#snippet row(p)}
				<tr onclick={(e) => openRow(e, `/providers/${p.name}`)}>
					<td>
						<div class="cell-name">
							<PuzzleIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								<a href={to(`/providers/${p.name}`)}>{p.name}</a>
								<div class="cell-desc">{p.description}</div>
							</div>
						</div>
					</td>
					<td class="muted"><code>{p.className}</code></td>
					<td class="muted">{usedBySummary(p)}</td>
					<td class="muted"><code>{p.sourceFile}</code></td>
				</tr>
	{/snippet}
</DataTable>

<style>
</style>
