<script lang="ts">
	let { class: cls = 'h-5 w-5' }: { class?: string } = $props();
</script>

<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class={cls} aria-hidden="true">
	<path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" opacity="0.35"/>
	<path d="M9 18V5l12-2v13" stroke-width="2.2"/>
	<circle cx="6" cy="18" r="3" fill="currentColor"/>
	<circle cx="18" cy="16" r="3" fill="currentColor"/>
</svg>
