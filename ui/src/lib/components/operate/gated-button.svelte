<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Capability } from '$lib/api';
	import Button, { type ButtonSize, type ButtonVariant } from '$lib/components/ui/button/button.svelte';
	import { app } from '$lib/state/app.svelte';

	// An action button that is disabled, with a "Requires …" hint, unless the
	// principal holds every capability in `requires`. The server still checks.
	let {
		requires,
		variant = 'outline',
		size = 'sm',
		disabled = false,
		reason,
		busy = false,
		onclick,
		children
	}: {
		requires: Capability | Capability[];
		variant?: ButtonVariant;
		size?: ButtonSize;
		/** Disabled for a reason other than permissions. */
		disabled?: boolean;
		reason?: string;
		busy?: boolean;
		onclick?: () => void;
		children: Snippet;
	} = $props();

	const needed = $derived((Array.isArray(requires) ? requires : [requires]).filter((c) => !app.can(c)));
	const title = $derived(needed.length ? `You don't have permission to do this (${needed.join(', ')})` : disabled ? reason : undefined);
</script>

<span class="gated" {title}>
	<Button {variant} {size} disabled={needed.length > 0 || disabled || busy} {onclick} aria-busy={busy}>
		{@render children()}
	</Button>
</span>

<style>
	.gated {
		display: inline-flex;
	}
</style>
