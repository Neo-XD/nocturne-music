//! YouTube comments model and parser.
//!
//! Handles top-level comment threads, nested replies, pagination continuation tokens,
//! and action payloads (likes, replies, posting).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::metadata::{find_all, find_first_str, last_thumbnail, runs_text};

/// A single YouTube comment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItem {
    pub id: String,
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_channel_id: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub like_count: Option<String>,
    #[serde(default)]
    pub reply_count: i64,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub is_channel_owner: bool,
    #[serde(default)]
    pub is_liked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replies_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub like_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlike_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_action: Option<String>,
}

/// A paginated page of YouTube comments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsPage {
    pub comments: Vec<CommentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_token_top: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_token_newest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_params: Option<String>,
}

/// Result of fetching replies for a comment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentRepliesResult {
    pub replies: Vec<CommentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
}

/// Extract the root comments continuation token from a `/next` video response.
pub fn extract_root_comments_token(root: &Value) -> Option<String> {
    // 1. Look for itemSectionRenderer with comments sectionIdentifier / targetId
    for section in find_all(root, "itemSectionRenderer") {
        let is_comments = section
            .get("targetId")
            .or_else(|| section.get("sectionIdentifier"))
            .and_then(Value::as_str)
            .is_some_and(|id| id.contains("comment"));

        if is_comments {
            if let Some(token) = extract_continuation_token(section) {
                return Some(token);
            }
        }
    }

    // 2. Look for engagementPanelSectionListRenderer with comments target
    for panel in find_all(root, "engagementPanelSectionListRenderer") {
        let is_comments = panel
            .get("targetId")
            .or_else(|| panel.get("panelIdentifier"))
            .and_then(Value::as_str)
            .is_some_and(|id| id.contains("comment"));

        if is_comments {
            if let Some(token) = extract_continuation_token(panel) {
                return Some(token);
            }
        }
    }

    // 3. Fallback: look for continuationCommand whose token has comment params
    for cmd in find_all(root, "continuationCommand") {
        if let Some(token) = cmd.get("token").and_then(Value::as_str) {
            return Some(token.to_owned());
        }
    }

    None
}

/// Parse comments page from InnerTube continuation response.
pub fn parse_comments_page(root: &Value) -> CommentsPage {
    let mut page = CommentsPage::default();

    // Parse header info (total count, sort tokens, createParams)
    if let Some(header) = find_all(root, "commentsHeaderRenderer").into_iter().next() {
        page.total_count = runs_text(header.get("countText")).or_else(|| {
            header.pointer("/countText/simpleText").and_then(Value::as_str).map(str::to_owned)
        });

        // Sort menu tokens
        for item in find_all(header, "sortFilterSubMenuItem") {
            let title = item.get("title").and_then(Value::as_str).unwrap_or_default();
            let token = item
                .pointer("/serviceEndpoint/continuationCommand/token")
                .and_then(Value::as_str)
                .map(str::to_owned);

            if title.to_lowercase().contains("top") {
                page.sort_token_top = token;
            } else if title.to_lowercase().contains("new") {
                page.sort_token_newest = token;
            }
        }

        // Create comment token (for posting top-level comments)
        let create_ptr = "/createRenderer/commentSimpleboxRenderer/serviceEndpoint/\
            createCommentEndpoint/createCommentParams";
        page.create_params = header.pointer(create_ptr).and_then(Value::as_str).map(str::to_owned);
    }

    // Gather mutations map if ViewModel architecture is used
    let mut mutations_map = std::collections::HashMap::new();
    for mutation in find_all(root, "mutations").into_iter().flat_map(|m| m.as_array()).flatten() {
        if let (Some(k), Some(p)) = (
            mutation.get("entityKey").and_then(Value::as_str),
            mutation.pointer("/payload/commentEntityPayload"),
        ) {
            mutations_map.insert(k.to_owned(), p);
        }
    }

    // Parse comment threads
    for thread in find_all(root, "commentThreadRenderer") {
        let comment_node = thread.get("comment");
        if let Some(comment) = comment_node.and_then(|c| parse_single_comment(c, &mutations_map)) {
            let mut item = comment;
            // Replies token for this thread
            item.replies_token = thread.get("replies").and_then(extract_continuation_token);
            page.comments.push(item);
        }
    }

    // Next page continuation token (skip replies continuations)
    for cont in find_all(root, "continuationItemRenderer") {
        // Ensure it's not a reply button or nested thread
        let is_reply = cont.pointer("/button/buttonRenderer/command/continuationCommand").is_some();
        if !is_reply {
            if let Some(token) = extract_continuation_token(cont) {
                page.continuation = Some(token);
                break;
            }
        }
    }

    page
}

