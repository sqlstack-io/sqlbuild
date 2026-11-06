<script lang="ts">
	import { page } from '$app/state';
	import LockIcon from '@lucide/svelte/icons/lock';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ConfigForm from '$lib/config/config-form.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const name = $derived(page.params.id ?? '');
	const connections = createResource((client, scope) => client.listWarehouseConnections(scope));
	const warehouse = $derived(connections.data?.find((c) => c.name === name) ?? null);
</script>

<AppTopbar title="Connections">
	<Button variant="outline" size="sm" onclick={() => connections.refresh()}>↻ Refresh</Button>
</AppTopbar>

<div class="flex min-h-0 flex-1 flex-col">
	<div class="flex items-center gap-3 border-b border-border px-6 py-3.5">
		<a href="/connections" class="text-muted-foreground hover:text-foreground text-[13px]">← Connections</a>
		<span class="font-display text-[17px] font-semibold">{name}</span>
		{#if warehouse}
			<span class="text-muted-foreground text-[12px]">Warehouse · {warehouse.adapter} · target {warehouse.target}</span>
		{/if}
	</div>

	{#if connections.error}
		<div class="text-[var(--sb-error)] px-6 py-4 text-[13px]">Couldn't load connections: {connections.error}</div>
	{:else if connections.data && !warehouse}
		<div class="text-muted-foreground px-6 py-4 text-[13px]">
			{app.location?.name} has no warehouse connection named <code>{name}</code>. Looking for an API or storage connection? See <a class="text-[var(--primary)]" href="/providers/{name}">Providers</a>.
		</div>
	{:else if warehouse}
		<div class="flex min-h-0 flex-1">
			<div class="min-h-0 flex-1 overflow-auto">
				<div class="flex max-w-[680px] flex-col p-6">
					<div class="mb-5">
						<ConnectionTest scope={{ locationId: app.locationId }} connection={warehouse.name} last={warehouse.lastTest} />
					</div>
					<div class="text-[var(--sb-text-faint)] mb-4 flex items-center gap-2 rounded-[2px] border border-[var(--border-subtle)] bg-[var(--sb-surface-low)] px-3 py-2 text-[11.5px]">
						<LockIcon size={12} />
						<span>Read-only. Set in <code>{warehouse.sourceFile}</code>; secrets show only where they come from.</span>
					</div>
					<ConfigForm schema={warehouse.settings} values={warehouse.values} readOnly />
				</div>
			</div>

			<aside class="w-[300px] shrink-0 overflow-auto border-l border-border p-5">
				<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Warehouse connection</div>
				<KeyValues
					rows={[
						{ label: 'Adapter', value: warehouse.adapter },
						{ label: 'Target', value: warehouse.target },
						{ label: 'Used for', value: warehouse.purpose === 'catalog' ? 'Reading SQLBuild’s own tables back (catalog_connection)' : 'Builds, and reading SQLBuild’s own tables back' },
						{ label: 'Defined in', value: warehouse.sourceFile, mono: true }
					]}
				/>
			</aside>
		</div>
	{:else}
		<div class="text-muted-foreground px-6 py-4 text-[13px]">Loading…</div>
	{/if}
</div>
