<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Comment01Icon,
		ThumbsUpIcon,
		PinIcon,
		ArrowDown01Icon,
		ArrowUp01Icon,
		Refresh01Icon,
		SentIcon,
		MusicNote01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { CommentItem, CommentsPage } from '$lib/api';
	import { thumb } from '$lib/thumb';
	import { Button } from '$lib/components/ui/button';

	interface Props {
		videoId?: string;
	}

	let { videoId }: Props = $props();

	let comments = $state<CommentItem[]>([]);
	let continuation = $state<string | undefined>(undefined);
	let totalCount = $state<string | undefined>(undefined);
	let sortTokenTop = $state<string | undefined>(undefined);
	let sortTokenNewest = $state<string | undefined>(undefined);
	let createParams = $state<string | undefined>(undefined);

	let activeSort = $state<'top' | 'newest'>('top');
	let loading = $state(false);
	let loadingMore = $state(false);
	let error = $state<string | null>(null);

	// Posting new top-level comment
	let newCommentText = $state('');
	let isPosting = $state(false);

	// Track open reply forms and expanded replies
	let activeReplyId = $state<string | null>(null);
	let replyText = $state('');
	let isPostingReply = $state(false);

	let expandedReplies = $state<Record<string, boolean>>({});
	let commentReplies = $state<Record<string, CommentItem[]>>({});
	let replyContinuations = $state<Record<string, string | undefined>>({});
	let loadingReplies = $state<Record<string, boolean>>({});

	// Reset and load when videoId changes
	$effect(() => {
		if (videoId) {
			loadInitial(videoId);
		} else {
			comments = [];
			continuation = undefined;
			totalCount = undefined;
		}
	});

	async function loadInitial(vid: string, token?: string) {
		loading = true;
		error = null;
		activeReplyId = null;
		replyText = '';
		try {
			const res = await api.getComments(vid, token);
			comments = res.comments ?? [];
			continuation = res.continuation;
			totalCount = res.totalCount;
			sortTokenTop = res.sortTokenTop;
			sortTokenNewest = res.sortTokenNewest;
			createParams = res.createParams;
		} catch (err: any) {
			error = err?.message || 'Failed to load comments';
		} finally {
			loading = false;
		}
	}

	async function switchSort(sort: 'top' | 'newest') {
		if (activeSort === sort || !videoId) return;
		activeSort = sort;
		const token = sort === 'top' ? sortTokenTop : sortTokenNewest;
		await loadInitial(videoId, token);
	}

	async function loadMore() {
		if (!continuation || !videoId || loadingMore) return;
		loadingMore = true;
		try {
			const res = await api.getComments(videoId, continuation);
			if (res.comments?.length) {
				comments = [...comments, ...res.comments];
			}
			continuation = res.continuation;
		} catch (err) {
			console.error('Failed to load more comments:', err);
		} finally {
			loadingMore = false;
		}
	}

	async function postComment() {
		const text = newCommentText.trim();
		if (!text || !createParams || isPosting) return;
		isPosting = true;
		try {
			const created = await api.createComment(createParams, text);
			if (created) {
				comments = [created, ...comments];
			}
			newCommentText = '';
		} catch (err) {
			console.error('Failed to post comment:', err);
		} finally {
			isPosting = false;
		}
	}

	async function toggleReplies(comment: CommentItem) {
		const cid = comment.id;
		expandedReplies[cid] = !expandedReplies[cid];

		if (expandedReplies[cid] && !commentReplies[cid] && comment.repliesToken) {
			await fetchReplies(cid, comment.repliesToken);
		}
	}

	async function fetchReplies(cid: string, token: string) {
		loadingReplies[cid] = true;
		try {
			const res = await api.getCommentReplies(token);
			commentReplies[cid] = res.replies ?? [];
			replyContinuations[cid] = res.continuation;
		} catch (err) {
			console.error('Failed to load replies:', err);
		} finally {
			loadingReplies[cid] = false;
		}
	}

	async function loadMoreReplies(cid: string) {
		const token = replyContinuations[cid];
		if (!token || loadingReplies[cid]) return;
		loadingReplies[cid] = true;
		try {
			const res = await api.getCommentReplies(token);
			if (res.replies?.length) {
				commentReplies[cid] = [...(commentReplies[cid] ?? []), ...res.replies];
			}
			replyContinuations[cid] = res.continuation;
		} catch (err) {
			console.error('Failed to load more replies:', err);
		} finally {
			loadingReplies[cid] = false;
		}
	}

	async function handleLike(comment: CommentItem) {
		const action = comment.isLiked ? comment.unlikeAction : comment.likeAction;
		if (!action) return;

		// Optimistic update
		const wasLiked = comment.isLiked;
		comment.isLiked = !wasLiked;
		const curNum = parseInt(comment.likeCount?.replace(/[^0-9]/g, '') || '0', 10);
		if (comment.isLiked) {
			comment.likeCount = curNum > 0 ? `${curNum + 1}` : '1';
		} else if (curNum > 1) {
			comment.likeCount = `${curNum - 1}`;
		} else {
			comment.likeCount = undefined;
		}

		try {
			await api.performCommentAction(action);
		} catch (err) {
			// Rollback on error
			comment.isLiked = wasLiked;
			console.error('Failed to like comment:', err);
		}
	}

	async function postReply(comment: CommentItem) {
		const text = replyText.trim();
		const replyAction = comment.replyAction;
		if (!text || !replyAction || isPostingReply) return;
		isPostingReply = true;
		try {
			const created = await api.createCommentReply(replyAction, text);
			if (created) {
				commentReplies[comment.id] = [...(commentReplies[comment.id] ?? []), created];
				expandedReplies[comment.id] = true;
				comment.replyCount += 1;
			}
			replyText = '';
			activeReplyId = null;
		} catch (err) {
			console.error('Failed to post reply:', err);
		} finally {
			isPostingReply = false;
		}
	}
