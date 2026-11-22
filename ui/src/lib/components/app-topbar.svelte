<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import SunIcon from '@lucide/svelte/icons/sun';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import type { Snippet } from 'svelte';
	import type { ConnectionState, PlatformEvent } from '$lib/api';
	import { theme, toggleTheme } from '$lib/theme.svelte';
	import { formatAgo } from '$lib/format/time';
	import { pageTitle } from '$lib/format/page-title';
	import { app } from '$lib/state/app.svelte';
	import PlatformEventBanner from './shell/platform-event-banner.svelte';
	import WarehouseSyncStatus from './shell/warehouse-sync-status.svelte';

	/**
	 * `events` picks the platform problems the banner shows on this page; by default, all of them.
	 * `subject` names what a detail page shows in the browser tab, such as `Run 1a2b3c4d · orders_job`.
	 */
	type Props = { title: string; subject?: string | null; children?: Snippet; events?: (event: PlatformEvent) => boolean };
	let { title, subject, children, events }: Props = $props();

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

	const snapshot = $derived(app.snapshot);
	const where = $derived(app.location ? `${app.location.name} · ${app.location.target.name}` : '—');
</script>

<svelte:head><title>{pageTitle(title, subject)}</title></svelte:head>

<div class="app-topbar flex h-[52px] shrink-0 items-center gap-3 border-b border-border px-5">
	<h1 class="text-[15px] font-semibold">{title}</h1>
	<span class="text-muted-foreground min-w-0 truncate text-[12.5px]" title={where}>{where}</span>
	<div class="ml-auto flex items-center gap-2.5">
		<span
			class="text-muted-foreground flex items-center gap-[7px] text-[12px]"
			title={snapshot === undefined
				? connectionTitle[app.connection]
				: `${connectionTitle[app.connection]}\n${snapshot
					? `Version ${snapshot.generation}${snapshot.gitRevision ? `: commit ${snapshot.gitRevision}${snapshot.gitDirty ? ' (with uncommitted changes)' : ''}${snapshot.gitAuthor ? ` by ${snapshot.gitAuthor}` : ''}` : ', not from a git commit'} (snapshot ${snapshot.id}, generation ${snapshot.generation})`
					: 'Nothing deployed to this location yet'}`}
		>
			<!-- A fixed slot, so going from Connecting to Live doesn't shift the bar. -->
			<span class="connection"><span class="h-[7px] w-[7px] rounded-full" style:background={connectionTone[app.connection]}></span>{connectionLabel[app.connection]}</span>
			{#if snapshot !== undefined}
				<span class="text-[var(--sb-text-faint)]">·</span>
				<span class="whitespace-nowrap">{snapshot ? `v${snapshot.generation} · ${formatAgo(snapshot.publishedAt, app.now)}` : 'Not deployed'}</span>
			{/if}
		</span>
		{#if app.client.supports('getWarehouseSync')}
			<span class="text-[var(--sb-text-faint)]">·</span>
			<WarehouseSyncStatus />
		{/if}
		{#if children}<span class="ml-1 flex items-center gap-2.5">{@render children()}</span>{/if}
		<Button size="icon-bar" class="text-muted-foreground hover:text-foreground" aria-label="Toggle theme" title="Toggle theme" onclick={toggleTheme}>
			{#if theme.value === 'dark'}<SunIcon />{:else}<MoonIcon />{/if}
		</Button>
	</div>
</div>
<PlatformEventBanner only={events} />

<style>
	.connection {
		display: inline-flex;
		min-width: 6.8em;
		align-items: center;
		justify-content: flex-end;
		gap: 7px;
	}
	.app-topbar {
		background: color-mix(in srgb, var(--background) 94%, transparent);
		backdrop-filter: blur(12px);
		/* The blur makes the bar its own layer; this keeps it and its popovers, such as Run now, above the page. */
		position: relative;
		z-index: 20;
	}
</style>
