<script lang="ts">
	import PinIcon from '@lucide/svelte/icons/pin';
	import type { Trigger } from '$lib/api';
	import Button, { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import * as Popover from '$lib/components/ui/popover';
	import { cn } from '$lib/utils';
	import { setInUiExplanation } from './utils';

	// The pin after a trigger's status when it was set in the UI and so differs from the code. Hovering
	// (or focusing and pressing Enter) opens a popup that explains it and offers the reset; the popup
	// stays open while the pointer moves into it, and Escape closes it.
	let { trigger, canReset, onreset }: { trigger: Pick<Trigger, 'name' | 'state' | 'defaultState'>; canReset: boolean; onreset: () => void } = $props();
	let open = $state(false);
</script>

<Popover.Root bind:open>
	<Popover.Trigger
		openOnHover
		openDelay={150}
		closeDelay={200}
		class={cn(buttonVariants({ variant: 'ghost', size: 'icon-xs' }), 'pin-trigger')}
		aria-label="Set in UI: {trigger.name}"
		onclick={(e: MouseEvent) => e.stopPropagation()}
	>
		<PinIcon class="size-3.5" />
	</Popover.Trigger>
	<Popover.Content class="pin-pop" side="bottom" align="start" sideOffset={4}>
		<p><b>Set in UI.</b> {setInUiExplanation(trigger)}</p>
		<span title={canReset ? undefined : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)"}>
			<Button
				variant="link"
				size="inline"
				disabled={!canReset}
				onclick={() => {
					open = false;
					onreset();
				}}>Reset to the code's setting</Button
			>
		</span>
	</Popover.Content>
</Popover.Root>

<style>
	:global(.pin-trigger) {
		margin-left: 4px;
		color: var(--sb-text-faint);
		vertical-align: middle;
	}
	:global(.pin-trigger:hover),
	:global(.pin-trigger[data-state='open']) {
		color: var(--foreground);
	}
	:global(.pin-pop) {
		width: 260px !important;
		gap: 6px !important;
		padding: 8px 10px !important;
		border-radius: 2px !important;
		font-size: 12px !important;
		line-height: 1.45;
	}
	:global(.pin-pop p) {
		margin: 0;
	}
</style>