</script>

<div class="flex h-full w-full flex-col min-h-0 select-text overflow-hidden">
	<!-- Comments Header -->
	<div class="flex shrink-0 items-center justify-between border-b border-white/10 pb-3 px-1">
		<div class="flex items-center gap-2">
			<HugeiconsIcon icon={Comment01Icon} class="h-4 w-4 text-primary" />
			<span class="text-sm font-semibold tracking-wide text-foreground">
				{totalCount || 'Comments'}
			</span>
		</div>

		<div class="flex items-center gap-2">
			<!-- Sort Switcher -->
			<div class="flex items-center rounded-lg border border-white/10 bg-white/5 p-0.5 text-xs">
				<button
					type="button"
					onclick={() => switchSort('top')}
					class="rounded-md px-2.5 py-1 font-medium transition-all cursor-pointer {activeSort === 'top'
						? 'bg-white/20 dark:bg-white/15 text-foreground shadow-xs font-semibold'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					Top
				</button>
				<button
					type="button"
					onclick={() => switchSort('newest')}
					class="rounded-md px-2.5 py-1 font-medium transition-all cursor-pointer {activeSort === 'newest'
						? 'bg-white/20 dark:bg-white/15 text-foreground shadow-xs font-semibold'
						: 'text-muted-foreground hover:text-foreground'}"
				>
					Newest
				</button>
			</div>

			<Button
				variant="ghost"
				size="icon-xs"
				onclick={() => videoId && loadInitial(videoId)}
				class="h-7 w-7 rounded-lg text-muted-foreground hover:text-foreground cursor-pointer"
				title="Refresh comments"
			>
				<HugeiconsIcon icon={Refresh01Icon} class="h-3.5 w-3.5 {loading ? 'animate-spin' : ''}" />
			</Button>
		</div>
	</div>

	<!-- Add Comment Input Box -->
	{#if createParams}
		<div class="my-3 flex items-start gap-2.5 shrink-0 px-1">
			<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/20 text-primary font-bold text-xs ring-1 ring-white/10">
				Y
			</div>
			<div class="relative flex-1">
				<input
					type="text"
					bind:value={newCommentText}
					placeholder="Add a comment..."
					onkeydown={(e) => {
						if (e.key === 'Enter' && !e.shiftKey) {
							e.preventDefault();
							postComment();
						}
					}}
					class="w-full rounded-xl border border-white/10 bg-white/5 px-3 py-1.5 pr-9 text-xs text-foreground placeholder:text-muted-foreground focus:border-primary/50 focus:outline-none focus:ring-1 focus:ring-primary/40 backdrop-blur-md"
				/>
				<button
					type="button"
					onclick={postComment}
					disabled={!newCommentText.trim() || isPosting}
					class="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-lg p-1 text-primary hover:bg-primary/10 disabled:opacity-30 disabled:pointer-events-none cursor-pointer transition-colors"
					title="Post comment"
				>
					<HugeiconsIcon icon={SentIcon} class="h-3.5 w-3.5" />
				</button>
			</div>
		</div>
	{/if}

	<!-- Comment List Scroll Container -->
	<div class="flex-1 min-h-0 overflow-y-auto pr-1 space-y-4 pt-2 [scrollbar-width:thin]">
		{#if loading && !comments.length}
			<!-- Loading Skeleton -->
			<div class="space-y-3 p-2">
				{#each Array(5) as _}
					<div class="flex items-start gap-3 animate-pulse">
						<div class="h-8 w-8 rounded-full bg-white/10 shrink-0"></div>
						<div class="flex-1 space-y-1.5">
							<div class="h-3 w-28 rounded-md bg-white/10"></div>
							<div class="h-2.5 w-full rounded-md bg-white/10"></div>
							<div class="h-2.5 w-4/5 rounded-md bg-white/10"></div>
						</div>
					</div>
				{/each}
			</div>
		{:else if error}
			<div class="flex h-48 flex-col items-center justify-center gap-3 text-center px-4">
				<p class="text-xs text-muted-foreground">{error}</p>
				<Button
					variant="outline"
					size="sm"
					onclick={() => videoId && loadInitial(videoId)}
					class="text-xs rounded-lg cursor-pointer"
				>
					Try Again
				</Button>
			</div>
		{:else if !comments.length}
			<div class="flex h-48 flex-col items-center justify-center gap-2 text-center text-muted-foreground">
				<HugeiconsIcon icon={MusicNote01Icon} class="h-8 w-8 opacity-40" />
				<p class="text-xs font-medium">No comments on this track yet</p>
			</div>
		{:else}
			{#each comments as comment (comment.id)}
				<div class="group/comment flex items-start gap-3 text-left">
					<!-- Author Avatar -->
					{#if comment.authorAvatar}
						<img
							src={thumb(comment.authorAvatar, 64)}
							alt={comment.author}
							class="h-8 w-8 rounded-full object-cover shrink-0 ring-1 ring-white/10 shadow-sm"
						/>
					{:else}
						<div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-white/10 text-xs font-bold text-foreground">
							{comment.author.charAt(0).toUpperCase()}
						</div>
					{/if}

					<!-- Comment Content -->
					<div class="min-w-0 flex-1 space-y-1">
						<!-- Author Meta Line -->
						<div class="flex flex-wrap items-center gap-2 text-xs">
							<span class="font-semibold text-foreground/90 truncate max-w-[140px] sm:max-w-[200px]">
								{comment.author}
							</span>
							{#if comment.isChannelOwner}
								<span class="rounded bg-primary/20 px-1.5 py-0.2 text-[10px] font-bold text-primary tracking-wide">
									Creator
								</span>
							{/if}
							{#if comment.isPinned}
								<span class="flex items-center gap-0.5 text-[10px] text-muted-foreground">
									<HugeiconsIcon icon={PinIcon} class="h-2.5 w-2.5 text-primary" />
									Pinned
								</span>
							{/if}
							{#if comment.publishedTime}
								<span class="text-[11px] text-muted-foreground">
									{comment.publishedTime}
								</span>
							{/if}
						</div>

						<!-- Comment Body Text -->
						<p class="text-xs leading-relaxed text-foreground/85 whitespace-pre-wrap break-words">
							{comment.text}
						</p>

						<!-- Actions: Like, Reply toggle, Replies expand -->
						<div class="flex flex-wrap items-center gap-3 pt-1 text-xs">
							<!-- Like Button -->
							<button
								type="button"
								onclick={() => handleLike(comment)}
								class="flex items-center gap-1 text-[11px] font-medium transition-colors cursor-pointer {comment.isLiked
									? 'text-primary'
									: 'text-muted-foreground hover:text-foreground'}"
								title={comment.isLiked ? 'Unlike' : 'Like'}
							>
								<HugeiconsIcon icon={ThumbsUpIcon} class="h-3 w-3 {comment.isLiked ? 'fill-current' : ''}" />
								{#if comment.likeCount}
									<span class="tabular-nums">{comment.likeCount}</span>
								{/if}
							</button>

							<!-- Reply Button (If user can reply) -->
							{#if comment.replyAction}
								<button
									type="button"
									onclick={() => {
										activeReplyId = activeReplyId === comment.id ? null : comment.id;
										replyText = '';
									}}
									class="text-[11px] font-medium text-muted-foreground hover:text-foreground cursor-pointer transition-colors"
								>
									Reply
								</button>
							{/if}

							<!-- Thread Replies Button -->
							{#if comment.replyCount > 0 || comment.repliesToken}
								<button
									type="button"
									onclick={() => toggleReplies(comment)}
									class="flex items-center gap-1 text-[11px] font-semibold text-primary hover:underline cursor-pointer"
								>
									<HugeiconsIcon
										icon={expandedReplies[comment.id] ? ArrowUp01Icon : ArrowDown01Icon}
										class="h-3 w-3"
									/>
									<span>
										{expandedReplies[comment.id]
											? 'Hide replies'
											: `${comment.replyCount || ''} ${comment.replyCount === 1 ? 'reply' : 'replies'}`}
									</span>
								</button>
							{/if}
						</div>

						<!-- Inline Reply Input Box -->
						{#if activeReplyId === comment.id}
							<div class="mt-2 flex items-center gap-2 pl-2 border-l-2 border-primary/40">
								<input
									type="text"
									bind:value={replyText}
									placeholder="Write a reply..."
									onkeydown={(e) => {
										if (e.key === 'Enter' && !e.shiftKey) {
											e.preventDefault();
											postReply(comment);
										}
									}}
									class="flex-1 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1 text-xs text-foreground placeholder:text-muted-foreground focus:border-primary/50 focus:outline-none"
								/>
								<Button
									variant="ghost"
									size="icon-xs"
									onclick={() => postReply(comment)}
									disabled={!replyText.trim() || isPostingReply}
									class="h-7 w-7 rounded-lg text-primary cursor-pointer disabled:opacity-30"
								>
									<HugeiconsIcon icon={SentIcon} class="h-3.5 w-3.5" />
								</Button>
							</div>
						{/if}

						<!-- Nested Replies Tree -->
						{#if expandedReplies[comment.id]}
							<div class="mt-2.5 space-y-3 border-l-2 border-white/10 pl-3">
								{#if loadingReplies[comment.id] && !commentReplies[comment.id]?.length}
									<div class="text-[11px] text-muted-foreground animate-pulse">Loading replies...</div>
								{:else if commentReplies[comment.id]?.length}
									{#each commentReplies[comment.id] as reply (reply.id)}
										<div class="flex items-start gap-2.5 text-left">
											{#if reply.authorAvatar}
												<img
													src={thumb(reply.authorAvatar, 48)}
													alt={reply.author}
													class="h-6 w-6 rounded-full object-cover shrink-0 ring-1 ring-white/10"
												/>
											{:else}
												<div class="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-white/10 text-[10px] font-bold text-foreground">
													{reply.author.charAt(0).toUpperCase()}
												</div>
											{/if}
											<div class="min-w-0 flex-1 space-y-0.5">
												<div class="flex items-center gap-1.5 text-[11px]">
													<span class="font-semibold text-foreground/90 truncate max-w-[120px]">
														{reply.author}
													</span>
													{#if reply.isChannelOwner}
														<span class="rounded bg-primary/20 px-1 text-[9px] font-bold text-primary">
															Creator
														</span>
													{/if}
													{#if reply.publishedTime}
														<span class="text-[10px] text-muted-foreground">{reply.publishedTime}</span>
													{/if}
												</div>
												<p class="text-xs leading-relaxed text-foreground/85 whitespace-pre-wrap break-words">
													{reply.text}
												</p>
												<div class="flex items-center gap-2 pt-0.5 text-[11px]">
													<button
														type="button"
														onclick={() => handleLike(reply)}
														class="flex items-center gap-1 text-[10px] font-medium transition-colors cursor-pointer {reply.isLiked ? 'text-primary' : 'text-muted-foreground hover:text-foreground'}"
													>
														<HugeiconsIcon icon={ThumbsUpIcon} class="h-2.5 w-2.5 {reply.isLiked ? 'fill-current' : ''}" />
														{#if reply.likeCount}
															<span class="tabular-nums">{reply.likeCount}</span>
														{/if}
													</button>
												</div>
											</div>
										</div>
									{/each}

									{#if replyContinuations[comment.id]}
										<button
											type="button"
											onclick={() => loadMoreReplies(comment.id)}
											disabled={loadingReplies[comment.id]}
											class="text-[11px] font-medium text-primary hover:underline cursor-pointer disabled:opacity-50"
										>
											{loadingReplies[comment.id] ? 'Loading...' : 'Show more replies'}
										</button>
									{/if}
								{/if}
							</div>
						{/if}
					</div>
				</div>
			{/each}

			<!-- Load More Button -->
			{#if continuation}
				<div class="pt-2 pb-4 text-center">
					<Button
						variant="outline"
						size="sm"
						onclick={loadMore}
						disabled={loadingMore}
						class="rounded-xl border-white/10 bg-white/5 text-xs text-foreground backdrop-blur-md transition-all hover:bg-white/10 cursor-pointer"
					>
						{loadingMore ? 'Loading more comments...' : 'Load more comments'}
					</Button>
				</div>
			{/if}
		{/if}
	</div>
</div>
