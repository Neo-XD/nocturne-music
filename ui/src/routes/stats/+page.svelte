<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Analytics01Icon,
		PlayIcon,
		MusicNote01Icon,
		Clock01Icon,
		UserIcon,
		Album02Icon,
		Refresh01Icon,
		Sun01Icon,
		Sun02Icon,
		Moon02Icon,
		SunsetIcon,
		ArrowRight01Icon,
		Settings01Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import type { LocalStats, LastfmStats, SongItem } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { playback, toast } from '$lib/player.svelte';

	let activeSource = $state<'local' | 'lastfm'>('local');
	let timeRangeDays = $state<number>(30);
	let lastfmPeriod = $state<'7day' | '1month' | '3month' | '6month' | '12month' | 'overall'>('1month');

	let loading = $state(true);
	let localStats = $state<LocalStats | null>(null);
	let lastfmStats = $state<LastfmStats | null>(null);
	let lastfmConnected = $state(false);

	let activeTab = $state<'songs' | 'artists' | 'albums'>('songs');

	// Format duration into readable hours & minutes
	function formatDuration(seconds: number): string {
		if (seconds <= 0) return '0m';
		const hours = Math.floor(seconds / 3600);
		const mins = Math.floor((seconds % 3600) / 60);
		if (hours > 0) {
			return `${hours}h ${mins}m`;
		}
		return `${mins}m`;
	}

	// Calculate time-of-day distribution from hourly plays (using user's local timezone)
	const timeOfDayDistribution = $derived(() => {
		if (!localStats) {
			return { night: 0, morning: 0, afternoon: 0, evening: 0, peak: 'Evening' };
		}

		let night = 0; // 0..5
		let morning = 0; // 6..11
		let afternoon = 0; // 12..17
		let evening = 0; // 18..23

		if (localStats.recentPlays?.length) {
			for (const r of localStats.recentPlays) {
				const hour = new Date(r.playedAt * 1000).getHours();
				if (hour >= 0 && hour < 6) night++;
				else if (hour >= 6 && hour < 12) morning++;
				else if (hour >= 12 && hour < 18) afternoon++;
				else evening++;
			}
		} else {
			for (let h = 0; h < 24; h++) {
				const count = localStats.hourlyPlays[h] ?? 0;
				if (h < 6) night += count;
				else if (h < 12) morning += count;
				else if (h < 18) afternoon += count;
				else evening += count;
			}
		}

		const total = night + morning + afternoon + evening || 1;
		const pNight = Math.round((night / total) * 100);
		const pMorning = Math.round((morning / total) * 100);
		const pAfternoon = Math.round((afternoon / total) * 100);
		const pEvening = Math.round((evening / total) * 100);

		let peak = 'Evening';
		let maxVal = evening;
		if (night > maxVal) { peak = 'Night'; maxVal = night; }
		if (morning > maxVal) { peak = 'Morning'; maxVal = morning; }
		if (afternoon > maxVal) { peak = 'Afternoon'; maxVal = afternoon; }

		return {
			night: pNight,
			morning: pMorning,
			afternoon: pAfternoon,
			evening: pEvening,
			peak
		};
	});

	async function loadAll() {
		loading = true;
		try {
			const [localRes, lfmStatus, lfmRes] = await Promise.allSettled([
				api.getLocalStats(timeRangeDays),
				api.lastfmStatus(),
				api.getLastfmStats(lastfmPeriod)
			]);

			if (localRes.status === 'fulfilled') {
				localStats = localRes.value;
			}
			if (lfmStatus.status === 'fulfilled' && lfmStatus.value?.connected) {
				lastfmConnected = true;
			}
			if (lfmRes.status === 'fulfilled') {
				lastfmStats = lfmRes.value;
			}
		} catch (e) {
			console.error('Failed to load stats:', e);
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		// Re-fetch when time range changes
		if (timeRangeDays) {
			api.getLocalStats(timeRangeDays).then((res) => {
				localStats = res;
			}).catch(() => {});
		}
	});

	$effect(() => {
		// Re-fetch Last.fm when period changes
		if (lastfmPeriod) {
			api.getLastfmStats(lastfmPeriod).then((res) => {
				lastfmStats = res;
			}).catch(() => {});
		}
	});

	onMount(() => {
		loadAll();
	});

	function playSong(song: SongItem) {
		api.play(song);
	}

	function playAllTopSongs() {
		if (!localStats?.topSongs?.length) return;
		const tracks = localStats.topSongs.map((s) => s.song);
		api.playPlaylist(tracks, 0, undefined, 'Top Listening Stats', false);
		toast.success(`Playing top ${tracks.length} tracks`);
	}
