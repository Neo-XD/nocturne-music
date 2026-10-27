<script lang="ts">
	import { page } from '$app/state';
</script>

<div style="padding: 2.5rem; color: #ff6b6b; font-family: monospace; background: #121214; min-height: 100vh; word-break: break-all;">
	<h1 style="font-size: 1.5rem; color: #fff; margin-bottom: 1rem;">Nocturne Error ({page.status})</h1>
	<p style="font-size: 1.1rem; color: #fca5a5; font-weight: bold;">{page.error?.message}</p>
	<pre style="margin-top: 1rem; color: #94a3b8; font-size: 0.85rem; overflow: auto; background: #1c1c22; padding: 1rem; border-radius: 8px;">{page.error?.stack || JSON.stringify(page.error, null, 2)}</pre>
</div>
