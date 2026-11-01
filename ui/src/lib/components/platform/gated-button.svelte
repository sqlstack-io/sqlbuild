<script lang="ts">
	import type { Snippet } from 'svelte';
	import Button, { type ButtonSize, type ButtonVariant } from '$lib/components/ui/button/button.svelte';
	import type { Capability } from '$lib/api';
	import { requiresText } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';

	type Props = {
		/** Capability the action needs; the button is disabled with a hint without it. */
		requires: Capability;
		onclick: () => void;
		disabled?: boolean;
		busy?: boolean;
		title?: string;
		variant?: ButtonVariant;
		size?: ButtonSize;
		children: Snippet;
	};
	let { requires, onclick, disabled = false, busy = false, title, variant = 'outline', size = 'sm', children }: Props = $props();

	const allowed = $derived(app.can(requires));
</script>

<Button {variant} {size} disabled={!allowed || disabled || busy} title={allowed ? title : `You don't have permission to do this. ${requiresText(requires)}`} {onclick}>
	{@render children()}
</Button>
