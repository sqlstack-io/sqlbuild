<script lang="ts">
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ConnectionTest from '$lib/components/platform/connection-test.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import type { SettingField, WarehouseConnection } from '$lib/api';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const connections = createResource((client, scope) => client.listWarehouseConnections(scope));

	const warehouse = $derived(connections.data ?? []);

	function shownValue(c: WarehouseConnection, field: SettingField): string {
		if (field.type === 'secret') return field.secretSource ?? 'not set';
		const value = c.values[field.key] ?? field.default;
		return value === undefined ? '—' : String(value);
	}
</script>

<AppTopbar title="Connections">
	<Button variant="outline" size="sm" onclick={() => connections.refresh()}>↻ Refresh</Button>
</AppTopbar>

<div class="min-h-0 flex-1 overflow-auto px-6 pt-5">
	<p class="lede">
		How SQLBuild reaches the warehouse for <b>{app.location?.name}</b> (target <b>{app.target?.name}</b>). Set in the project config and read-only here; secrets show only where they come from, never their value.
		APIs, storage and webhooks are under <a href="/providers">Providers</a>.
	</p>

	{#if connections.error}
		<p class="err">Couldn't load connections: {connections.error} <button class="retry" onclick={() => connections.refresh()}>Retry</button></p>
	{/if}

	<Panel title="Warehouse" note="Test a connection to check SQLBuild can sign in and reach the warehouse (the same check as sqb debug)">
		{#each warehouse as c (c.name)}
			<div class="warehouse">
				<div class="warehouse-head">
					<DatabaseIcon size={15} strokeWidth={1.75} />
					<a href="/connections/{c.name}" class="name">{c.name}</a>
					<span class="muted" title={c.purpose === 'catalog' ? 'catalog_connection' : undefined}>{c.adapter} · {c.purpose === 'catalog' ? 'used to read SQLBuild’s own tables back (warehouse sync)' : `builds target ${c.target}`} · {c.sourceFile}</span>
				</div>
				<dl class="settings">
					{#each c.settings as f (f.key)}
						<div title={f.description}>
							<dt>{f.key}</dt>
							<dd class:secret={f.type === 'secret'} class:default={f.type !== 'secret' && c.values[f.key] === undefined}>{shownValue(c, f)}</dd>
						</div>
					{/each}
				</dl>
				<ConnectionTest scope={{ locationId: app.locationId }} connection={c.name} last={c.lastTest} />
			</div>
		{:else}
			<p class="muted">{connections.data ? 'This location’s target has no warehouse connection. Add one to the target in the project config.' : connections.error ? '' : 'Loading…'}</p>
		{/each}
	</Panel>

</div>

<style>
	.lede {
		margin-bottom: 18px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.err {
		margin-bottom: 16px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.retry {
		margin-left: 8px;
		color: var(--primary);
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.warehouse {
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin-bottom: 10px;
		padding: 12px 14px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--card);
	}
	.warehouse-head {
		display: flex;
		align-items: center;
		gap: 9px;
	}
	.warehouse-head :global(svg) {
		color: var(--muted-foreground);
	}
	.name {
		font-size: 13px;
		font-weight: 500;
	}
	.name:hover {
		color: var(--primary);
	}
	.settings {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
		gap: 8px 16px;
		margin: 0;
		font-size: 12.5px;
	}
	.settings dt {
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.settings dd {
		margin: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.settings dd.default {
		color: var(--muted-foreground);
	}
	.settings dd.secret {
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.lede a {
		color: var(--primary);
	}
</style>