</script>

<div class="relative flex h-full w-full flex-col overflow-y-auto px-6 py-8 sm:px-10 lg:px-14 xl:px-16 select-text">
	<!-- Page Header -->
	<header class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between border-b border-border/50 pb-6">
		<div class="flex items-center gap-3.5">
			<div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-primary/10 text-primary shadow-sm border border-primary/20">
				<HugeiconsIcon icon={Analytics01Icon} class="h-6 w-6" />
			</div>
			<div>
				<h1 class="font-heading text-2xl sm:text-3xl font-extrabold tracking-tight text-foreground">
					Listening Stats
				</h1>
				<p class="text-xs sm:text-sm text-muted-foreground mt-0.5">
					Your listening habits, top tracks, artists, and scrobble analytics
				</p>
			</div>
		</div>

		<!-- Action Controls -->
		<div class="flex items-center gap-2.5 flex-wrap">
			<!-- Source Switcher: Local vs Last.fm -->
			<div class="flex items-center rounded-xl border border-border/80 bg-muted/40 p-1 text-xs">
				<button
					type="button"
					onclick={() => (activeSource = 'local')}
					class="rounded-lg px-3 py-1.5 font-medium transition-all cursor-pointer {activeSource === 'local'
						? 'bg-background text-foreground shadow-xs font-semibold'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					Local App
				</button>
				<button
					type="button"
					onclick={() => (activeSource = 'lastfm')}
					class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 font-medium transition-all cursor-pointer {activeSource === 'lastfm'
						? 'bg-background text-foreground shadow-xs font-semibold'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					<span>Last.fm</span>
					{#if lastfmConnected}
						<span class="h-1.5 w-1.5 rounded-full bg-emerald-500"></span>
					{/if}
				</button>
			</div>

			{#if activeSource === 'local'}
				<!-- Time Period Selector -->
				<div class="flex items-center rounded-xl border border-border/80 bg-muted/40 p-1 text-xs">
					<button
						type="button"
						onclick={() => (timeRangeDays = 7)}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {timeRangeDays === 7
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						7D
					</button>
					<button
						type="button"
						onclick={() => (timeRangeDays = 30)}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {timeRangeDays === 30
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						30D
					</button>
					<button
						type="button"
						onclick={() => (timeRangeDays = 90)}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {timeRangeDays === 90
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						90D
					</button>
					<button
						type="button"
						onclick={() => (timeRangeDays = 0)}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {timeRangeDays === 0
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						All
					</button>
				</div>

				{#if localStats?.topSongs?.length}
					<Button
						size="sm"
						variant="secondary"
						onclick={playAllTopSongs}
						class="gap-1.5 rounded-xl cursor-pointer shadow-xs"
					>
						<HugeiconsIcon icon={PlayIcon} class="h-3.5 w-3.5 fill-current" />
						<span>Play Top</span>
					</Button>
				{/if}
			{:else}
				<!-- Last.fm Period Selector -->
				<div class="flex items-center rounded-xl border border-border/80 bg-muted/40 p-1 text-xs">
					<button
						type="button"
						onclick={() => (lastfmPeriod = '7day')}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {lastfmPeriod === '7day'
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						7D
					</button>
					<button
						type="button"
						onclick={() => (lastfmPeriod = '1month')}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {lastfmPeriod === '1month'
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						1M
					</button>
					<button
						type="button"
						onclick={() => (lastfmPeriod = '3month')}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {lastfmPeriod === '3month'
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						3M
					</button>
					<button
						type="button"
						onclick={() => (lastfmPeriod = 'overall')}
						class="rounded-lg px-2.5 py-1.5 font-medium transition-all cursor-pointer {lastfmPeriod === 'overall'
							? 'bg-background text-foreground shadow-xs font-semibold'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						All
					</button>
				</div>
			{/if}

			<Button
				size="icon-sm"
				variant="ghost"
				onclick={loadAll}
				class="rounded-xl text-muted-foreground hover:text-foreground cursor-pointer"
				title="Refresh stats"
			>
				<HugeiconsIcon icon={Refresh01Icon} class="h-4 w-4 {loading ? 'animate-spin' : ''}" />
			</Button>
		</div>
	</header>

	<!-- Main Stats Body -->
	<main class="mt-8 space-y-8 pb-16">
		{#if loading && !localStats && !lastfmStats}
			<div class="flex flex-col items-center justify-center py-24 gap-3 text-muted-foreground">
				<div class="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
				<p class="text-sm font-medium">Gathering your listening insights…</p>
			</div>
		{:else if activeSource === 'local'}
			<!-- LOCAL APP STATS VIEW -->

			<!-- Overview Metrics Row -->
			<section class="grid grid-cols-2 lg:grid-cols-4 gap-4">
				<div class="flex flex-col rounded-2xl border border-border/70 bg-card/60 backdrop-blur-md p-5 shadow-xs">
					<div class="flex items-center justify-between text-muted-foreground">
						<span class="text-xs font-semibold uppercase tracking-wider">Listening Time</span>
						<HugeiconsIcon icon={Clock01Icon} class="h-4 w-4 text-primary" />
					</div>
					<div class="mt-3 text-3xl font-extrabold tracking-tight text-foreground">
						{formatDuration(localStats?.totalDurationSecs ?? 0)}
					</div>
					<span class="mt-1 text-xs text-muted-foreground">
						{timeRangeDays === 0 ? 'All recorded history' : `Past ${timeRangeDays} days`}
					</span>
				</div>

				<div class="flex flex-col rounded-2xl border border-border/70 bg-card/60 backdrop-blur-md p-5 shadow-xs">
					<div class="flex items-center justify-between text-muted-foreground">
						<span class="text-xs font-semibold uppercase tracking-wider">Total Plays</span>
						<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4 text-primary" />
					</div>
					<div class="mt-3 text-3xl font-extrabold tracking-tight text-foreground">
						{(localStats?.totalPlays ?? 0).toLocaleString()}
					</div>
					<span class="mt-1 text-xs text-muted-foreground">Completed track plays</span>
				</div>

				<div class="flex flex-col rounded-2xl border border-border/70 bg-card/60 backdrop-blur-md p-5 shadow-xs">
					<div class="flex items-center justify-between text-muted-foreground">
						<span class="text-xs font-semibold uppercase tracking-wider">Unique Tracks</span>
						<HugeiconsIcon icon={Album02Icon} class="h-4 w-4 text-primary" />
					</div>
					<div class="mt-3 text-3xl font-extrabold tracking-tight text-foreground">
						{(localStats?.uniqueTracks ?? 0).toLocaleString()}
					</div>
					<span class="mt-1 text-xs text-muted-foreground">Different songs explored</span>
				</div>

				<div class="flex flex-col rounded-2xl border border-border/70 bg-card/60 backdrop-blur-md p-5 shadow-xs">
					<div class="flex items-center justify-between text-muted-foreground">
						<span class="text-xs font-semibold uppercase tracking-wider">Artists Heard</span>
						<HugeiconsIcon icon={UserIcon} class="h-4 w-4 text-primary" />
					</div>
					<div class="mt-3 text-3xl font-extrabold tracking-tight text-foreground">
						{(localStats?.uniqueArtists ?? 0).toLocaleString()}
					</div>
					<span class="mt-1 text-xs text-muted-foreground">Distinct music creators</span>
				</div>
			</section>

			<!-- Listening Habits by Time of Day -->
			{@const tod = timeOfDayDistribution()}
			<section class="rounded-3xl border border-border/70 bg-card/40 backdrop-blur-md p-6 shadow-sm">
				<div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 mb-6">
					<div>
						<h2 class="text-base font-bold text-foreground tracking-tight">Listening Habits by Time of Day</h2>
						<p class="text-xs text-muted-foreground">When you stream and explore music the most</p>
					</div>
					<div class="inline-flex items-center gap-1.5 self-start rounded-full bg-primary/10 border border-primary/20 px-3 py-1 text-xs font-semibold text-primary">
						<span>Peak Time:</span>
						<span class="capitalize font-bold">{tod.peak}</span>
					</div>
				</div>

				<!-- Visual Segmented Bar -->
				<div class="relative flex h-3.5 w-full overflow-hidden rounded-full bg-muted/60 mb-6">
					<div style="width: {tod.morning}%;" class="bg-amber-400 transition-all duration-500" title="Morning: {tod.morning}%"></div>
					<div style="width: {tod.afternoon}%;" class="bg-orange-500 transition-all duration-500" title="Afternoon: {tod.afternoon}%"></div>
					<div style="width: {tod.evening}%;" class="bg-indigo-500 transition-all duration-500" title="Evening: {tod.evening}%"></div>
					<div style="width: {tod.night}%;" class="bg-blue-600 transition-all duration-500" title="Night: {tod.night}%"></div>
				</div>

				<!-- 4 Time Quadrants Grid -->
				<div class="grid grid-cols-2 md:grid-cols-4 gap-3">
					<div class="flex items-center gap-3 rounded-2xl border border-border/40 bg-muted/20 p-3.5">
						<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-amber-400/15 text-amber-400">
							<HugeiconsIcon icon={Sun01Icon} class="h-4.5 w-4.5" />
						</div>
						<div>
							<span class="block text-xs font-semibold text-foreground">Morning</span>
							<span class="text-[11px] text-muted-foreground">6 AM – 12 PM ({tod.morning}%)</span>
						</div>
					</div>

					<div class="flex items-center gap-3 rounded-2xl border border-border/40 bg-muted/20 p-3.5">
						<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-orange-500/15 text-orange-500">
							<HugeiconsIcon icon={Sun02Icon} class="h-4.5 w-4.5" />
						</div>
						<div>
							<span class="block text-xs font-semibold text-foreground">Afternoon</span>
							<span class="text-[11px] text-muted-foreground">12 PM – 6 PM ({tod.afternoon}%)</span>
						</div>
					</div>

					<div class="flex items-center gap-3 rounded-2xl border border-border/40 bg-muted/20 p-3.5">
						<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-indigo-500/15 text-indigo-400">
							<HugeiconsIcon icon={SunsetIcon} class="h-4.5 w-4.5" />
						</div>
						<div>
							<span class="block text-xs font-semibold text-foreground">Evening</span>
							<span class="text-[11px] text-muted-foreground">6 PM – 12 AM ({tod.evening}%)</span>
						</div>
					</div>

					<div class="flex items-center gap-3 rounded-2xl border border-border/40 bg-muted/20 p-3.5">
						<div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-blue-600/15 text-blue-400">
							<HugeiconsIcon icon={Moon02Icon} class="h-4.5 w-4.5" />
						</div>
						<div>
							<span class="block text-xs font-semibold text-foreground">Night</span>
							<span class="text-[11px] text-muted-foreground">12 AM – 6 AM ({tod.night}%)</span>
						</div>
					</div>
				</div>
			</section>

			<!-- Top Content Navigation Tabs -->
			<section class="space-y-4">
				<div class="flex items-center justify-between border-b border-border/50 pb-2">
					<div class="flex items-center gap-2">
						<button
							type="button"
							onclick={() => (activeTab = 'songs')}
							class="rounded-xl px-4 py-2 text-xs font-semibold transition-all cursor-pointer {activeTab === 'songs'
								? 'bg-primary text-primary-foreground shadow-sm'
								: 'text-muted-foreground hover:text-foreground hover:bg-muted/40'}"
						>
							Top Songs ({localStats?.topSongs?.length ?? 0})
						</button>
						<button
							type="button"
							onclick={() => (activeTab = 'artists')}
							class="rounded-xl px-4 py-2 text-xs font-semibold transition-all cursor-pointer {activeTab === 'artists'
								? 'bg-primary text-primary-foreground shadow-sm'
								: 'text-muted-foreground hover:text-foreground hover:bg-muted/40'}"
						>
							Top Artists ({localStats?.topArtists?.length ?? 0})
						</button>
						<button
							type="button"
							onclick={() => (activeTab = 'albums')}
							class="rounded-xl px-4 py-2 text-xs font-semibold transition-all cursor-pointer {activeTab === 'albums'
								? 'bg-primary text-primary-foreground shadow-sm'
								: 'text-muted-foreground hover:text-foreground hover:bg-muted/40'}"
						>
							Top Albums ({localStats?.topAlbums?.length ?? 0})
						</button>
					</div>
				</div>

				<!-- TAB CONTENT: Top Songs -->
				{#if activeTab === 'songs'}
					{#if !localStats?.topSongs?.length}
						<div class="flex flex-col items-center justify-center rounded-2xl border border-dashed border-border/60 py-16 text-center text-muted-foreground">
							<HugeiconsIcon icon={MusicNote01Icon} class="h-10 w-10 mb-2.5 opacity-40" />
							<p class="text-sm font-semibold">No recorded plays in this timeframe</p>
							<p class="text-xs text-muted-foreground/80 mt-1">Listen to songs in Nocturne to see them appear here.</p>
						</div>
					{:else}
						<div class="grid grid-cols-1 md:grid-cols-2 gap-2.5">
							{#each localStats.topSongs as { song, playCount }, idx}
								{@const isCurrent = playback.now?.videoId === song.video_id}
								<div
									role="button"
									tabindex="0"
									onclick={() => playSong(song)}
									onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && playSong(song)}
									class="group flex items-center gap-3.5 rounded-2xl border border-border/40 bg-card/40 hover:bg-muted/40 p-3 transition-all duration-150 cursor-pointer {isCurrent ? 'ring-1 ring-primary bg-primary/10' : ''}"
								>
									<!-- Rank -->
									<span class="w-6 text-center text-xs font-black {idx < 3 ? 'text-primary' : 'text-muted-foreground/60'}">
										{idx + 1}
									</span>

									<!-- Artwork -->
									<div class="relative h-12 w-12 shrink-0 overflow-hidden rounded-xl bg-muted shadow-xs">
										{#if song.thumbnail}
											<img
												src={thumb(song.thumbnail, 96)}
												alt={song.title}
												class="h-full w-full object-cover transition-transform group-hover:scale-105"
												loading="lazy"
											/>
										{:else}
											<div class="flex h-full w-full items-center justify-center text-muted-foreground">
												<HugeiconsIcon icon={MusicNote01Icon} class="h-5 w-5" />
											</div>
										{/if}
										<div class="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity">
											<HugeiconsIcon icon={PlayIcon} class="h-5 w-5 fill-white text-white ml-0.5" />
										</div>
									</div>

									<!-- Title & Artist -->
									<div class="min-w-0 flex-1">
										<p class="text-xs font-bold text-foreground truncate {isCurrent ? 'text-primary' : ''}">
											{song.title}
										</p>
										<p class="text-[11px] text-muted-foreground truncate mt-0.5">
											{song.artists}
										</p>
									</div>

									<!-- Plays Badge -->
									<span class="shrink-0 rounded-full bg-primary/10 border border-primary/20 px-2.5 py-1 text-[11px] font-bold text-primary tabular-nums">
										{playCount} {playCount === 1 ? 'play' : 'plays'}
									</span>
								</div>
							{/each}
						</div>
					{/if}

				<!-- TAB CONTENT: Top Artists -->
				{:else if activeTab === 'artists'}
					{#if !localStats?.topArtists?.length}
						<div class="flex flex-col items-center justify-center rounded-2xl border border-dashed border-border/60 py-16 text-center text-muted-foreground">
							<HugeiconsIcon icon={UserIcon} class="h-10 w-10 mb-2.5 opacity-40" />
							<p class="text-sm font-semibold">No artists found in this timeframe</p>
						</div>
					{:else}
						<div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
							{#each localStats.topArtists as { name, playCount }, idx}
								<div class="flex flex-col items-center text-center rounded-2xl border border-border/40 bg-card/40 p-4 transition-all hover:bg-muted/40">
									<div class="flex h-16 w-16 items-center justify-center rounded-full bg-primary/15 text-primary text-lg font-bold mb-3 shadow-xs">
										{name.charAt(0).toUpperCase()}
									</div>
									<h4 class="text-xs font-bold text-foreground truncate w-full" title={name}>{name}</h4>
									<span class="mt-1 text-[11px] font-semibold text-muted-foreground">
										{playCount} {playCount === 1 ? 'play' : 'plays'}
									</span>
								</div>
							{/each}
						</div>
					{/if}

				<!-- TAB CONTENT: Top Albums -->
				{:else if activeTab === 'albums'}
					{#if !localStats?.topAlbums?.length}
						<div class="flex flex-col items-center justify-center rounded-2xl border border-dashed border-border/60 py-16 text-center text-muted-foreground">
							<HugeiconsIcon icon={Album02Icon} class="h-10 w-10 mb-2.5 opacity-40" />
							<p class="text-sm font-semibold">No albums recorded in this timeframe</p>
						</div>
					{:else}
						<div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3.5">
							{#each localStats.topAlbums as { name, artist, thumbnail, playCount }, idx}
								<div class="group flex flex-col rounded-2xl border border-border/40 bg-card/40 p-3 transition-all hover:bg-muted/40">
									<div class="relative aspect-square w-full overflow-hidden rounded-xl bg-muted mb-2.5 shadow-xs">
										{#if thumbnail}
											<img src={thumb(thumbnail, 240)} alt={name} class="h-full w-full object-cover transition-transform group-hover:scale-105" loading="lazy" />
										{:else}
											<div class="flex h-full w-full items-center justify-center text-muted-foreground">
												<HugeiconsIcon icon={Album02Icon} class="h-8 w-8" />
											</div>
										{/if}
									</div>
									<h4 class="text-xs font-bold text-foreground truncate" title={name}>{name}</h4>
									<p class="text-[11px] text-muted-foreground truncate">{artist}</p>
									<span class="mt-1 text-[10px] font-semibold text-primary">
										{playCount} {playCount === 1 ? 'play' : 'plays'}
									</span>
								</div>
							{/each}
						</div>
					{/if}
				{/if}
			</section>

		{:else}
			<!-- LAST.FM STATS VIEW -->
			{#if !lastfmConnected}
				<div class="flex flex-col items-center justify-center rounded-3xl border border-dashed border-border/70 p-12 text-center bg-card/20 max-w-xl mx-auto my-8">
					<div class="flex h-16 w-16 items-center justify-center rounded-2xl bg-red-500/10 text-red-500 mb-4">
						<HugeiconsIcon icon={Analytics01Icon} class="h-8 w-8" />
					</div>
					<h3 class="text-lg font-bold text-foreground">Last.fm is not connected</h3>
					<p class="text-xs text-muted-foreground mt-1.5 max-w-sm">
						Connect your Last.fm account in Settings to sync and view your global scrobbling history, top artists, tracks, and albums here.
					</p>
					<a
						href="/settings"
						class="mt-5 inline-flex items-center gap-2 rounded-xl bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground shadow-sm hover:opacity-95 transition-all cursor-pointer"
					>
						<HugeiconsIcon icon={Settings01Icon} class="h-4 w-4" />
						<span>Open Settings to Connect</span>
					</a>
				</div>
			{:else if lastfmStats}
				<!-- Last.fm User Profile Banner -->
				{#if lastfmStats.profile}
					<section class="flex flex-col sm:flex-row items-center justify-between gap-6 rounded-3xl border border-border/70 bg-gradient-to-r from-red-500/10 via-card/50 to-card/30 backdrop-blur-md p-6 shadow-xs">
						<div class="flex items-center gap-4 min-w-0">
							{#if lastfmStats.profile.image}
								<img
									src={lastfmStats.profile.image}
									alt={lastfmStats.username}
									class="h-16 w-16 shrink-0 rounded-2xl object-cover ring-2 ring-red-500/30 shadow-md"
								/>
							{:else}
								<div class="flex h-16 w-16 shrink-0 items-center justify-center rounded-2xl bg-red-500/20 text-red-500 font-bold text-xl">
									{lastfmStats.username.charAt(0).toUpperCase()}
								</div>
							{/if}
							<div class="min-w-0">
								<div class="flex items-center gap-2">
									<h3 class="text-base font-extrabold text-foreground truncate">{lastfmStats.username}</h3>
									<span class="rounded-full bg-red-500/15 border border-red-500/20 px-2 py-0.5 text-[10px] font-bold text-red-500">
										Last.fm
									</span>
								</div>
								{#if lastfmStats.profile.url}
									<a
										href={lastfmStats.profile.url}
										target="_blank"
										rel="noreferrer"
										class="text-xs text-muted-foreground hover:text-red-500 transition-colors inline-flex items-center gap-1 mt-0.5"
									>
										<span>View Last.fm Profile</span>
										<HugeiconsIcon icon={ArrowRight01Icon} class="h-3 w-3" />
									</a>
								{/if}
							</div>
						</div>

						<div class="flex items-center gap-4 sm:gap-6 shrink-0 text-center">
							<div>
								<span class="block text-xl font-extrabold text-foreground">{lastfmStats.profile.scrobbles.toLocaleString()}</span>
								<span class="text-[11px] text-muted-foreground">Scrobbles</span>
							</div>
							<div class="h-8 w-px bg-border/60"></div>
							<div>
								<span class="block text-xl font-extrabold text-foreground">{lastfmStats.profile.artists.toLocaleString()}</span>
								<span class="text-[11px] text-muted-foreground">Artists</span>
							</div>
							<div class="h-8 w-px bg-border/60"></div>
							<div>
								<span class="block text-xl font-extrabold text-foreground">{lastfmStats.profile.tracks.toLocaleString()}</span>
								<span class="text-[11px] text-muted-foreground">Tracks</span>
							</div>
						</div>
					</section>
				{/if}

				<!-- Last.fm Top Grids -->
				<section class="space-y-6">
					<!-- Top Tracks -->
					<div>
						<h3 class="text-sm font-bold text-foreground mb-3 flex items-center gap-2">
							<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4 text-red-500" />
							<span>Top Tracks ({lastfmStats.topTracks.length})</span>
						</h3>
						<div class="grid grid-cols-1 md:grid-cols-2 gap-2.5">
							{#each lastfmStats.topTracks as item, idx}
								<div class="flex items-center gap-3.5 rounded-2xl border border-border/40 bg-card/40 p-3 transition-colors hover:bg-muted/40">
									<span class="w-6 text-center text-xs font-black {idx < 3 ? 'text-red-500' : 'text-muted-foreground/60'}">
										{idx + 1}
									</span>
									<div class="relative h-11 w-11 shrink-0 overflow-hidden rounded-xl bg-muted">
										{#if item.image}
											<img src={item.image} alt={item.name} class="h-full w-full object-cover" loading="lazy" />
										{:else}
											<div class="flex h-full w-full items-center justify-center text-muted-foreground">
												<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4" />
											</div>
										{/if}
									</div>
									<div class="min-w-0 flex-1">
										<p class="text-xs font-bold text-foreground truncate">{item.name}</p>
										<p class="text-[11px] text-muted-foreground truncate">{item.artist || 'Unknown'}</p>
									</div>
									<span class="shrink-0 rounded-full bg-red-500/10 border border-red-500/20 px-2.5 py-0.5 text-[11px] font-bold text-red-500">
										{item.playCount} scrobbles
									</span>
								</div>
							{/each}
						</div>
					</div>

					<!-- Top Artists -->
					<div>
						<h3 class="text-sm font-bold text-foreground mb-3 flex items-center gap-2">
							<HugeiconsIcon icon={UserIcon} class="h-4 w-4 text-red-500" />
							<span>Top Artists ({lastfmStats.topArtists.length})</span>
						</h3>
						<div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
							{#each lastfmStats.topArtists as item, idx}
								<div class="flex flex-col items-center text-center rounded-2xl border border-border/40 bg-card/40 p-4 transition-colors hover:bg-muted/40">
									<div class="relative h-16 w-16 shrink-0 overflow-hidden rounded-full bg-muted mb-2.5 shadow-xs">
										{#if item.image}
											<img src={item.image} alt={item.name} class="h-full w-full object-cover" loading="lazy" />
										{:else}
											<div class="flex h-full w-full items-center justify-center bg-red-500/15 text-red-500 font-bold text-lg">
												{item.name.charAt(0).toUpperCase()}
											</div>
										{/if}
									</div>
									<h4 class="text-xs font-bold text-foreground truncate w-full" title={item.name}>{item.name}</h4>
									<span class="mt-1 text-[11px] font-semibold text-muted-foreground">
										{item.playCount} scrobbles
									</span>
								</div>
							{/each}
						</div>
					</div>

					<!-- Top Albums -->
					<div>
						<h3 class="text-sm font-bold text-foreground mb-3 flex items-center gap-2">
							<HugeiconsIcon icon={Album02Icon} class="h-4 w-4 text-red-500" />
							<span>Top Albums ({lastfmStats.topAlbums.length})</span>
						</h3>
						<div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3.5">
							{#each lastfmStats.topAlbums as item, idx}
								<div class="group flex flex-col rounded-2xl border border-border/40 bg-card/40 p-3 transition-colors hover:bg-muted/40">
									<div class="relative aspect-square w-full overflow-hidden rounded-xl bg-muted mb-2.5 shadow-xs">
										{#if item.image}
											<img src={item.image} alt={item.name} class="h-full w-full object-cover" loading="lazy" />
										{:else}
											<div class="flex h-full w-full items-center justify-center text-muted-foreground">
												<HugeiconsIcon icon={Album02Icon} class="h-8 w-8" />
											</div>
										{/if}
									</div>
									<h4 class="text-xs font-bold text-foreground truncate" title={item.name}>{item.name}</h4>
									<p class="text-[11px] text-muted-foreground truncate">{item.artist || ''}</p>
									<span class="mt-1 text-[10px] font-semibold text-red-500">
										{item.playCount} scrobbles
									</span>
								</div>
							{/each}
						</div>
					</div>
				</section>
			{/if}
		{/if}
	</main>
</div>
