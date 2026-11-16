<script lang="ts">
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import { cn } from '$lib/utils';

	// The top bar's Refresh: an icon-only outline button with a muted icon that brightens on hover.
	// Pages update live, so it is a quiet secondary action.
	let { onclick }: { onclick: () => void } = $props();
</script>

<Tooltip.Provider delayDuration={150}>
	<Tooltip.Root>
		<Tooltip.Trigger class={cn(buttonVariants({ variant: 'outline', size: 'icon-bar' }), 'text-muted-foreground hover:text-foreground')} aria-label="Refresh now" {onclick}>
			<RefreshCwIcon />
		</Tooltip.Trigger>
		<Tooltip.Content side="bottom" sideOffset={4}>Refresh now</Tooltip.Content>
	</Tooltip.Root>
</Tooltip.Provider>
