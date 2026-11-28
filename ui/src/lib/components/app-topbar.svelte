<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import SunIcon from '@lucide/svelte/icons/sun';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import type { Snippet } from 'svelte';
	import type { ConnectionState, PlatformEvent } from '$lib/api';
	import { theme, toggleTheme } from '$lib/theme.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { pageTitle } from '$lib/format/page-title';
	import { app } from '$lib/state/app.svelte';
	import PlatformEventBanner from './shell/platform-event-banner.svelte';
	import HealthBanner from './shell/health-banner.svelte';
	import { health } from '$lib/state/health.svelte';
	import { to } from '$lib/paths';
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
		offline: 'Not receiving live updates. Reload to read everything again.'
	};
	const connectionLabel: Record<ConnectionState, string> = {
		live: 'Live',
		connecting: 'Connecting',
		reconnecting: 'Reconnecting',
		offline: 'Offline'
	};

	/** The server's own health decides the label once read: Live, Degraded or Unavailable; Live still says whether updates stream. */
	const healthTone: Record<'degraded' | 'unavailable', string> = { degraded: 'var(--sb-warning)', unavailable: 'var(--sb-error)' };
	const healthLabel: Record<'degraded' | 'unavailable', string> = { degraded: 'Degraded', unavailable: 'Unavailable' };
	const serverState = $derived(health.status === 'degraded' || health.status === 'unavailable' ? health.status : null);
	const indicatorTone = $derived(serverState ? healthTone[serverState] : connectionTone[app.connection]);
	const indicatorLabel = $derived(serverState ? healthLabel[serverState] : connectionLabel[app.connection]);
	const indicatorTitle = $derived(
		serverState === 'unavailable'
			? (health.outage?.reason ?? 'The server is unavailable.')
			: serverState === 'degraded'
				? health.degraded.map((part) => `${part.name}: ${part.reason ?? 'degraded'}`).join('\n')
				: connectionTitle[app.connection]
	);

	const snapshot = $derived(app.snapshot);
	/** Live updates dropped: the page may be behind, so it says since when and offers to catch up. */
	const dropped = $derived(!serverState && (app.connection === 'reconnecting' || app.connection === 'offline'));
	const where = $derived(app.location ? `${app.location.name} · ${app.location.target.name}` : '—');
</script>

<svelte:head><title>{pageTitle(title, subject)}</title></svelte:head>

<div class="app-topbar flex h-[52px] shrink-0 items-center gap-3 border-b border-border px-5">
	<h1 class="text-[15px] font-semibold">{title}</h1>
	<span class="text-muted-foreground min-w-0 truncate text-[12.5px]" title={where}>{where}</span>
	{#if app.client.supports('getWarehouseSync')}
		<span class="text-[var(--sb-text-faint)]">·</span>
		<WarehouseSyncStatus />
	{/if}
	<div class="ml-auto flex items-center gap-2.5">
		<span
			class="text-muted-foreground flex items-center gap-[7px] text-[12px]"
			data-testid="server-indicator"
			title={snapshot === undefined
				? indicatorTitle
				: `${indicatorTitle}\n${snapshot
					? `Version ${snapshot.generation}${snapshot.gitRevision ? `: commit ${snapshot.gitRevision}${snapshot.gitDirty ? ' (with uncommitted changes)' : ''}${snapshot.gitAuthor ? ` by ${snapshot.gitAuthor}` : ''}` : ', not from a git commit'} (snapshot ${snapshot.id}, generation ${snapshot.generation})`
					: 'Nothing deployed to this location yet'}`}
		>
			<!-- A fixed slot, so going from Connecting to Live doesn't shift the bar. -->
			<a class="connection" href={to('/status')}><span class="h-[7px] w-[7px] rounded-full" style:background={indicatorTone}></span>{indicatorLabel}</a>
			{#if snapshot !== undefined}
				<span class="text-[var(--sb-text-faint)]">·</span>
				<span class="whitespace-nowrap">{snapshot ? `v${snapshot.generation} · ${formatAgo(snapshot.publishedAt, app.now)}` : 'Not deployed'}</span>
			{/if}
		</span>
		{#if dropped}
			<span class="text-muted-foreground whitespace-nowrap text-[12px]" title="Live updates stopped {formatStamp(new Date(app.updatedAt).toISOString())}; this page may be behind since.">updated {formatAgo(new Date(app.updatedAt).toISOString(), app.now)}</span>
			{#if app.connection === 'reconnecting'}
				<Button variant="link" size="inline" class="text-[12px]" data-testid="reconnect" title="Connect for live updates now and read this page again" onclick={() => app.reconnect()}>Reconnect</Button>
			{:else}
				<Button variant="link" size="inline" class="text-[12px]" data-testid="reload" title="Load the page again, reading everything afresh" onclick={() => window.location.reload()}>Reload</Button>
			{/if}
		{/if}
		{#if children}<span class="ml-1 flex items-center gap-2.5">{@render children()}</span>{/if}
		<Button size="icon-bar" class="text-muted-foreground hover:text-foreground" aria-label="Toggle theme" title="Toggle theme" onclick={toggleTheme}>
			{#if theme.value === 'dark'}<SunIcon />{:else}<MoonIcon />{/if}
		</Button>
	</div>
</div>
<HealthBanner />
<PlatformEventBanner only={events} />

<style>
	.connection {
		color: inherit;
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
