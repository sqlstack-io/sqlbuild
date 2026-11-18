<script lang="ts">
	import { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import { cn } from '$lib/utils';
	import Button from '$lib/components/ui/button/button.svelte';
	import * as Popover from '$lib/components/ui/popover';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { NODE_FIELDS, nodeFields } from '$lib/lineage/node-fields.svelte';
	import SlidersIcon from '@lucide/svelte/icons/sliders-horizontal';

	let open = $state(false);
	const count = $derived(Object.values(nodeFields.value).filter(Boolean).length);
</script>

<Popover.Root bind:open>
	<Popover.Trigger class={cn(buttonVariants({ variant: 'ghost', size: 'bar' }))}>
		<SlidersIcon />Node fields
		<span class="text-[var(--sb-text-faint)] text-[11.5px] font-normal">{count}</span>
	</Popover.Trigger>
	<Popover.Content class="w-[220px] p-0" align="start">
		<div class="border-b border-border px-3 py-2.5 text-[12px] font-semibold">
			Show on nodes
		</div>
		<div class="flex flex-col p-1.5">
			{#each NODE_FIELDS as field (field.key)}
				<!-- raw-button: a checkable item in the popover's menu -->
				<button
					class="hover:bg-[var(--sb-hover)] flex items-center gap-2.5 rounded-[2px] px-2 py-1.5 text-left text-[12.5px]"
					onclick={() => nodeFields.toggle(field.key)}
				>
					<Checkbox checked={nodeFields.value[field.key]} class="pointer-events-none size-3.5" />
					{field.label}
				</button>
			{/each}
		</div>
		<div class="flex items-center justify-between border-t border-border px-3 py-2">
			<Button variant="ghost" size="sm" class="-ml-2" onclick={() => nodeFields.reset()}>Reset to default</Button>
		</div>
	</Popover.Content>
</Popover.Root>
