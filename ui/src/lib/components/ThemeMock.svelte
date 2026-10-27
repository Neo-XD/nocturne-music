<script lang="ts">
	// A miniature of the app (sidebar, a shelf of cards, the player bar) in one palette.
	// It carries `theme-<id>` and `dark` itself, so every token re-resolves for this subtree alone,
	// and the overrides land as inline vars here the way `applyTheme` puts them on <html>.
	// That is what lets the Theme section follow live hover or slider drags without whole-document restyling.
	import { onAccent, type Custom, type ThemeId } from '$lib/theme.svelte';

	type Overrides = Pick<Custom, 'accent' | 'hue' | 'radius' | 'fontSans' | 'fontHeading'>;

	let {
		id,
		dark,
		overrides,
		detail
	}: {
		id: ThemeId;
		dark: boolean;
		overrides: Overrides;
		detail: { heading: string; title: string; artist: string; play: string; cover?: string };
	} = $props();

	const fg = $derived(overrides.accent ? onAccent(overrides.accent) : null);
	let coverFailed = $state(false);
	$effect(() => {
		void detail.cover;
		coverFailed = false;
	});
</script>

<div
	class="mock theme-{id} {dark ? 'dark' : ''}"
	style:--primary={overrides.accent}
	style:--primary-foreground={fg}
	style:--accent={overrides.accent}
	style:--accent-foreground={fg}
	style:--ring={overrides.accent}
	style:--sidebar-primary={overrides.accent}
	style:--sidebar-primary-foreground={fg}
	style:--sidebar-accent={overrides.accent ? `color-mix(in oklab, ${overrides.accent} 22%, var(--sidebar))` : null}
	style:--sidebar-accent-foreground={fg}
	style:--sidebar-ring={overrides.accent}
	style:--hue={overrides.hue === null ? null : String(overrides.hue)}
	style:--radius={overrides.radius === null ? '6px' : `${overrides.radius}px`}
	style:--font-sans={overrides.fontSans}
	style:--font-heading={overrides.fontHeading}
	aria-hidden="true"
>
	<div class="body">
		<div class="side">
			<i class="logo"></i>
			<i class="nav on"></i>
			<i class="nav"></i>
			<i class="nav short"></i>
			<i class="nav"></i>
		</div>
		<div class="main">
			<p class="heading">{detail.heading}</p>
			<span class="pill">{detail.play}</span>
			<div class="shelf">
				{#each [0, 1, 2] as n (n)}
					<div class="card">
						<i class="art"></i>
						<i class="line"></i>
						<i class="line faint"></i>
					</div>
				{/each}
			</div>
		</div>
	</div>
	<div class="bar">
		{#if detail.cover && !coverFailed}
			<img class="cover" src={detail.cover} alt="" onerror={() => (coverFailed = true)} />
		{:else}
			<i class="cover"></i>
		{/if}
		<div class="meta">
			<span class="title">{detail.title}</span>
			<span class="artist">{detail.artist}</span>
		</div>
		<div class="track"><i class="fill"></i></div>
		<i class="play"></i>
	</div>
</div>

<style>
	.mock {
		--k: 0.7;
		display: flex;
		flex-direction: column;
		aspect-ratio: 16 / 10;
		overflow: hidden;
		font-size: 10px;
		font-family: var(--font-sans);
		color: var(--foreground);
		background: var(--background);
		border: 1px solid var(--border);
		border-radius: calc(var(--radius) * 1.6);
		transition: background-color 0.15s ease, border-color 0.15s ease;
	}
	i {
		display: block;
	}
	.body {
		display: flex;
		flex: 1;
		min-height: 0;
	}
	.side {
		display: flex;
		width: 24%;
		flex-direction: column;
		gap: 0.7em;
		padding: 1em 0.9em;
		background: var(--sidebar);
		border-right: 1px solid var(--sidebar-border);
	}
	.logo {
		width: 1.4em;
		height: 1.4em;
		margin-bottom: 0.4em;
		border-radius: 999px;
		background: var(--primary);
	}
	.nav {
		height: 0.7em;
		border-radius: calc(var(--radius) * var(--k));
		background: var(--muted-foreground);
		opacity: 0.35;
	}
	.nav.on {
		height: 1.4em;
		opacity: 1;
		background: var(--sidebar-accent);
	}
	.nav.short {
		width: 65%;
	}
	.main {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-direction: column;
		gap: 0.9em;
		overflow: hidden;
		padding: 1.1em 1.2em;
	}
	.main > * {
		flex-shrink: 0;
	}
	.heading {
		overflow: hidden;
		font-family: var(--font-heading);
		font-size: 1.45em;
		font-weight: 600;
		line-height: 1.1;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	.shelf {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.8em;
	}
	.card {
		display: flex;
		flex-direction: column;
		gap: 0.5em;
		padding: 0.6em;
		border: 1px solid var(--border);
		border-radius: calc(var(--radius) * var(--k) * 1.4);
		background: var(--card);
	}
	.art {
		aspect-ratio: 1;
		border-radius: calc(var(--radius) * var(--k));
		background: var(--muted);
	}
	.line {
		height: 0.55em;
		border-radius: 999px;
		background: var(--muted-foreground);
		opacity: 0.45;
	}
	.line.faint {
		width: 60%;
		opacity: 0.25;
	}
	.pill {
		align-self: flex-start;
		padding: 0.35em 1em;
		border-radius: calc(var(--radius) * 2.6 * var(--k));
		background: var(--primary);
		color: var(--primary-foreground);
		font-size: 0.95em;
		font-weight: 600;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 1em;
		padding: 0.8em 1.2em;
		border-top: 1px solid var(--border);
		background: var(--card);
	}
	.cover {
		width: 2.6em;
		height: 2.6em;
		flex-shrink: 0;
		object-fit: cover;
		border-radius: calc(var(--radius) * var(--k));
		background: var(--muted);
	}
	.meta {
		display: flex;
		width: 26%;
		min-width: 0;
		flex-direction: column;
		gap: 0.35em;
		line-height: 1.15;
	}
	.title,
	.artist {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	.title {
		font-weight: 600;
	}
	.artist {
		font-size: 0.85em;
		color: var(--muted-foreground);
	}
	.track {
		flex: 1;
		height: 0.45em;
		overflow: hidden;
		border-radius: 999px;
		background: var(--muted);
	}
	.fill {
		width: 38%;
		height: 100%;
		background: var(--primary);
	}
	.play {
		width: 2.2em;
		height: 2.2em;
		flex-shrink: 0;
		border-radius: 999px;
		background: var(--primary);
	}
</style>
