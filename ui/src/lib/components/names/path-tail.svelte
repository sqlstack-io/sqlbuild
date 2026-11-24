<script lang="ts">
	// A file path on one line: when it doesn't fit, its start gives way to "…", so the file name stays;
	// the whole path is in the tooltip.
	let { path }: { path: string } = $props();
</script>

<span class="ptail" title={path}><bdi>{path}</bdi></span>

<style>
	.ptail {
		display: block;
		min-width: 0;
		overflow: hidden;
		direction: rtl;
		font-family: var(--font-mono);
		font-size: 11.5px;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.ptail bdi {
		direction: ltr;
		unicode-bidi: isolate;
	}
</style>
