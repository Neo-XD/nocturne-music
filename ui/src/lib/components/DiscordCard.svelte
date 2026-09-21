<script lang="ts">
	// The colors are Discord's, hardcoded rather than taken from the theme tokens. This is a picture
	// of another application, so following Nocturne's theme would make it less accurate, not more.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		MoreHorizontalIcon,
		MusicNote01Icon,
		UserCircleIcon,
		Album02Icon,
		Exchange01Icon
	} from '@hugeicons/core-free-icons';
	import { thumb } from '$lib/thumb';
	import nocturneLogo from '$lib/assets/icon.png';
	import type { DiscordConfig, PreviewTrack } from '$lib/discord';
	import { cardText, cardLink, BUTTON_LABELS } from '$lib/discord';
	import * as api from '$lib/api';

	let {
		cfg,
		track,
		enabled,
		paused = false,
		position = 0,
		duration = 0,
		customButtonLabel,
		editable = false,
		onSet,
		onInsertToken
	}: {
		cfg: DiscordConfig;
		track: PreviewTrack;
		enabled: boolean;
		paused?: boolean;
		position?: number;
		duration?: number;
		customButtonLabel?: string;
		editable?: boolean;
		onSet?: (patch: Partial<DiscordConfig>) => void;
		onInsertToken?: (field: 'line1' | 'line2', token: string) => void;
	} = $props();

	/** Discord's own wording */
	const LISTENING_TO = 'Listening to';

	const appName = $derived(cfg.app_name.trim() || 'Nocturne');
	// The profile card's header is *always* the application name. `status_display_type` only moves
	// the one-line status Discord writes under your name in the member list, which is why that gets
	// its own mock below rather than being folded into this header.
	const line1 = $derived(cardText(cfg.line1, track) ?? track.title);
	const line2 = $derived(cardText(cfg.line2, track));
	// Under "hide details" there is nothing left but the app name, in both places.
	const statusText = $derived(
		cfg.hide_details
			? appName
			: cfg.status_line === 'line1'
				? line1
				: cfg.status_line === 'line2'
					? (line2 ?? appName)
					: appName
	);
	// Is this card actually on the profile right now? When it is not, the preview still draws it —
	// you are editing a layout, and taking it away mid-edit loses the thing you are working on — but
	// dimmed, with a line underneath saying why.
	const offAir = $derived(!enabled || (paused && !cfg.show_paused));
	// Discord has no paused state, so the backend drops the timeline rather than leaving a bar that
	// keeps advancing. That only applies to a card that is up: an off-air one is showing what it
	// *would* look like, and it would look like playback.
	const showBar = $derived(cfg.timestamps && !(paused && cfg.show_paused));
	// `large_text` is a third line on the card, not only the artwork's tooltip.
	const line3 = $derived(cfg.cover ? cardText(cfg.line3, track) : null);
	// Line 1 falls back to the title when its slot is empty, so the link follows the fallback
	const line1Slot = $derived(cardText(cfg.line1, track) === null ? 'title' : cfg.line1);
	const link1 = $derived(cfg.link_line1 && !!cardLink(line1Slot, track));
	const link2 = $derived(cfg.link_line2 && !!cardLink(cfg.line2, track));
	const linkCover = $derived(
		cfg.link_cover && !!(cardLink('album', track) ?? cardLink('title', track))
	);

	const art = $derived(thumb(track.thumbnail, 128));
	const badge = $derived(cfg.cover && cfg.badge);
	const pct = $derived(duration > 0 ? Math.min(100, (position / duration) * 100) : 0);

	function clock(secs: number) {
		const s = Math.max(0, Math.floor(secs));
		return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
	}

	async function handleListenTogetherClick() {
		if (typeof window !== 'undefined') {
			window.dispatchEvent(
				new CustomEvent('nocturne:incoming-lt-request', {
					detail: { username: 'Discord Friend', userId: `user_${Date.now()}` }
				})
			);
		}
		try {
			await api.triggerLtJoinRequest('Discord Friend');
		} catch {
			// Window event already dispatched above
		}
	}
</script>

