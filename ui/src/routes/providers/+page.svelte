<script lang="ts">
	import { goto } from '$app/navigation';
	import PuzzleIcon from '@lucide/svelte/icons/puzzle';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { usedBySummary } from './utils';

	const providers = createResource((client, scope) => client.listProviders(scope));

	let query = $state('');
	const all = $derived(providers.data ?? []);
	const q = $derived(query.trim().toLowerCase());
	const shown = $derived(all.filter((p) => !q || `${p.name} ${p.type} ${p.description}`.toLowerCase().includes(q)));
</script>

<AppTopbar title="Providers">
	<Button variant="outline" size="sm" onclick={() => providers.refresh()}>↻ Refresh</Button>
</AppTopbar>

<ListToolbar bind:query placeholder="Search providers" total="{shown.length} of {all.length} providers">
	{#snippet actions()}
		<span class="note">Connections to APIs, storage and webhooks that models, loaders and sensors use. Declared in code; read-only here.</span>
	{/snippet}
</ListToolbar>

<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th>Provider</th>
				<th>Type</th>
				<th>Used by</th>
				<th>Defined in</th>
			</tr>
		</thead>
		<tbody>
			{#each shown as p (p.name)}
				<tr onclick={() => goto(`/providers/${p.name}`)}>
					<td>
						<div class="cell-name">
							<PuzzleIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								<a href="/providers/{p.name}" onclick={(e) => e.stopPropagation()}>{p.name}</a>
								<div class="cell-desc">{p.description}</div>
							</div>
						</div>
					</td>
					<td><code>{p.type}</code></td>
					<td class="muted">{usedBySummary(p)}</td>
					<td class="muted"><code>{p.sourceFile}</code></td>
				</tr>
			{:else}
				<LoadRow colspan={4} resource={providers} empty={all.length ? 'No providers match the search.' : 'This project declares no providers. Warehouse connections are under Connections.'} />
			{/each}
		</tbody>
	</table>
</div>

<style>
	.note {
		margin-left: 8px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
</style>
