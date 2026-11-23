<script lang="ts">
	import { page } from '$app/state';
	import Original from './variants/original.svelte';
	import FirstAttempt from './variants/first-attempt.svelte';
	import VariantA from './variants/variant-a.svelte';
	import VariantB from './variants/variant-b.svelte';
	import VariantC from './variants/variant-c.svelte';
	import VariantD from './variants/variant-d.svelte';

	// Layout mockups: `?variant=a|b|c|d` (or `first`) picks one; without it the page is unchanged.
	const variant = $derived(page.url.searchParams.get('variant'));
</script>

{#if variant === 'a'}
	<VariantA />
{:else if variant === 'b'}
	<VariantB />
{:else if variant === 'c'}
	<VariantC />
{:else if variant === 'd'}
	<VariantD />
{:else if variant === 'first'}
	<FirstAttempt />
{:else}
	<Original />
{/if}