/// Parse comment replies from InnerTube continuation response.
pub fn parse_comment_replies(root: &Value) -> CommentRepliesResult {
    let mut replies = Vec::new();
    let mut continuation = None;

    let mut mutations_map = std::collections::HashMap::new();
    for mutation in find_all(root, "mutations").into_iter().flat_map(|m| m.as_array()).flatten() {
        if let (Some(k), Some(p)) = (
            mutation.get("entityKey").and_then(Value::as_str),
            mutation.pointer("/payload/commentEntityPayload"),
        ) {
            mutations_map.insert(k.to_owned(), p);
        }
    }

    for node in find_all(root, "commentRenderer") {
        if let Some(item) = parse_comment_renderer(node) {
            replies.push(item);
        }
    }

    if replies.is_empty() {
        for node in find_all(root, "commentViewModel") {
            if let Some(item) = parse_comment_view_model(node, &mutations_map) {
                replies.push(item);
            }
        }
    }

    for cont in find_all(root, "continuationItemRenderer") {
        if let Some(token) = extract_continuation_token(cont) {
            continuation = Some(token);
            break;
        }
    }

    CommentRepliesResult { replies, continuation }
}

fn parse_single_comment(
    node: &Value,
    mutations: &std::collections::HashMap<String, &Value>,
) -> Option<CommentItem> {
    if let Some(renderer) = node.get("commentRenderer") {
        return parse_comment_renderer(renderer);
    }
    if let Some(vm) = node.get("commentViewModel") {
        return parse_comment_view_model(vm, mutations);
    }
    parse_comment_renderer(node)
}

