<script lang="ts">
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import { page } from '$app/state';
	import LockIcon from '@lucide/svelte/icons/lock';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import KeyValues from '$lib/components/platform/key-values.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { consumerKindLabel, usedCount } from '../utils';

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
			<div class="min-h-0 flex-1 overflow-auto">
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
							<span>Values come from each location's environment and never reach the server. Declared in <code>{provider.sourceFile}</code>.</span>
						</div>
						{#if provider.settings.length}
							<table class="sb-list settings">
								<thead>
									<tr><th>Setting</th><th>Type</th><th>Environment variable</th><th>Required</th></tr>
								</thead>
								<tbody>
									{#each provider.settings as f (f.name)}
										<tr>
											<td>
												<div class="flex items-center gap-1.5"><code>{f.name}</code>{#if f.secret}<span class="secret" title="A secret type; its value is never shown"><LockIcon size={11} /> secret</span>{/if}</div>
												{#if f.description}<div class="cell-desc">{f.description}</div>{/if}
											</td>
											<td class="muted"><code>{f.type}</code></td>
											<td><code>{f.env}</code></td>
											<td class="muted">{f.required ? 'Required' : 'Optional'}</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{:else}
							<p class="text-muted-foreground text-[12.5px]">This provider declares no settings.</p>
						{/if}
					</div>
				{:else}
					<div class="flex flex-col gap-6 p-6">
						{#if !usedCount(provider)}
							<p class="text-muted-foreground text-[12.5px]">Nothing uses this provider yet.</p>
						{/if}
						{#if provider.usedBy.length}
							<div>
								<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Code · {provider.usedBy.length}</div>
								<div class="flex flex-wrap gap-1.5">
									{#each provider.usedBy as c (c.kind + c.name)}
										<span class="flex items-center gap-2 rounded-[2px] border border-border px-2.5 py-1.5 font-mono text-[12px]">{c.name}<span class="text-muted-foreground font-sans text-[11px]">{consumerKindLabel[c.kind]}</span></span>
									{/each}
								</div>
							</div>
						{/if}
						{#if provider.jobs.length}
							<div>
								<div class="text-[var(--sb-text-faint)] mb-2 text-[12px] font-medium">Jobs that run this code · {provider.jobs.length}</div>
								<div class="flex flex-wrap gap-1.5">
									{#each provider.jobs as j (j)}
										<a href={to(`/jobs/${j}`)} class="bg-secondary hover:bg-[var(--sb-hover)] rounded-[2px] border border-border px-2.5 py-1.5 text-[12px]">{j}</a>
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
	.settings {
		max-width: 960px;
	}
	.secret {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		color: var(--muted-foreground);
		font-size: 11px;
	}
</style>
