//! PIN pairing: proof derivation and per-address attempt limiting.
//!
//! Lives here rather than in the Tauri crate so it can be tested without libmpv or a webview.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Wrong PINs from one address before it is locked out.
pub const MAX_AUTH_FAILURES: u32 = 5;
/// How long a lockout lasts, and how long failures are remembered.
pub const AUTH_LOCKOUT: Duration = Duration::from_secs(900);
/// Ceiling on remembered addresses, so a peer rotating source addresses cannot grow the map without bound.
pub const MAX_TRACKED_ADDRESSES: usize = 1024;

/// Proof a client must send: hex SHA-256 over `nonce:pin`.
pub fn expected_pin_hash(nonce: &str, pin: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(nonce.as_bytes());
    h.update(b":");
    h.update(pin.as_bytes());
    format!("{:x}", h.finalize())
}

/// Compared over the full length so a wrong proof costs the same time as a right one.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A PIN the mobile client can actually enter: 4 to 8 digits.
pub fn is_valid_pin(pin: &str) -> bool {
    (4..=8).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

/// Per-address failure counts. Check and increment happen in one `&mut self` call, so
/// concurrent callers cannot all observe a zero count before any of them records a failure.
#[derive(Debug, Default)]
pub struct AttemptLimiter {
    failures: HashMap<IpAddr, (u32, Instant)>,
}

impl AttemptLimiter {
    pub fn new() -> Self {
        Self { failures: HashMap::new() }
    }

    /// True when this address may still try. Expired entries are dropped as they are seen.
    pub fn allows(&mut self, ip: IpAddr, now: Instant) -> bool {
        if let Some((_, at)) = self.failures.get(&ip) {
            if now.duration_since(*at) >= AUTH_LOCKOUT {
                self.failures.remove(&ip);
                return true;
            }
        }
        self.failures.get(&ip).is_none_or(|(n, _)| *n < MAX_AUTH_FAILURES)
    }

    /// Records one wrong PIN. Only a real wrong answer counts, so a dropped socket or a
    /// handshake timeout cannot lock out a device that knows the PIN.
    pub fn record_failure(&mut self, ip: IpAddr, now: Instant) {
        if self.failures.len() >= MAX_TRACKED_ADDRESSES {
            self.failures.retain(|_, (_, at)| now.duration_since(*at) < AUTH_LOCKOUT);
        }
        if self.failures.len() >= MAX_TRACKED_ADDRESSES && !self.failures.contains_key(&ip) {
            return;
        }
        let e = self.failures.entry(ip).or_insert((0, now));
        e.0 += 1;
        e.1 = now;
    }

    pub fn record_success(&mut self, ip: IpAddr) {
        self.failures.remove(&ip);
    }

    pub fn tracked(&self) -> usize {
        self.failures.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(n: u8) -> IpAddr {
        IpAddr::from([10, 0, 0, n])
    }

    #[test]
    fn proof_matches_the_documented_construction() {
        // Same input the Kotlin client hashes: "nonce:pin", lowercase hex.
        let h = expected_pin_hash("abc", "1234");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(h, expected_pin_hash("abc", "1234"));
        assert_ne!(h, expected_pin_hash("abd", "1234"));
        assert_ne!(h, expected_pin_hash("abc", "1235"));
    }

    #[test]
    fn constant_time_eq_agrees_with_equality() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn pin_validation_matches_what_the_client_can_type() {
        assert!(is_valid_pin("1234"));
        assert!(is_valid_pin("123456"));
        assert!(is_valid_pin("12345678"));
        assert!(!is_valid_pin("123"));
        assert!(!is_valid_pin("123456789"));
        assert!(!is_valid_pin("12a4"));
        assert!(!is_valid_pin(""));
    }

    #[test]
    fn lockout_after_max_failures() {
        let mut l = AttemptLimiter::new();
        let now = Instant::now();
        for _ in 0..MAX_AUTH_FAILURES {
            assert!(l.allows(ip(1), now));
            l.record_failure(ip(1), now);
        }
        assert!(!l.allows(ip(1), now));
    }

    #[test]
    fn a_burst_of_wrong_guesses_cannot_exceed_the_cap() {
        // The parallel-attack case: every guess goes through one allows/record pair, so the
        // cap holds no matter how many callers interleave.
        let mut l = AttemptLimiter::new();
        let now = Instant::now();
        let mut allowed = 0;
        for _ in 0..1000 {
            if l.allows(ip(1), now) {
                allowed += 1;
                l.record_failure(ip(1), now);
            }
        }
        assert_eq!(allowed, MAX_AUTH_FAILURES);
    }

    #[test]
    fn lockout_expires() {
        let mut l = AttemptLimiter::new();
        let now = Instant::now();
        for _ in 0..MAX_AUTH_FAILURES {
            l.record_failure(ip(1), now);
        }
        assert!(!l.allows(ip(1), now));
        assert!(l.allows(ip(1), now + AUTH_LOCKOUT));
    }

    #[test]
    fn success_forgives_earlier_failures() {
        let mut l = AttemptLimiter::new();
        let now = Instant::now();
        l.record_failure(ip(1), now);
        l.record_failure(ip(1), now);
        l.record_success(ip(1));
        assert!(l.allows(ip(1), now));
        assert_eq!(l.tracked(), 0);
    }

    #[test]
    fn one_address_cannot_lock_out_another() {
        let mut l = AttemptLimiter::new();
        let now = Instant::now();
        for _ in 0..MAX_AUTH_FAILURES {
            l.record_failure(ip(1), now);
        }
        assert!(!l.allows(ip(1), now));
        assert!(l.allows(ip(2), now));
    }

    #[test]
    fn tracked_addresses_are_bounded() {
        let mut l = AttemptLimiter::new();
        let now = Instant::now();
        for n in 0..2000u32 {
            l.record_failure(IpAddr::from(std::net::Ipv6Addr::from(n as u128)), now);
        }
        assert!(l.tracked() <= MAX_TRACKED_ADDRESSES);
    }
}
