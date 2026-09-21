<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowTurnBackwardIcon } from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import * as Select from '$lib/components/ui/select';
	import * as api from '$lib/api';
	import { playback, prefs, toast } from '$lib/player.svelte';
	import DiscordCard from './DiscordCard.svelte';
	import type { DiscordConfig, PreviewTrack } from '$lib/discord';
	import { DISCORD_DEFAULTS, parseDiscordConfig } from '$lib/discord';

	let { settings }: { settings: Record<string, string> } = $props();

	// Preserved card layout: a single JSON blob rather than a settings row per field.
	let cfg = $state<DiscordConfig>(parseDiscordConfig(settings.discord_rpc_config));

	// Enabled flag matches titlebar toggle
	const enabled = $derived(prefs.visibleIcons.titlebar.discord);

	async function setEnabled(on: boolean) {
		prefs.visibleIcons.titlebar.discord = on;
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('icon_tb_discord', on ? 'true' : 'false');
		}
		settings.discord_rpc = on ? 'true' : 'false';
		await api.setSetting('discord_rpc', on ? 'true' : 'false');
	}

	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	function saveConfig() {
		const json = JSON.stringify(cfg);
		settings.discord_rpc_config = json;
		settings.discord_rpc_details = cfg.line1;
		settings.discord_rpc_state = cfg.line2;
		settings.discord_rpc_show_time = cfg.timestamps ? 'true' : 'false';
		settings.discord_rpc_show_pause = cfg.show_paused ? 'true' : 'false';
		settings.discord_rpc_show_button = cfg.button1 !== 'off' ? 'true' : 'false';
		settings.discord_rpc_button_label = cfg.custom_button_label ?? 'Listen on Nocturne';
		settings.discord_rpc_app_id = cfg.app_id ?? '';

		clearTimeout(saveTimer);
		saveTimer = setTimeout(async () => {
			try {
				await api.setSetting('discord_rpc_config', json);
				await api.setSetting('discord_rpc_details', cfg.line1);
				await api.setSetting('discord_rpc_state', cfg.line2);
				await api.setSetting('discord_rpc_show_time', cfg.timestamps ? 'true' : 'false');
				await api.setSetting('discord_rpc_show_pause', cfg.show_paused ? 'true' : 'false');
				await api.setSetting('discord_rpc_show_button', cfg.button1 !== 'off' ? 'true' : 'false');
				if (cfg.custom_button_label) {
					await api.setSetting('discord_rpc_button_label', cfg.custom_button_label);
				}
				if (cfg.app_id !== undefined) {
					await api.setSetting('discord_rpc_app_id', cfg.app_id);
				}
			} catch (e) {
				toast.error(String(e));
			}
		}, 250);
	}

	function set(patch: Partial<DiscordConfig>) {
		Object.assign(cfg, patch);
		saveConfig();
	}

	function insertToken(field: 'line1' | 'line2', token: string) {
		let cur = cfg[field] || '';
		if (cur === 'title') cur = '{title}';
		else if (cur === 'artist') cur = '{artist}';
		else if (cur === 'album') cur = '{album}';
		else if (cur === 'off') cur = '';
		const updated = cur.trim() ? `${cur} ${token}` : token;
		set({ [field]: updated });
	}

	function reset() {
		cfg = { ...DISCORD_DEFAULTS };
		saveConfig();
		toast.success('Discord RPC layout reset to defaults');
	}

	const isDefault = $derived(
		(Object.keys(DISCORD_DEFAULTS) as (keyof DiscordConfig)[]).every(
			(k) => cfg[k] === DISCORD_DEFAULTS[k]
		)
	);

	/** Stand-in track used when nothing is currently playing. */
	const SAMPLE_TRACK: PreviewTrack = {
		videoId: 'kJQP7kiw5Fk',
		title: 'Starboy',
		artists: 'The Weeknd, Daft Punk',
		artistId: 'UC0WP5P-ufpRfjbNrmOWwLBQ',
		album: 'Starboy',
		albumId: 'MPREb_32B8r7xU4w2',
		thumbnail: 'https://lh3.googleusercontent.com/4qD4y4aF'
	};

	const currentQueueItem = $derived(
		playback.queue.items[playback.queue.currentIndex]
	);

	const track = $derived<PreviewTrack>(
		playback.now
			? {
					videoId: playback.now.videoId,
					title: playback.now.title,
					artists: playback.now.artists,
					artistId: playback.now.artistId,
					album: currentQueueItem?.album,
					albumId: currentQueueItem?.album_id,
					thumbnail: playback.now.thumbnail,
					local: currentQueueItem?.is_upload
				}
			: SAMPLE_TRACK
	);

	const previewPaused = $derived(playback.now ? playback.paused : false);
	const previewPosition = $derived(playback.now ? playback.position : 84);
	const previewDuration = $derived(playback.now ? playback.duration : 228);

	const BUTTONS: Record<string, string> = {
		listen: 'Listen on YouTube Music',
		album: 'View album',
		artist: 'View artist',
		app: 'Get Nocturne',
		off: 'Off'
	};

	const BUTTON_OPTIONS = ['listen', 'album', 'artist', 'app', 'off'];
	const STATUS_OPTIONS = [
		{ id: 'line2', label: 'Bottom line / Artist (default)' },
		{ id: 'line1', label: 'Top line / Title' },
		{ id: 'app', label: 'App name ("Nocturne")' }
	];

	const statusLabel = $derived(
		STATUS_OPTIONS.find((o) => o.id === cfg.status_line)?.label ?? cfg.status_line
	);

	const GROUP = 'mb-6 last:mb-0';
	const LABEL =
		'mb-2 px-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground';
	const CARD =
		'divide-y divide-border/60 overflow-hidden rounded-xl border border-border/60 bg-card/80 dark:bg-card/65 backdrop-blur-md shadow-xs';
