<script lang="ts">
	import ConstructionIcon from '@lucide/svelte/icons/construction';
	import LockIcon from '@lucide/svelte/icons/lock';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import type { Capability } from '$lib/api';
	import { hasAnyCapability } from '$lib/api';
	import { app } from '$lib/state/app.svelte';

	type Props = {
		title: string;
		/** Navigation area the page belongs to, e.g. `Platform › Locations`. */
		area: string;
		/** What the page will show once built. */
		description: string;
		/** Capabilities that grant access; any one is enough. */
		requires?: readonly Capability[];
	};
	let { title, area, description, requires }: Props = $props();

	const allowed = $derived(!requires || hasAnyCapability(app.capabilities, requires));
</script>

<AppTopbar {title} />

<div class="grid min-h-0 flex-1 place-items-center p-8">
	<div class="max-w-md text-center">
		<div class="text-muted-foreground mx-auto mb-3 grid h-9 w-9 place-items-center rounded-md border border-border">
			{#if allowed}<ConstructionIcon size={16} />{:else}<LockIcon size={16} />{/if}
		</div>
		{#if allowed}
			<h2 class="text-[14px] font-semibold">Not built yet</h2>
			<p class="text-muted-foreground mt-1.5 text-[13px]">Tracked in {area}. {description}</p>
		{:else}
			<h2 class="text-[14px] font-semibold">You don't have access to {title}</h2>
			<p class="text-muted-foreground mt-1.5 text-[13px]">Ask an administrator for a role that grants it.</p>
		{/if}
	</div>
</div>