fn parse_comment_renderer(node: &Value) -> Option<CommentItem> {
    let id = find_first_str(node, "commentId")?;
    let author = runs_text(node.get("authorText"))
        .or_else(|| {
            node.pointer("/authorText/simpleText").and_then(Value::as_str).map(str::to_owned)
        })
        .unwrap_or_else(|| "Anonymous".into());

    let author_avatar = last_thumbnail(node.get("authorThumbnail").unwrap_or(&Value::Null));
    let author_channel_id = node
        .pointer("/authorEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .map(str::to_owned);

    let text = runs_text(node.get("contentText"))
        .or_else(|| {
            node.pointer("/contentText/simpleText").and_then(Value::as_str).map(str::to_owned)
        })
        .unwrap_or_default();

    let published_time = runs_text(node.get("publishedTimeText")).or_else(|| {
        node.pointer("/publishedTimeText/simpleText").and_then(Value::as_str).map(str::to_owned)
    });

    let like_count = runs_text(node.get("voteCount")).or_else(|| {
        node.pointer("/voteCount/simpleText").and_then(Value::as_str).map(str::to_owned)
    });

    let reply_count = node.get("replyCount").and_then(Value::as_i64).unwrap_or(0);

    let is_pinned = node.get("pinnedCommentBadge").is_some();
    let is_channel_owner =
        node.get("authorIsChannelOwner").and_then(Value::as_bool).unwrap_or(false);
    let is_liked = node.get("isLiked").and_then(Value::as_bool).unwrap_or(false);

    let like_ptr = "/actionButtons/commentActionButtonsRenderer/likeButton/\
        toggleButtonRenderer/defaultServiceEndpoint/performCommentActionEndpoint/action";
    let like_action = node.pointer(like_ptr).and_then(Value::as_str).map(str::to_owned);

    let unlike_ptr = "/actionButtons/commentActionButtonsRenderer/likeButton/\
        toggleButtonRenderer/toggledServiceEndpoint/performCommentActionEndpoint/action";
    let unlike_action = node.pointer(unlike_ptr).and_then(Value::as_str).map(str::to_owned);

    let reply_ptr = "/actionButtons/commentActionButtonsRenderer/replyButton/\
        buttonRenderer/serviceEndpoint/createCommentReplyDialogEndpoint/dialog/\
        commentReplyDialogRenderer/replyButton/buttonRenderer/serviceEndpoint/\
        createCommentReplyEndpoint/createCommentReplyParams";
    let reply_action = node.pointer(reply_ptr).and_then(Value::as_str).map(str::to_owned);

    Some(CommentItem {
        id,
        author,
        author_avatar,
        author_channel_id,
        text,
        published_time,
        like_count,
        reply_count,
        is_pinned,
        is_channel_owner,
        is_liked,
        replies_token: None,
        like_action,
        unlike_action,
        reply_action,
    })
}

fn parse_comment_view_model(
    node: &Value,
    mutations: &std::collections::HashMap<String, &Value>,
) -> Option<CommentItem> {
    let key = node.get("commentKey").and_then(Value::as_str);
    let payload = key.and_then(|k| mutations.get(k).copied());

    let id = node
        .get("commentId")
        .or_else(|| key.map(|_| node.get("commentKey").unwrap()))
        .and_then(Value::as_str)?
        .to_owned();

    let mut author = node
        .pointer("/author/displayName")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "Anonymous".into());

    let mut text = node
        .pointer("/content/content")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_default();

    let mut published_time =
        node.pointer("/publishedTime/text").and_then(Value::as_str).map(str::to_owned);

    let like_count =
        node.pointer("/toolbar/likeCountNotliked").and_then(Value::as_str).map(str::to_owned);

    let mut reply_count = node
        .pointer("/toolbar/replyCount")
        .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
        .unwrap_or(0);

    let mut author_avatar =
        node.pointer("/author/avatarThumbnailUrl").and_then(Value::as_str).map(str::to_owned);

    let author_channel_id =
        node.pointer("/author/channelId").and_then(Value::as_str).map(str::to_owned);

    if let Some(p) = payload {
        if let Some(props) = p.get("properties") {
            if let Some(t) = props.pointer("/content/content").and_then(Value::as_str) {
                text = t.to_owned();
            }
            if let Some(a) = props.get("author").and_then(Value::as_str) {
                author = a.to_owned();
            }
            if let Some(pt) = props.get("publishedTimeText").and_then(Value::as_str) {
                published_time = Some(pt.to_owned());
            }
            if let Some(rc) = props.get("replyCount").and_then(Value::as_i64) {
                reply_count = rc;
            }
        }
        if let Some(av) = p.pointer("/author/avatarThumbnailUrl").and_then(Value::as_str) {
            author_avatar = Some(av.to_owned());
        }
    }

    if author_avatar.is_none() {
        author_avatar = last_thumbnail(node);
    }

    Some(CommentItem {
        id,
        author,
        author_avatar,
        author_channel_id,
        text,
        published_time,
        like_count,
        reply_count,
        is_pinned: false,
        is_channel_owner: false,
        is_liked: false,
        replies_token: None,
        like_action: None,
        unlike_action: None,
        reply_action: None,
    })
}

fn extract_continuation_token(node: &Value) -> Option<String> {
    find_first_str(node, "token").or_else(|| find_first_str(node, "continuation"))
}
