<script lang="ts">
	import { to } from '$lib/paths';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import OctagonAlertIcon from '@lucide/svelte/icons/octagon-alert';
	import type { PlatformEvent } from '$lib/api';
	import { eventKindLabel, messageParts, problemHref, problemPhrase, stopsRuns } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { plural } from '$lib/format/plural';

	let { only = () => true }: { only?: (event: PlatformEvent) => boolean } = $props();

	// Every open problem, the location's and the server's, on one line: the worst leads with its
	// location, the rest is a count. Red when one stops runs, amber otherwise; never the error itself.
	const rank = (e: PlatformEvent) => (stopsRuns(e) ? 3 : e.severity === 'critical' ? 2 : e.severity === 'warning' ? 1 : 0);
	const events = $derived(
		app.platformEvents
			.filter((e) => e.active && e.severity !== 'info' && only(e))
			.sort((a, b) => rank(b) - rank(a) || a.occurredAt.localeCompare(b.occurredAt))
	);
	const red = $derived(events.some((e) => stopsRuns(e) || (e.locationId === null && e.severity === 'critical')));
	const places = $derived(new Set(events.flatMap((e) => (e.locationId ? [e.locationId] : []))));
	const lead = $derived(events[0] ?? null);
	const nameOf = (e: PlatformEvent) => app.bootstrap?.locations.find((l) => l.id === e.locationId)?.name ?? e.subject ?? 'A location';
	const line = $derived.by(() => {
		if (!lead) return null;
		const server = events.filter((e) => e.locationId === null).length;
		if (places.size > 1) {
			return { text: `${places.size} locations have problems${server ? ` · ${plural(server, 'server problem')}` : ''}`, href: '/locations' };
		}
		const what = lead.locationId ? `${nameOf(lead)}${problemPhrase(lead)}` : eventKindLabel[lead.kind];
		const rest = events.length - 1;
		const href = lead.locationId && !app.client.supports('getLocation') ? '/locations' : problemHref(lead);
		return { text: `${what}${rest ? ` · ${plural(rest, 'more problem')}` : ''}`, href };
	});
</script>

{#if line}
	<div class="banner" class:critical={red} role="status" aria-live="polite" data-testid="platform-banner">
		<span class="icon">
			{#if red}<OctagonAlertIcon size={14} />{:else}<TriangleAlertIcon size={14} />{/if}
		</span>
		<strong title={lead ? messageParts(lead.message).summary : undefined}>{line.text}</strong>
		<span class="sep">·</span>
		<a href={to(line.href)}>View</a>
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
		min-width: 0;
		overflow: hidden;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.sep {
		color: var(--sb-text-faint);
	}
	a {
		color: var(--primary);
		font-weight: 500;
		line-height: 20px;
		white-space: nowrap;
	}
</style>
