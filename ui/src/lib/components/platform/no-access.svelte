<script lang="ts">
	import LockIcon from '@lucide/svelte/icons/lock';

	import type { Capability } from '$lib/api';
	import { PERMISSIONS } from '$lib/platform/permission-catalog';

	let { what, requires }: { what: string; requires: Capability | Capability[] } = $props();
	const list = $derived(Array.isArray(requires) ? requires : [requires]);
</script>

<div class="grid min-h-0 flex-1 place-items-center p-8">
	<div class="max-w-md text-center">
		<div class="text-muted-foreground mx-auto mb-3 grid h-9 w-9 place-items-center rounded-md border border-border">
			<LockIcon size={16} />
		</div>
		<h2 class="text-[14px] font-semibold">You don't have access to {what}</h2>
		<p class="text-muted-foreground mt-1.5 text-[13px]">
			Ask an administrator for a role that allows
			{#each list as c, i (c)}{#if i}&nbsp;or{' '}{/if}“{PERMISSIONS[c].label}” (<code>{c}</code>){/each}.
		</p>
	</div>
</div>
