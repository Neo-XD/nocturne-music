//! The pairing handshake, here so it tests without libmpv or a webview.
//!
//! Frames arrive as `Option<String>`: `Some` is a text frame, `None` a frame of any other kind that
//! the handshake skips. A transport error ends the stream, which reads as an abandoned handshake.

use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::pairing::{constant_time_eq, expected_pin_hash};
use crate::{RoomState, Track};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncWireMessage {
    AuthChallenge { nonce: String, host_device_name: String },
    AuthResponse { client_device_name: String, pin_hash: Option<String> },
    AuthResult { success: bool, session_token: Option<String>, message: Option<String> },
    SyncState { state: RoomState },
    PlaybackAction { action: RemotePlaybackAction },
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePlaybackAction {
    pub kind: String,
    #[serde(default)]
    pub position_ms: i64,
    #[serde(default)]
    pub track: Option<Track>,
    #[serde(default)]
    pub queue: Option<Vec<Track>>,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub volume: f64,
}

/// How one handshake ended. `WrongPin` is the only outcome that may cost the peer a guess.
#[derive(Debug, PartialEq, Eq)]
pub enum HandshakeOutcome {
    Authenticated { client_device_name: String },
    WrongPin,
    Aborted,
}

/// Sends the challenge, then verifies the first `AuthResponse` and answers with an `AuthResult`.
///
/// `current_pin` is `FnOnce` on purpose: the attempt limiter is only per-attempt because one
/// admitted handshake evaluates one PIN, and a second evaluation here would fail to compile.
pub async fn run_handshake<S, R, F, Fut>(
    receiver: &mut R,
    sender: &mut S,
    nonce: &str,
    host_device_name: &str,
    current_pin: F,
    session_token: &str,
) -> HandshakeOutcome
where
    S: Sink<String> + Unpin,
    R: Stream<Item = Option<String>> + Unpin,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = String>,
{
    let challenge = SyncWireMessage::AuthChallenge {
        nonce: nonce.to_string(),
        host_device_name: host_device_name.to_string(),
    };
    match serde_json::to_string(&challenge) {
        Ok(json) => {
            if sender.send(json).await.is_err() {
                return HandshakeOutcome::Aborted;
            }
        }
        Err(_) => return HandshakeOutcome::Aborted,
    }

    while let Some(frame) = receiver.next().await {
        // Anything that is not an AuthResponse is skipped without spending the one evaluation, which
        // is why a Ping or a playback_action from an unauthenticated peer neither authenticates nor
        // costs it a guess.
        let Some(text) = frame else { continue };
        let Ok(SyncWireMessage::AuthResponse { client_device_name, pin_hash }) =
            serde_json::from_str::<SyncWireMessage>(&text)
        else {
            continue;
        };

        // Read here rather than at connect time, so a PIN regenerated mid-handshake is the one
        // this attempt is judged against, exactly as the inline version did.
        let pin = current_pin().await;
        let ok = !pin.is_empty()
            && pin_hash.is_some_and(|h| constant_time_eq(&h, &expected_pin_hash(nonce, &pin)));
        let reply = SyncWireMessage::AuthResult {
            success: ok,
            session_token: ok.then(|| session_token.to_string()),
            message: (!ok).then(|| "Incorrect pairing PIN".to_string()),
        };
        if let Ok(json) = serde_json::to_string(&reply) {
            let _ = sender.send(json).await;
        }
        return if ok {
            HandshakeOutcome::Authenticated { client_device_name }
        } else {
            HandshakeOutcome::WrongPin
        };
    }
    HandshakeOutcome::Aborted
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    type Sent = Arc<Mutex<Vec<String>>>;

    fn recorder() -> (Sent, impl Sink<String, Error = std::convert::Infallible> + Unpin) {
        let sent: Sent = Arc::new(Mutex::new(Vec::new()));
        let sink = futures_util::sink::unfold(sent.clone(), |log: Sent, msg: String| async move {
            log.lock().unwrap().push(msg);
            Ok::<_, std::convert::Infallible>(log)
        });
        (sent, Box::pin(sink))
    }

    fn text(msg: &SyncWireMessage) -> Option<String> {
        Some(serde_json::to_string(msg).unwrap())
    }

    fn auth_response(name: &str, pin_hash: Option<&str>) -> Option<String> {
        text(&SyncWireMessage::AuthResponse {
            client_device_name: name.to_string(),
            pin_hash: pin_hash.map(str::to_string),
        })
    }

    fn kinds(sent: &Sent) -> Vec<String> {
        sent.lock()
            .unwrap()
            .iter()
            .map(|s| {
                serde_json::from_str::<serde_json::Value>(s).unwrap()["type"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    const PIN: &str = "559691";
    const NONCE: &str = "fd63a2173d04fdf561260a32627c38c6";

    /// The load-bearing one: the whole attempt limit rests on a handshake costing one guess.
    #[tokio::test]
    async fn one_admitted_handshake_evaluates_only_one_pin() {
        let reads = Arc::new(AtomicUsize::new(0));
        let counted = reads.clone();
        let (sent, mut tx) = recorder();
        let mut rx = stream::iter(vec![
            auth_response("phone", Some("00")),
            auth_response("phone", Some(&expected_pin_hash(NONCE, PIN))),
        ]);

        let outcome = run_handshake(
            &mut rx,
            &mut tx,
            NONCE,
            "host",
            || async move {
                counted.fetch_add(1, Ordering::SeqCst);
                PIN.to_string()
            },
            "token",
        )
        .await;

        assert_eq!(outcome, HandshakeOutcome::WrongPin, "the second proof must never be read");
        assert_eq!(reads.load(Ordering::SeqCst), 1, "the PIN must be evaluated exactly once");
        assert_eq!(
            kinds(&sent),
            vec!["auth_challenge", "auth_result"],
            "exactly one verdict may be sent per handshake"
        );
    }

    #[tokio::test]
    async fn a_correct_proof_authenticates() {
        let (sent, mut tx) = recorder();
        let mut rx =
            stream::iter(vec![auth_response("phone", Some(&expected_pin_hash(NONCE, PIN)))]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(
            outcome,
            HandshakeOutcome::Authenticated { client_device_name: "phone".to_string() }
        );
        let last = sent.lock().unwrap().last().unwrap().clone();
        let v: serde_json::Value = serde_json::from_str(&last).unwrap();
        assert_eq!(v["success"], true);
        assert_eq!(v["session_token"], "tok", "a session token is issued only on success");
    }

    #[tokio::test]
    async fn a_wrong_proof_does_not_authenticate() {
        let (sent, mut tx) = recorder();
        let mut rx =
            stream::iter(vec![auth_response("phone", Some(&expected_pin_hash(NONCE, "000000")))]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(outcome, HandshakeOutcome::WrongPin);
        let last = sent.lock().unwrap().last().unwrap().clone();
        let v: serde_json::Value = serde_json::from_str(&last).unwrap();
        assert_eq!(v["success"], false);
        assert!(v["session_token"].is_null(), "a refused handshake must not carry a token");
    }

    #[tokio::test]
    async fn a_missing_pin_hash_does_not_authenticate() {
        let (_sent, mut tx) = recorder();
        let mut rx = stream::iter(vec![auth_response("phone", None)]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(outcome, HandshakeOutcome::WrongPin);
    }

    /// An empty stored PIN must refuse everyone, which is what a profile that never generated one
    /// used to hold.
    #[tokio::test]
    async fn an_empty_stored_pin_authenticates_nobody() {
        let (_sent, mut tx) = recorder();
        let mut rx =
            stream::iter(vec![auth_response("phone", Some(&expected_pin_hash(NONCE, "")))]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { String::new() }, "tok").await;
        assert_eq!(outcome, HandshakeOutcome::WrongPin);
    }

    #[tokio::test]
    async fn an_unauthenticated_peer_gets_no_state_and_its_actions_are_ignored() {
        let (sent, mut tx) = recorder();
        let mut rx = stream::iter(vec![
            text(&SyncWireMessage::PlaybackAction {
                action: RemotePlaybackAction {
                    kind: "play".to_string(),
                    position_ms: 0,
                    track: None,
                    queue: None,
                    playing: true,
                    volume: 1.0,
                },
            }),
            text(&SyncWireMessage::Ping),
        ]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(outcome, HandshakeOutcome::Aborted, "actions must not authenticate anyone");
        assert_eq!(
            kinds(&sent),
            vec!["auth_challenge"],
            "no sync_state and no verdict may reach an unauthenticated peer"
        );
    }

    /// A stalled peer must cost the address nothing; only a wrong PIN may.
    #[tokio::test]
    async fn a_handshake_that_never_answers_times_out_without_counting_a_wrong_pin() {
        let (sent, mut tx) = recorder();
        let mut rx = stream::pending::<Option<String>>();
        let elapsed = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok"),
        )
        .await;
        assert!(elapsed.is_err(), "a peer that never answers must be cut off by the timeout");
        assert_eq!(kinds(&sent), vec!["auth_challenge"], "no verdict is reached on a timeout");
    }

    #[tokio::test]
    async fn junk_before_the_response_is_skipped_without_spending_the_attempt() {
        let (sent, mut tx) = recorder();
        let mut rx = stream::iter(vec![
            Some("not json at all".to_string()),
            Some(r#"{"type":"nonexistent_kind"}"#.to_string()),
            Some("{}".to_string()),
            None,
            text(&SyncWireMessage::Pong),
            auth_response("phone", Some(&expected_pin_hash(NONCE, PIN))),
        ]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(
            outcome,
            HandshakeOutcome::Authenticated { client_device_name: "phone".to_string() },
            "malformed frames must neither authenticate nor consume the one evaluation"
        );
        assert_eq!(kinds(&sent), vec!["auth_challenge", "auth_result"]);
    }

    #[tokio::test]
    async fn junk_alone_never_authenticates() {
        let (_sent, mut tx) = recorder();
        let mut rx = stream::iter(vec![Some("<html>".to_string()), None]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(outcome, HandshakeOutcome::Aborted);
    }

    #[tokio::test]
    async fn the_challenge_is_sent_before_anything_is_read() {
        let (sent, mut tx) = recorder();
        let mut rx = stream::iter(Vec::<Option<String>>::new());
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "the-host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(outcome, HandshakeOutcome::Aborted);
        let first = sent.lock().unwrap().first().unwrap().clone();
        match serde_json::from_str::<SyncWireMessage>(&first).unwrap() {
            SyncWireMessage::AuthChallenge { nonce, host_device_name } => {
                assert_eq!(nonce, NONCE, "the challenge must carry the nonce it was given");
                assert_eq!(host_device_name, "the-host");
            }
            other => panic!("the first frame must be the challenge, got {other:?}"),
        }
    }

    /// The nonce is minted per connection by the caller; this proves the handshake binds the proof
    /// to whichever nonce it was handed, so two connections cannot share a proof.
    #[tokio::test]
    async fn a_proof_is_bound_to_the_nonce_of_its_own_connection() {
        let other_nonce = "c701fb074c09b58f0452b2ef9da6704e";
        assert_ne!(NONCE, other_nonce);
        let (_sent, mut tx) = recorder();
        let mut rx =
            stream::iter(vec![auth_response("phone", Some(&expected_pin_hash(other_nonce, PIN)))]);
        let outcome =
            run_handshake(&mut rx, &mut tx, NONCE, "host", || async { PIN.to_string() }, "tok")
                .await;
        assert_eq!(
            outcome,
            HandshakeOutcome::WrongPin,
            "a proof for another connection's nonce must not authenticate"
        );
    }
}
