<script lang="ts">
	import { breakable } from '$lib/lineage/discovered-labels';

	// A long identifier that wraps only between its words, after `_ . [ / :`, never mid-word.
	let { text }: { text: string } = $props();
	const parts = $derived(breakable(text));
</script>

<span class="bname">{#each parts as part, i (i)}{#if i}<wbr />{/if}{part}{/each}</span>

<style>
	.bname {
		overflow-wrap: normal;
		word-break: normal;
	}
</style>
