<script lang="ts">
	import { to } from '$lib/paths';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import OctagonAlertIcon from '@lucide/svelte/icons/octagon-alert';
	import type { PlatformEvent } from '$lib/api';
	import { formatAgo } from '$lib/format/time';
	import { eventKindLabel, messageParts, problemHref } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { plural } from '$lib/format/plural';

	let { only = () => true }: { only?: (event: PlatformEvent) => boolean } = $props();

	const rank = (e: PlatformEvent) => (e.severity === 'critical' ? 2 : e.severity === 'warning' ? 1 : 0);
	// Only what concerns the whole server, such as a stopped scheduler: a location's problems are counted
	// beside its name in the top bar and listed in full on its page and the Overview.
	const events = $derived(
		app.platformEvents.filter((e) => e.active && e.severity !== 'info' && e.locationId === null && only(e)).sort((a, b) => rank(b) - rank(a))
	);
	const critical = $derived(events.some((e) => e.severity === 'critical'));
	const first = $derived(events[0] ?? null);
	const href = $derived(app.client.supports('platformHealth') ? problemHref({ locationId: null }) : '/locations');
</script>

{#if first}
	<div class="banner" class:critical role="status" aria-live="polite" data-testid="platform-banner">
		<span class="icon">
			{#if critical}<OctagonAlertIcon size={14} />{:else}<TriangleAlertIcon size={14} />{/if}
		</span>
		{#if events.length === 1}
			<strong>{eventKindLabel[first.kind]}</strong>
			<span>{messageParts(first.message).summary}</span>
			<small>{formatAgo(first.occurredAt, app.now)}</small>
		{:else}
			<strong>{plural(events.length, 'server problem')}</strong>
			<span>{events.map((e) => eventKindLabel[e.kind]).join(', ')}</span>
		{/if}
		<a href={to(href)}>View server status →</a>
	</div>
{/if}

<style>
	.banner {
		--tone: var(--sb-warning);
		display: flex;
		align-items: baseline;
		gap: 8px;
		padding: 5px 20px;
		border-bottom: 1px solid color-mix(in srgb, var(--tone) 35%, var(--border));
		background: color-mix(in srgb, var(--tone) 9%, var(--background));
		font-size: 12.5px;
	}
	.banner.critical {
		--tone: var(--sb-error);
	}
	.icon {
		display: grid;
		align-self: center;
		height: 20px;
		place-items: center;
		color: var(--tone);
	}
	strong {
		font-weight: 600;
		white-space: nowrap;
	}
	span {
		min-width: 0;
		overflow: hidden;
		color: var(--muted-foreground);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	small {
		color: var(--sb-text-faint);
		font-size: 11.5px;
		white-space: nowrap;
	}
	a {
		margin-left: auto;
		color: var(--primary);
		font-weight: 500;
		line-height: 20px;
		white-space: nowrap;
	}
</style>
