<script lang="ts">
	import HelpTip from '$lib/components/help-tip.svelte';
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import { page } from '$app/state';
	import LockIcon from '@lucide/svelte/icons/lock';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';
	import { usedCount } from '../utils';
	import UsedByGraph from './used-by-graph.svelte';

	const name = $derived(page.params.name ?? '');
	const providers = createResource((client, scope) => client.listProviders(scope));
	const provider = $derived(providers.data?.find((p) => p.name === name) ?? null);
	// Node kinds come from the Catalog where it lists the node, e.g. a source with a loader is a loader.
	const catalog = useCatalogNodes();
	const kindByKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n.kind as string])));

	type Tab = 'settings' | 'uses';
	let tab = $state<Tab>('settings');
	const settingColumns: TableColumn[] = [
		{ id: 'setting', label: 'Setting', minWidth: 200, truncate: false },
		{ id: 'type', label: 'Type', width: '140px' },
		{ id: 'env', label: 'Environment variable', width: '34%' },
		{ id: 'required', label: 'Required', width: '100px' }
	];
</script>

<AppTopbar title="Providers" subject="{name} · Providers">
	<RefreshButton onclick={() => providers.refresh()} />
</AppTopbar>

<div class="flex min-h-0 flex-1 flex-col">
	<div class="flex items-center gap-3 border-b border-border px-6 py-3.5">
		<a href={to(`/providers`)} class="text-muted-foreground hover:text-foreground text-[13px]">← Providers</a>
		<span class="font-display text-[17px] font-semibold">{name}</span>
		{#if provider}<span class="text-muted-foreground font-mono text-[12px]">{provider.className}</span>{/if}
	</div>

	{#if providers.error}
		<div class="text-[var(--sb-error)] px-6 py-4 text-[13px]">Couldn't load providers: {providers.error}</div>
	{:else if providers.data && !provider}
		<div class="text-muted-foreground px-6 py-4 text-[13px]">No provider named <code>{name}</code> in {app.location?.name ?? 'this location'}.</div>
	{:else if provider}
		<div class="flex min-h-0 flex-1">
			<PageBody padding="0">
				<PageTabs
					bind:value={tab}
					tabs={[
						{ value: 'settings', label: 'Settings' },
						{ value: 'uses', label: 'Used by', count: usedCount(provider) }
					]}
				/>
				{#if tab === 'settings'}
					<div class="flex flex-col p-6">
						<div class="text-[var(--sb-text-faint)] mb-4 flex max-w-[760px] items-center gap-2 rounded-[2px] border border-[var(--border-subtle)] bg-[var(--sb-surface-low)] px-3 py-2 text-[11.5px]">
							<LockIcon size={12} />
							<span>Declared in <code>{provider.sourceFile}</code></span>
							<HelpTip text="Values come from each location's environment and never reach the server" />
						</div>
						{#if provider.settings.length}
							<DataTable class="settings static" embedded columns={settingColumns} rows={provider.settings} key={(f) => f.name} empty="This provider declares no settings.">
								{#snippet row(f)}
										<tr>
											<td>
												<div class="flex items-center gap-1.5"><code>{f.name}</code>{#if f.secret}<span class="secret" title="A secret type; its value is never shown"><LockIcon size={11} /> secret</span>{/if}</div>
												{#if f.description}<div class="cell-desc">{f.description}</div>{/if}
											</td>
											<td class="muted"><code>{f.type}</code></td>
											<td><code>{f.env}</code>{#if f.missing}<span class="missing" title="Unset in the location's environment; runs that use this provider fail">missing</span>{/if}</td>
											<td class="muted">{f.required ? 'Required' : 'Optional'}</td>
										</tr>
								{/snippet}
							</DataTable>
						{:else}
							<p class="text-muted-foreground text-[12.5px]">This provider declares no settings.</p>
						{/if}
					</div>
				{:else}
					<div class="p-6">
						<UsedByGraph {provider} catalogKind={(key) => kindByKey.get(key) ?? null} />
					</div>
				{/if}
			</PageBody>

			<aside class="w-[300px] shrink-0 overflow-auto border-l border-border p-5">
				<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Provider</div>
				<KeyValues
					rows={[
						{ label: 'Defined in', value: provider.sourceFile, mono: true }
					]}
				/>
				<p class="text-muted-foreground mt-5 whitespace-pre-line text-[12.5px] leading-relaxed">{provider.description}</p>
			</aside>
		</div>
	{:else}
		<div class="px-6 py-4"><Loading /></div>
	{/if}
</div>

<style>
	:global(table.settings) {
		max-width: 960px;
	}
	.secret {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		color: var(--muted-foreground);
		font-size: 11px;
	}
	.missing {
		margin-left: 6px;
		color: var(--sb-error);
		font-size: 11px;
	}
</style>