<div class="flex flex-col items-center w-full">
	<div class={offAir ? 'opacity-50 transition-opacity w-full flex flex-col items-center' : 'transition-opacity w-full flex flex-col items-center'}>
		<!-- Discord Card: comfortable width for inline input editing -->
		<div
			class="w-full max-w-[420px] rounded-xl bg-[#232428] p-4 text-left shadow-2xl ring-1 ring-black/40 select-none"
		>
			<div class="mb-3 flex items-center justify-between">
				<span class="truncate text-[12px] font-bold uppercase tracking-wider text-[#b5bac1]">
					{LISTENING_TO} {appName}
				</span>
				<HugeiconsIcon icon={MoreHorizontalIcon} size={16} class="shrink-0 text-[#b5bac1]" />
			</div>

			{#if !cfg.hide_details}
				<div class="flex gap-3.5">
					{#if cfg.cover}
						<div
							class="relative size-[68px] shrink-0 rounded-lg bg-[#1e1f22] overflow-visible {linkCover
								? 'cursor-pointer ring-1 ring-[#b5bac1]/50'
								: ''}"
							title={line3 ?? undefined}
						>
							{#if art}
								<img src={art} alt="" class="size-full rounded-lg object-cover shadow-sm" draggable="false" />
							{:else}
								<div
									class="flex size-full items-center justify-center rounded-lg bg-gradient-to-br from-[#5865f2]/50 to-[#eb459e]/40"
								>
									<HugeiconsIcon icon={MusicNote01Icon} size={24} class="text-white/70" />
								</div>
							{/if}
							{#if badge}
								<!-- Discord draws small_image as a circle clipped to the artwork's bottom-right. -->
								<img
									src={nocturneLogo}
									alt="Nocturne"
									class="absolute -right-1.5 -bottom-1.5 size-[24px] rounded-full ring-2 ring-[#232428] shadow-md"
									draggable="false"
								/>
							{/if}
						</div>
					{/if}

					<div class="flex min-w-0 flex-1 flex-col gap-2.5">
						<!-- Top Line (Details) -->
						<div class="space-y-1">
							<div class="flex items-center justify-between gap-1">
								<span class="text-[10px] font-bold uppercase tracking-wider text-[#949ba4]">
									Top Line (Details)
								</span>
								{#if editable}
									<span class="truncate max-w-[140px] text-[10px] text-[#b5bac1]" title={line1}>
										Preview: <strong class="text-white font-medium">{line1}</strong>
									</span>
								{/if}
							</div>
							{#if editable}
								<input
									type="text"
									value={cfg.line1}
									oninput={(e) => onSet?.({ line1: e.currentTarget.value })}
									placeholder={'e.g. {title}'}
									class="w-full rounded bg-[#1e1f22] px-2 py-1 text-[12.5px] font-semibold text-white placeholder-white/30 border border-white/10 focus:border-[#5865f2] focus:outline-none focus:ring-1 focus:ring-[#5865f2] transition-colors"
								/>
								<!-- Small Insert Cards -->
								<div class="flex flex-wrap items-center gap-1.5 pt-0.5">
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10.5px] font-medium text-[#dbdee1] hover:text-white transition-all cursor-pointer shadow-2xs"
										onclick={() => onInsertToken?.('line1', '{title}')}
										title="Insert track title"
									>
										<HugeiconsIcon icon={MusicNote01Icon} size={11} class="text-[#5865f2]" />
										<span>Song Title</span>
									</button>
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10.5px] font-medium text-[#dbdee1] hover:text-white transition-all cursor-pointer shadow-2xs"
										onclick={() => onInsertToken?.('line1', '{artist}')}
										title="Insert artist name"
									>
										<HugeiconsIcon icon={UserCircleIcon} size={11} class="text-[#5865f2]" />
										<span>Artist Name</span>
									</button>
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10.5px] font-medium text-[#dbdee1] hover:text-white transition-all cursor-pointer shadow-2xs"
										onclick={() => onInsertToken?.('line1', '{album}')}
										title="Insert album name"
									>
										<HugeiconsIcon icon={Album02Icon} size={11} class="text-[#5865f2]" />
										<span>Album Name</span>
									</button>
								</div>
							{:else}
								<div
									class="truncate text-[14px] leading-[18px] font-semibold text-[#f2f3f5] {link1
										? 'cursor-pointer underline decoration-dotted underline-offset-[3px] hover:decoration-solid'
										: ''}"
								>
									{line1}
								</div>
							{/if}
						</div>

						<!-- Bottom Line (State) -->
						<div class="space-y-1">
							<div class="flex items-center justify-between gap-1">
								<span class="text-[10px] font-bold uppercase tracking-wider text-[#949ba4]">
									Bottom Line (State)
								</span>
								{#if editable}
									<span class="truncate max-w-[140px] text-[10px] text-[#b5bac1]" title={line2 ?? '(empty)'}>
										Preview: <strong class="text-white font-medium">{line2 ?? '(empty)'}</strong>
									</span>
								{/if}
							</div>
							{#if editable}
								<input
									type="text"
									value={cfg.line2}
									oninput={(e) => onSet?.({ line2: e.currentTarget.value })}
									placeholder={'e.g. {artist}'}
									class="w-full rounded bg-[#1e1f22] px-2 py-1 text-[12px] text-[#dbdee1] placeholder-white/30 border border-white/10 focus:border-[#5865f2] focus:outline-none focus:ring-1 focus:ring-[#5865f2] transition-colors"
								/>
								<!-- Small Insert Cards -->
								<div class="flex flex-wrap items-center gap-1.5 pt-0.5">
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10.5px] font-medium text-[#dbdee1] hover:text-white transition-all cursor-pointer shadow-2xs"
										onclick={() => onInsertToken?.('line2', '{artist}')}
										title="Insert artist name"
									>
										<HugeiconsIcon icon={UserCircleIcon} size={11} class="text-[#5865f2]" />
										<span>Artist Name</span>
									</button>
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10.5px] font-medium text-[#dbdee1] hover:text-white transition-all cursor-pointer shadow-2xs"
										onclick={() => onInsertToken?.('line2', '{album}')}
										title="Insert album name"
									>
										<HugeiconsIcon icon={Album02Icon} size={11} class="text-[#5865f2]" />
										<span>Album Name</span>
									</button>
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10.5px] font-medium text-[#dbdee1] hover:text-white transition-all cursor-pointer shadow-2xs"
										onclick={() => onInsertToken?.('line2', '{title}')}
										title="Insert track title"
									>
										<HugeiconsIcon icon={MusicNote01Icon} size={11} class="text-[#5865f2]" />
										<span>Song Title</span>
									</button>
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10px] font-medium text-[#949ba4] hover:text-white transition-all cursor-pointer"
										onclick={() => onSet?.({ line2: '{artist} — {album}' })}
										title="Set to Artist — Album"
									>
										<span>Artist — Album</span>
									</button>
									<button
										type="button"
										class="inline-flex items-center gap-1 rounded bg-[#2b2d31] hover:bg-[#35373c] border border-white/10 hover:border-white/25 px-1.5 py-0.5 text-[10px] font-medium text-[#949ba4] hover:text-white transition-all cursor-pointer"
										onclick={() => onSet?.({ line2: 'off' })}
										title="Turn bottom line off"
									>
										<span>Off</span>
									</button>
								</div>
							{:else if line2}
								<div
									class="truncate text-[13px] leading-[17px] text-[#dbdee1] {link2
										? 'cursor-pointer underline decoration-dotted underline-offset-[3px] hover:decoration-solid'
										: ''}"
								>
									{line2}
								</div>
							{/if}
						</div>

						{#if line3 && !editable}
							<div class="truncate text-[13px] leading-[17px] text-[#b5bac1]">{line3}</div>
						{/if}

						{#if showBar}
							<div class="mt-1 flex items-center gap-2">
								<span class="shrink-0 font-mono text-[11px] leading-none text-[#b5bac1]">
									{clock(position)}
								</span>
								<div class="h-[4px] flex-1 overflow-hidden rounded-full bg-white/15">
									<div class="h-full rounded-full bg-[#dbdee1]" style:width="{pct}%"></div>
								</div>
								<span class="shrink-0 font-mono text-[11px] leading-none text-[#b5bac1]">
									{clock(duration)}
								</span>
							</div>
						{/if}
					</div>
				</div>
			{/if}

			<!-- Action buttons (Listen Together is always present) -->
			{#if !cfg.hide_details}
				<div class="mt-3.5 flex flex-col gap-2">
					<!-- Always-present Listen Together Button -->
					<button
						type="button"
						class="group relative flex w-full items-center justify-center gap-2 rounded-[4px] bg-[#4e5058] hover:bg-[#5865f2] px-3 py-2 text-center text-[13px] leading-4 font-medium text-white transition-colors cursor-pointer shadow-xs"
						onclick={handleListenTogetherClick}
						title="Click to preview the Listen Together join request popup"
					>
						<HugeiconsIcon icon={Exchange01Icon} size={14} class="text-[#dbdee1] group-hover:text-white transition-colors" />
						<span>Listen Together</span>
						<span class="absolute right-2 rounded bg-black/30 px-1.5 py-0.5 text-[9.5px] font-medium text-[#dbdee1] group-hover:text-white">
							Test Popup
						</span>
					</button>

					<!-- Secondary button if enabled -->
					{#if cfg.button1 !== 'off'}
						<div
							class="rounded-[4px] bg-[#4e5058] px-3 py-2 text-center text-[13px] leading-4 font-medium text-white/90"
						>
							{cfg.button1 === 'listen' && customButtonLabel?.trim() ? customButtonLabel : (BUTTON_LABELS[cfg.button1] ?? cfg.button1)}
						</div>
					{/if}
				</div>
			{/if}
		</div>

		<!-- Member list status preview: bottom line of the preview box with status selector -->
		<div
			class="mt-4 w-full max-w-[420px] rounded-xl bg-[#2b2d31] p-3 ring-1 ring-black/40 select-none space-y-2.5"
		>
			<div class="flex items-center justify-between">
				<span class="text-[10px] font-bold uppercase tracking-wider text-[#949ba4]">
					Member list status preview
				</span>
				<span class="text-[10.5px] text-[#949ba4]">
					Discord sidebar view
				</span>
			</div>

			<div class="flex items-center gap-2.5 rounded-lg bg-[#232428] px-2.5 py-2 ring-1 ring-white/5">
				<div
					class="size-8 shrink-0 rounded-full bg-gradient-to-br from-[#5865f2] to-[#eb459e] ring-2 ring-[#232428] relative"
				>
					<span class="absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full bg-[#23a55a] ring-2 ring-[#232428]"></span>
				</div>
				<div class="min-w-0 flex-1">
					<div class="truncate text-[14px] leading-tight font-medium text-[#dbdee1]">
						You
					</div>
					<div class="flex items-center gap-1.5 text-[12px] leading-tight text-[#b5bac1]">
						<HugeiconsIcon icon={MusicNote01Icon} size={11} class="shrink-0 text-[#5865f2]" />
						<span class="truncate font-medium">{statusText}</span>
					</div>
				</div>
			</div>

			{#if editable}
				<!-- Quick status line selector attached to the status preview -->
				<div class="pt-1 flex flex-wrap items-center justify-between gap-1.5">
					<span class="text-[10.5px] text-[#949ba4] font-medium">Status text source:</span>
					<div class="flex items-center gap-1">
						<button
							type="button"
							class="rounded px-2 py-0.5 text-[10.5px] font-medium transition-colors cursor-pointer {cfg.status_line === 'line2' ? 'bg-[#5865f2] text-white' : 'bg-[#1e1f22] text-[#949ba4] hover:text-white'}"
							onclick={() => onSet?.({ status_line: 'line2' })}
							title="Show Bottom Line / Artist under your name"
						>
							Bottom Line
						</button>
						<button
							type="button"
							class="rounded px-2 py-0.5 text-[10.5px] font-medium transition-colors cursor-pointer {cfg.status_line === 'line1' ? 'bg-[#5865f2] text-white' : 'bg-[#1e1f22] text-[#949ba4] hover:text-white'}"
							onclick={() => onSet?.({ status_line: 'line1' })}
							title="Show Top Line / Title under your name"
						>
							Top Line
						</button>
						<button
							type="button"
							class="rounded px-2 py-0.5 text-[10.5px] font-medium transition-colors cursor-pointer {cfg.status_line === 'app' ? 'bg-[#5865f2] text-white' : 'bg-[#1e1f22] text-[#949ba4] hover:text-white'}"
							onclick={() => onSet?.({ status_line: 'app' })}
							title="Show App Name (Nocturne) under your name"
						>
							App Name
						</button>
					</div>
				</div>
			{/if}
		</div>
	</div>

	{#if offAir}
		<p class="mt-2.5 max-w-[420px] text-center text-xs leading-relaxed text-muted-foreground">
			{enabled ? 'Playback is paused — Discord clears the card while paused to keep timelines truthful.' : 'Discord Rich Presence is disabled.'}
		</p>
	{/if}
</div>
