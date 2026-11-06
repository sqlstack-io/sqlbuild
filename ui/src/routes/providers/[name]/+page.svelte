<script lang="ts">
	import { page } from '$app/state';
	import LockIcon from '@lucide/svelte/icons/lock';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ConfigForm from '$lib/config/config-form.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { usedCount } from '../utils';

	const name = $derived(page.params.name ?? '');
	const providers = createResource((client, scope) => client.listProviders(scope));
	const provider = $derived(providers.data?.find((p) => p.name === name) ?? null);

	type Tab = 'settings' | 'uses';
	let tab = $state<Tab>('settings');
</script>

<AppTopbar title="Providers">
	<Button variant="outline" size="sm" onclick={() => providers.refresh()}>↻ Refresh</Button>
</AppTopbar>

<div class="flex min-h-0 flex-1 flex-col">
	<div class="flex items-center gap-3 border-b border-border px-6 py-3.5">
		<a href="/providers" class="text-muted-foreground hover:text-foreground text-[13px]">← Providers</a>
		<span class="font-display text-[17px] font-semibold">{name}</span>
		{#if provider}<span class="text-muted-foreground text-[12px]">{provider.type}</span>{/if}
	</div>

	{#if providers.error}
		<div class="text-[var(--sb-error)] px-6 py-4 text-[13px]">Couldn't load providers: {providers.error}</div>
	{:else if providers.data && !provider}
		<div class="text-muted-foreground px-6 py-4 text-[13px]">No provider named <code>{name}</code> in {app.location?.name ?? 'this location'}.</div>
	{:else if provider}
		<div class="flex min-h-0 flex-1">
			<div class="min-h-0 flex-1 overflow-auto">
				<PageTabs
					bind:value={tab}
					tabs={[
						{ value: 'settings', label: 'Settings' },
						{ value: 'uses', label: 'Used by', count: usedCount(provider) }
					]}
				/>
				{#if tab === 'settings'}
					<div class="flex max-w-[680px] flex-col p-6">
						<div class="text-[var(--sb-text-faint)] mb-4 flex items-center gap-2 rounded-[2px] border border-[var(--border-subtle)] bg-[var(--sb-surface-low)] px-3 py-2 text-[11.5px]">
							<LockIcon size={12} />
							<span>Read-only. Declared in <code>{provider.sourceFile}</code>; secrets show only where they come from.</span>
						</div>
						<ConfigForm schema={provider.settings} values={provider.values} readOnly />
					</div>
				{:else}
					<div class="flex flex-col gap-6 p-6">
						{#if !usedCount(provider)}
							<p class="text-muted-foreground text-[12.5px]">Nothing uses this provider yet.</p>
						{/if}
						{#if provider.usedBy.nodes.length}
							<div>
								<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Nodes · {provider.usedBy.nodes.length}</div>
								<div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-1.5">
									{#each provider.usedBy.nodes as m (m)}
										<a href="/catalog/{m}" class="bg-secondary hover:bg-[var(--sb-hover)] flex items-center gap-2 rounded-[2px] border border-border px-2.5 py-1.5 text-[12px]">{m}</a>
									{/each}
								</div>
							</div>
						{/if}
						{#if provider.usedBy.jobs.length}
							<div>
								<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Jobs · {provider.usedBy.jobs.length}</div>
								<div class="flex flex-wrap gap-1.5">
									{#each provider.usedBy.jobs as j (j)}
										<a href="/jobs/{j}" class="bg-secondary hover:bg-[var(--sb-hover)] rounded-[2px] border border-border px-2.5 py-1.5 text-[12px]">{j}</a>
									{/each}
								</div>
							</div>
						{/if}
						{#if provider.usedBy.triggers.length}
							<div>
								<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Sensors · {provider.usedBy.triggers.length}</div>
								<div class="flex flex-wrap gap-1.5">
									{#each provider.usedBy.triggers as t (t.id)}
										<a href="/triggers/{t.id}" class="bg-secondary hover:bg-[var(--sb-hover)] rounded-[2px] border border-border px-2.5 py-1.5 text-[12px]">{t.name}</a>
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			</div>

			<aside class="w-[300px] shrink-0 overflow-auto border-l border-border p-5">
				<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Provider</div>
				<KeyValues
					rows={[
						{ label: 'Type', value: provider.type },
						{ label: 'Defined in', value: provider.sourceFile, mono: true }
					]}
				/>
				<p class="text-muted-foreground mt-5 text-[12.5px] leading-relaxed">{provider.description}</p>
				<p class="text-muted-foreground mt-3 text-[12px] leading-relaxed">Change settings in the project code and deploy; the new values apply to runs that start after the deploy.</p>
			</aside>
		</div>
	{:else}
		<div class="text-muted-foreground px-6 py-4 text-[13px]">Loading…</div>
	{/if}
</div>
