<script lang="ts">
	import SunIcon from '@lucide/svelte/icons/sun';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import type { Snippet } from 'svelte';
	import type { ConnectionState } from '$lib/api';
	import { theme, toggleTheme } from '$lib/theme.svelte';
	import { formatAgo } from '$lib/format/time';
	import { targetWords } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import PlatformEventBanner from './shell/platform-event-banner.svelte';
	import WarehouseSyncStatus from './shell/warehouse-sync-status.svelte';

	type Props = { title: string; children?: Snippet };
	let { title, children }: Props = $props();

	const connectionTone: Record<ConnectionState, string> = {
		live: 'var(--sb-success)',
		connecting: 'var(--sb-text-faint)',
		reconnecting: 'var(--sb-warning)',
		offline: 'var(--sb-error)'
	};
	const connectionTitle: Record<ConnectionState, string> = {
		live: 'Pages update as things change.',
		connecting: 'Connecting for live updates…',
		reconnecting: 'Lost the live connection; trying again. Pages may be behind.',
		offline: 'Not receiving live updates. Refresh pages to see changes.'
	};
	const connectionLabel: Record<ConnectionState, string> = {
		live: 'Live',
		connecting: 'Connecting',
		reconnecting: 'Reconnecting',
		offline: 'Offline'
	};

	const snapshot = $derived(app.location?.snapshot);
</script>

<div class="app-topbar flex h-[52px] shrink-0 items-center gap-3 border-b border-border px-5">
	<h1 class="text-[15px] font-semibold">{title}</h1>
	<span class="text-muted-foreground text-[12.5px]">{app.location ? `${app.location.name} · ${targetWords(app.location.target)}` : '—'}</span>
	<div class="ml-auto flex items-center gap-2.5">
		<span
			class="text-muted-foreground flex items-center gap-[7px] text-[12px]"
			title={snapshot === undefined
				? connectionTitle[app.connection]
				: `${connectionTitle[app.connection]}\n${snapshot
					? `Version ${snapshot.generation}: commit ${snapshot.gitRevision}${snapshot.gitDirty ? ' (with uncommitted changes)' : ''} by ${snapshot.gitAuthor} (snapshot ${snapshot.id}, generation ${snapshot.generation})`
					: 'Nothing deployed to this location yet'}`}
		>
			<span class="h-[7px] w-[7px] rounded-full" style:background={connectionTone[app.connection]}></span>
			{connectionLabel[app.connection]}
			{#if snapshot !== undefined}
				<span class="text-[var(--sb-text-faint)]">·</span>
				{snapshot ? `Version ${snapshot.generation}, deployed ${formatAgo(snapshot.publishedAt, app.now)}` : 'nothing deployed yet'}
			{/if}
		</span>
		{#if app.client.supports('getWarehouseSync')}
			<span class="text-[var(--sb-text-faint)]">·</span>
			<WarehouseSyncStatus />
		{/if}
		{#if children}<span class="ml-1 flex items-center gap-2.5">{@render children()}</span>{/if}
		<button
			class="bg-card text-muted-foreground hover:text-foreground hover:bg-[var(--sb-hover)] grid h-8 w-8 place-items-center rounded-md border border-border"
			aria-label="Toggle theme"
			onclick={toggleTheme}
		>
			{#if theme.value === 'dark'}
				<SunIcon size={14} />
			{:else}
				<MoonIcon size={14} />
			{/if}
		</button>
	</div>
</div>
<PlatformEventBanner />

<style>
	.app-topbar {
		background: color-mix(in srgb, var(--background) 94%, transparent);
		backdrop-filter: blur(12px);
		/* The blur makes the bar its own layer; this keeps it and its popovers, such as Run now, above the page. */
		position: relative;
		z-index: 20;
	}
</style>