</script>

{#snippet row_(o: {
	title: string;
	desc?: string;
	control?: import('svelte').Snippet;
	below?: import('svelte').Snippet;
})}
	<div class="px-4 py-3">
		<div class="flex items-center justify-between gap-4">
			<span class="min-w-0 text-sm font-medium">{o.title}</span>
			{#if o.control}
				<div class="shrink-0">{@render o.control()}</div>
			{/if}
		</div>
		{#if o.desc}
			<p class="mt-1.5 text-xs leading-relaxed text-muted-foreground">{o.desc}</p>
		{/if}
		{#if o.below}
			<div class="mt-3">{@render o.below()}</div>
		{/if}
	</div>
{/snippet}

{#snippet picker(value: string, options: string[], labels: Record<string, string>, aria: string, onpick: (v: string) => void)}
	<Select.Root type="single" {value} onValueChange={onpick}>
		<Select.Trigger class="w-48 shrink-0" aria-label={aria}>
			<span class="flex-1 truncate text-left">{labels[value] ?? value}</span>
		</Select.Trigger>
		<Select.Content>
			{#each options as id (id)}
				<Select.Item value={id} label={labels[id]}>{labels[id]}</Select.Item>
			{/each}
		</Select.Content>
	</Select.Root>
{/snippet}

<div class="space-y-8">
	<!-- Top Box: RPC Preview & In-Place Editor -->
	<section class="rounded-2xl border border-border/60 bg-card/80 dark:bg-card/65 backdrop-blur-md p-5 sm:p-6 shadow-xs">
		<div class="mb-5 flex flex-wrap items-center justify-between gap-3 border-b border-border/40 pb-3.5">
			<div class="flex items-center gap-2.5">
				<h3 class="text-sm font-semibold tracking-wide text-foreground">
					Discord Activity Preview & Live Editor
				</h3>
				<span class="rounded-full bg-primary/15 px-2.5 py-0.5 text-[10.5px] font-semibold text-primary">
					{playback.now ? 'Live Playback' : 'Sample Preview'}
				</span>
			</div>
			<Button
				variant="ghost"
				size="sm"
				class="h-7 gap-1 px-2.5 text-xs cursor-pointer text-muted-foreground hover:text-foreground"
				disabled={isDefault}
				onclick={reset}
				title="Reset Discord layout to defaults"
			>
				<HugeiconsIcon icon={ArrowTurnBackwardIcon} class="h-3.5 w-3.5" />
				Reset Layout
			</Button>
		</div>

		<!-- Centered interactive Discord card -->
		<div class="flex justify-center">
			<DiscordCard
				{cfg}
				{track}
				{enabled}
				paused={previewPaused}
				position={previewPosition}
				duration={previewDuration}
				customButtonLabel={cfg.custom_button_label}
				editable={!cfg.hide_details && enabled}
				onSet={set}
				onInsertToken={insertToken}
			/>
		</div>

		<div class="mt-4 pt-3 border-t border-border/30 text-center text-xs text-muted-foreground">
			Edit the top and bottom lines directly inside the card above. Click token cards to insert dynamic tags.
		</div>
	</section>

	<!-- Bottom Section: Remaining Options (Organized into responsive columns) -->
	<div class="grid grid-cols-1 lg:grid-cols-2 gap-6 items-start">
		<!-- Left Column: Presence Behavior & Artwork -->
		<div class="space-y-6">
			<!-- Presence & Privacy -->
			<section class={GROUP}>
				<h3 class={LABEL}>Presence & Privacy</h3>
				<div class={CARD}>
					{@render row_({
						title: 'Enable Discord Rich Presence',
						desc: "Broadcast your currently playing music to your Discord profile card. Requires the official Discord desktop application running.",
						control: enableSwitch
					})}
					{@render row_({
						title: 'Show when paused',
						desc: 'Keep the rich presence card on your profile while music is paused. Removes progress scrubber to avoid false playhead timings.',
						control: pausedSwitch
					})}
					{@render row_({
						title: 'Hide song details (Incognito)',
						desc: 'Broadcasts only that you are listening to Nocturne without revealing track titles, artists, or album cover artwork.',
						control: hideDetailsSwitch
					})}
				</div>
			</section>

			<!-- Artwork & Timestamps -->
			<section class={GROUP}>
				<h3 class={LABEL}>Artwork & Timestamps</h3>
				<div class={CARD}>
					{@render row_({
						title: 'Album artwork',
						desc: 'Display high-resolution track cover artwork on your profile card.',
						control: coverSwitch
					})}
					{#if cfg.cover}
						{@render row_({
							title: 'Nocturne provider badge',
							desc: "Draw Nocturne's authentic application icon over the bottom-right corner of the artwork.",
							control: badgeSwitch
						})}
					{/if}
					{@render row_({
						title: 'Time progress bar',
						desc: 'Display elapsed and remaining song timestamps with a live updating timeline.',
						control: timestampsSwitch
					})}
				</div>
			</section>
		</div>

		<!-- Right Column: Interactive Links, Buttons & Identity -->
		<div class="space-y-6">
			<!-- Interactive links -->
			<section class={GROUP}>
				<h3 class={LABEL}>Interactive links</h3>
				<p class="mb-2 px-1 max-w-prose text-xs leading-relaxed text-muted-foreground">
					Make lines and album art clickable URLs in Discord (local files are never linked).
				</p>
				<div class={CARD}>
					{@render row_({ title: 'Make Top line clickable', control: link1Switch })}
					{@render row_({ title: 'Make Bottom line clickable', control: link2Switch })}
					{#if cfg.cover}
						{@render row_({ title: 'Make artwork clickable', control: linkCoverSwitch })}
					{/if}
				</div>
			</section>

			<!-- Action buttons -->
			<section class={GROUP}>
				<h3 class={LABEL}>Secondary Action Button</h3>
				<p class="mb-2 px-1 max-w-prose text-xs leading-relaxed text-muted-foreground">
					"Listen Together" is always primary. You can configure an optional secondary button.
				</p>
				<div class={CARD}>
					{@render row_({ title: 'Secondary button', control: button1Picker })}
					{#if cfg.button1 === 'listen'}
						{@render row_({
							title: 'Button label override',
							desc: 'Custom text label for the secondary action button.',
							control: customButtonInput
						})}
					{/if}
				</div>
			</section>

			<!-- Identity & Advanced -->
			<section class={GROUP}>
				<h3 class={LABEL}>Identity & Advanced</h3>
				<div class={CARD}>
					{@render row_({
						title: 'Status line text',
						desc: 'What Discord renders directly underneath your username in server member sidebars.',
						control: statusPicker
					})}
					{@render row_({
						title: 'Application name override',
						desc: 'Custom name displayed in the presence activity header (defaults to "Nocturne").',
						control: appNameInput
					})}
					{@render row_({
						title: 'Custom Application ID (optional)',
						desc: 'Supply your own registered Discord Developer Application ID snowflake (digits only). Leave blank for official Nocturne presence.',
						control: appIdInput
					})}
				</div>
			</section>
		</div>
	</div>
</div>

{#snippet enableSwitch()}<Switch checked={enabled} onCheckedChange={setEnabled} />{/snippet}
{#snippet pausedSwitch()}<Switch
		checked={cfg.show_paused}
		onCheckedChange={(v) => set({ show_paused: v })}
	/>{/snippet}
{#snippet hideDetailsSwitch()}<Switch
		checked={cfg.hide_details}
		onCheckedChange={(v) => set({ hide_details: v })}
	/>{/snippet}
{#snippet badgeSwitch()}<Switch checked={cfg.badge} onCheckedChange={(v) => set({ badge: v })} />{/snippet}
{#snippet coverSwitch()}<Switch checked={cfg.cover} onCheckedChange={(v) => set({ cover: v })} />{/snippet}
{#snippet timestampsSwitch()}<Switch
		checked={cfg.timestamps}
		onCheckedChange={(v) => set({ timestamps: v })}
	/>{/snippet}
{#snippet link1Switch()}<Switch
		checked={cfg.link_line1}
		onCheckedChange={(v) => set({ link_line1: v })}
	/>{/snippet}
{#snippet link2Switch()}<Switch
		checked={cfg.link_line2}
		onCheckedChange={(v) => set({ link_line2: v })}
	/>{/snippet}
{#snippet linkCoverSwitch()}<Switch
		checked={cfg.link_cover}
		onCheckedChange={(v) => set({ link_cover: v })}
	/>{/snippet}

{#snippet statusPicker()}
	<Select.Root
		type="single"
		value={cfg.status_line}
		onValueChange={(v) => set({ status_line: v })}
	>
		<Select.Trigger class="w-52 shrink-0" aria-label="Status line">
			<span class="flex-1 truncate text-left">{statusLabel}</span>
		</Select.Trigger>
		<Select.Content>
			{#each STATUS_OPTIONS as o (o.id)}
				<Select.Item value={o.id} label={o.label}>{o.label}</Select.Item>
			{/each}
		</Select.Content>
	</Select.Root>
{/snippet}

{#snippet appNameInput()}
	<Input
		class="w-52 h-8 text-xs"
		value={cfg.app_name}
		oninput={(e) => set({ app_name: e.currentTarget.value })}
		placeholder="Nocturne"
		aria-label="Application name"
		spellcheck={false}
	/>
{/snippet}

{#snippet customButtonInput()}
	<Input
		class="w-52 h-8 text-xs"
		value={cfg.custom_button_label ?? 'Listen on Nocturne'}
		oninput={(e) => set({ custom_button_label: e.currentTarget.value })}
		placeholder="Listen on Nocturne"
		aria-label="Custom button label"
	/>
{/snippet}

{#snippet appIdInput()}
	<Input
		class="w-52 h-8 text-xs font-mono"
		value={cfg.app_id ?? ''}
		oninput={(e) => set({ app_id: e.currentTarget.value })}
		placeholder="Default: Nocturne"
		aria-label="Custom Discord App ID"
	/>
{/snippet}

{#snippet button1Picker()}{@render picker(cfg.button1, BUTTON_OPTIONS, BUTTONS, 'Secondary Button', (v) => set({ button1: v }))}{/snippet}
