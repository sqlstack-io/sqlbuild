<script lang="ts">
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import OctagonAlertIcon from '@lucide/svelte/icons/octagon-alert';
	import type { PlatformEvent } from '$lib/api';
	import { formatAgo } from '$lib/format/time';
	import { eventKindLabel, problemHref } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';

	const rank = (e: PlatformEvent) => (e.severity === 'critical' ? 2 : e.severity === 'warning' ? 1 : 0);
	const events = $derived(
		app.platformEvents.filter((e) => e.active && e.severity !== 'info').sort((a, b) => rank(b) - rank(a))
	);
	const critical = $derived(events.some((e) => e.severity === 'critical'));
</script>

{#if events.length}
	<div class="banner" class:critical role="status" aria-live="polite">
		<span class="icon">
			{#if critical}<OctagonAlertIcon size={14} />{:else}<TriangleAlertIcon size={14} />{/if}
		</span>
		<ul>
			{#each events as event (event.id)}
				<li>
					<strong>{eventKindLabel[event.kind]}</strong>
					<span title={event.message}>{event.message}</span>
					<small>{formatAgo(event.occurredAt, app.now)}</small>
					<a href={problemHref(event)}>{event.locationId ? 'View location →' : 'View server status →'}</a>
				</li>
			{/each}
		</ul>
	</div>
{/if}

<style>
	.banner {
		--tone: var(--sb-warning);
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 7px 20px;
		border-bottom: 1px solid color-mix(in srgb, var(--tone) 35%, var(--border));
		background: color-mix(in srgb, var(--tone) 9%, var(--background));
		font-size: 12.5px;
	}
	.banner.critical {
		--tone: var(--sb-error);
	}
	.icon {
		display: grid;
		height: 20px;
		place-items: center;
		color: var(--tone);
	}
	ul {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-direction: column;
		gap: 2px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	li {
		display: flex;
		min-width: 0;
		align-items: baseline;
		gap: 8px;
		line-height: 20px;
	}
	li strong {
		font-weight: 600;
		white-space: nowrap;
	}
	li span {
		overflow: hidden;
		color: var(--muted-foreground);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	li small {
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
