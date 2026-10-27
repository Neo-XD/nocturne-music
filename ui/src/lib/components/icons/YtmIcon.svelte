<script lang="ts">
	let { class: cls = 'h-5 w-5' }: { class?: string } = $props();
</script>

<svg viewBox="0 0 24 24" fill="none" class={cls} aria-hidden="true">
	<circle cx="12" cy="12" r="11" fill="#FF0000" />
	<circle cx="12" cy="12" r="7.5" stroke="#FFFFFF" stroke-width="1.8" />
	<polygon points="10,8.5 16,12 10,15.5" fill="#FFFFFF" />
</svg>
