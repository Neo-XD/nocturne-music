//! PIN pairing and handshake admission, here so it tests without libmpv or a webview.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Wrong PINs from one address before it is locked out.
pub const MAX_AUTH_FAILURES: u32 = 5;
/// How long a lockout lasts, and how long failures are remembered.
pub const AUTH_LOCKOUT: Duration = Duration::from_secs(900);
/// Ceiling on remembered addresses, so a peer rotating source addresses cannot grow the map without bound.
pub const MAX_TRACKED_ADDRESSES: usize = 1024;
/// Unauthenticated handshakes allowed to occupy the server at once.
pub const MAX_HANDSHAKES_IN_FLIGHT: usize = 4;
/// Of those, how many one address may hold, so it cannot occupy every slot.
pub const MAX_HANDSHAKES_PER_ADDRESS: usize = 2;

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

/// Guesses charged per address, reachable only through `PairingGate`, which charges under one lock.
#[derive(Debug, Default)]
struct AttemptLimiter {
    failures: HashMap<IpAddr, (u32, Instant)>,
}

impl AttemptLimiter {
    /// Charges one guess to this address, or refuses because its allowance is spent.
    fn try_reserve(&mut self, ip: IpAddr, now: Instant) -> bool {
        if self.failures.get(&ip).is_some_and(|(_, at)| now.duration_since(*at) >= AUTH_LOCKOUT) {
            self.failures.remove(&ip);
        }
        if let Some(e) = self.failures.get_mut(&ip) {
            if e.0 >= MAX_AUTH_FAILURES {
                return false;
            }
            e.0 += 1;
            e.1 = now;
            return true;
        }
        self.make_room(now);
        self.failures.insert(ip, (1, now));
        true
    }

    /// Frees a charge whose handshake never produced an answer.
    fn refund(&mut self, ip: IpAddr) {
        if let Some(e) = self.failures.get_mut(&ip) {
            e.0 = e.0.saturating_sub(1);
            if e.0 == 0 {
                self.failures.remove(&ip);
            }
        }
    }

    /// Evicts oldest-first so a full map still tracks a new address instead of failing open.
    fn make_room(&mut self, now: Instant) {
        if self.failures.len() < MAX_TRACKED_ADDRESSES {
            return;
        }
        self.failures.retain(|_, (_, at)| now.duration_since(*at) < AUTH_LOCKOUT);
        while self.failures.len() >= MAX_TRACKED_ADDRESSES {
            let Some(oldest) =
                self.failures.iter().min_by_key(|(_, (_, at))| *at).map(|(ip, _)| *ip)
            else {
                return;
            };
            self.failures.remove(&oldest);
        }
    }

    fn record_success(&mut self, ip: IpAddr) {
        self.failures.remove(&ip);
    }

    fn tracked(&self) -> usize {
        self.failures.len()
    }
}

/// Why a handshake was turned away, which decides what the client is told.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    LockedOut,
    Busy,
}

/// How a handshake ended; the default covers a client that went away mid-handshake.
#[derive(Debug, Default)]
enum Settlement {
    #[default]
    Abandoned,
    WrongPin,
    Success,
}

#[derive(Debug, Default)]
struct GateState {
    limiter: AttemptLimiter,
    in_flight: HashMap<IpAddr, usize>,
    total_in_flight: usize,
}

/// Admission control: a guess is charged before the challenge goes out, not after the answer.
#[derive(Debug, Default)]
pub struct PairingGate {
    state: Mutex<GateState>,
}

impl PairingGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Admits one handshake or refuses it outright; queueing here would deny legitimate pairing.
    pub fn admit(self: &Arc<Self>, ip: IpAddr, now: Instant) -> Result<Ticket, Refusal> {
        let mut s = self.state.lock().unwrap();
        if s.total_in_flight >= MAX_HANDSHAKES_IN_FLIGHT {
            return Err(Refusal::Busy);
        }
        if s.in_flight.get(&ip).is_some_and(|n| *n >= MAX_HANDSHAKES_PER_ADDRESS) {
            return Err(Refusal::Busy);
        }
        if !s.limiter.try_reserve(ip, now) {
            return Err(Refusal::LockedOut);
        }
        s.total_in_flight += 1;
        *s.in_flight.entry(ip).or_insert(0) += 1;
        Ok(Ticket { gate: Arc::clone(self), ip, settlement: Settlement::Abandoned })
    }

    pub fn tracked(&self) -> usize {
        self.state.lock().unwrap().limiter.tracked()
    }

    pub fn in_flight(&self) -> usize {
        self.state.lock().unwrap().total_in_flight
    }
}

/// One admitted handshake, holding the guess it was charged until it says how the handshake ended.
pub struct Ticket {
    gate: Arc<PairingGate>,
    ip: IpAddr,
    settlement: Settlement,
}

impl Ticket {
    pub fn succeeded(mut self) {
        self.settlement = Settlement::Success;
    }

    pub fn wrong_pin(mut self) {
        self.settlement = Settlement::WrongPin;
    }
}

impl Drop for Ticket {
    /// Settling here is what keeps a dropped or timed-out socket from counting as a wrong PIN.
    fn drop(&mut self) {
        let mut s = self.gate.state.lock().unwrap();
        s.total_in_flight = s.total_in_flight.saturating_sub(1);
        if let Some(n) = s.in_flight.get_mut(&self.ip) {
            *n -= 1;
            if *n == 0 {
                s.in_flight.remove(&self.ip);
            }
        }
        match self.settlement {
            Settlement::Success => s.limiter.record_success(self.ip),
            Settlement::WrongPin => {}
            Settlement::Abandoned => s.limiter.refund(self.ip),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::sync::Barrier;

    fn ip(n: u8) -> IpAddr {
        IpAddr::from([10, 0, 0, n])
    }

    /// One wave of sockets that all reach `admit` before any answers, as a real burst does.
    async fn wrong_pin_wave(
        gate: &Arc<PairingGate>,
        addr: IpAddr,
        sockets: usize,
        now: Instant,
    ) -> u32 {
        let evaluated = Arc::new(AtomicU32::new(0));
        let barrier = Arc::new(Barrier::new(sockets));
        let mut tasks = Vec::with_capacity(sockets);
        for _ in 0..sockets {
            let (gate, evaluated, barrier) =
                (Arc::clone(gate), Arc::clone(&evaluated), Arc::clone(&barrier));
            tasks.push(tokio::spawn(async move {
                let admitted = gate.admit(addr, now).ok();
                barrier.wait().await;
                if let Some(ticket) = admitted {
                    evaluated.fetch_add(1, Ordering::SeqCst);
                    ticket.wrong_pin();
                }
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
        evaluated.load(Ordering::SeqCst)
    }

    /// Sends waves until one is refused outright, totalling the guesses that were evaluated.
    async fn guesses_until_locked_out(gate: &Arc<PairingGate>, addr: IpAddr, now: Instant) -> u32 {
        let mut evaluated = 0;
        for _ in 0..20 {
            let wave = wrong_pin_wave(gate, addr, 64, now).await;
            if wave == 0 {
                return evaluated;
            }
            evaluated += wave;
        }
        evaluated
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
        let gate = PairingGate::new();
        let now = Instant::now();
        for _ in 0..MAX_AUTH_FAILURES {
            gate.admit(ip(1), now).expect("below the cap the address may try").wrong_pin();
        }
        assert_eq!(gate.admit(ip(1), now).err(), Some(Refusal::LockedOut));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn a_concurrent_burst_from_one_address_cannot_exceed_the_cap() {
        let gate = PairingGate::new();
        let now = Instant::now();
        let evaluated = guesses_until_locked_out(&gate, ip(1), now).await;
        assert_eq!(
            evaluated, MAX_AUTH_FAILURES,
            "a concurrent burst got more guesses than the cap"
        );
        assert_eq!(gate.admit(ip(1), now).err(), Some(Refusal::LockedOut));
        assert_eq!(gate.in_flight(), 0, "every settled handshake must release its slot");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn a_full_attempt_map_still_limits_a_fresh_address() {
        let gate = PairingGate::new();
        let now = Instant::now();
        for n in 0..MAX_TRACKED_ADDRESSES as u128 {
            gate.admit(IpAddr::from(Ipv6Addr::from(n)), now).unwrap().wrong_pin();
        }
        assert_eq!(gate.tracked(), MAX_TRACKED_ADDRESSES, "the map should now be at its ceiling");
        let evaluated = guesses_until_locked_out(&gate, ip(9), now).await;
        assert_eq!(evaluated, MAX_AUTH_FAILURES, "a full map let a fresh address exceed the cap");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn an_abandoned_handshake_is_not_a_wrong_pin() {
        let gate = PairingGate::new();
        let now = Instant::now();
        for _ in 0..100 {
            drop(gate.admit(ip(1), now).expect("an abandoned handshake must not lock the address"));
        }
        assert_eq!(gate.tracked(), 0, "an abandoned handshake must leave no charge behind");
        let evaluated = guesses_until_locked_out(&gate, ip(1), now).await;
        assert_eq!(evaluated, MAX_AUTH_FAILURES, "aborts must leave the full allowance intact");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn aborts_neither_consume_nor_refund_a_real_wrong_pin() {
        let gate = PairingGate::new();
        let now = Instant::now();
        for _ in 0..MAX_AUTH_FAILURES - 1 {
            gate.admit(ip(1), now).unwrap().wrong_pin();
        }
        for _ in 0..100 {
            drop(gate.admit(ip(1), now).expect("one guess is still owed"));
        }
        let evaluated = guesses_until_locked_out(&gate, ip(1), now).await;
        assert_eq!(
            evaluated, 1,
            "four wrong PINs plus any number of aborts leave exactly one guess"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn a_second_address_pairs_while_the_first_is_mid_handshake() {
        let gate = PairingGate::new();
        let now = Instant::now();
        let held: Vec<Ticket> = (0..64).filter_map(|_| gate.admit(ip(1), now).ok()).collect();
        assert_eq!(
            held.len(),
            MAX_HANDSHAKES_PER_ADDRESS,
            "one address occupied more handshake slots than it may hold"
        );
        gate.admit(ip(2), now).expect("a busy address must not shut another one out").succeeded();
        assert_eq!(
            gate.in_flight(),
            held.len(),
            "a paired device kept holding a handshake slot after pairing"
        );
    }

    #[test]
    fn no_more_handshakes_run_at_once_than_the_global_cap() {
        let gate = PairingGate::new();
        let now = Instant::now();
        let held: Vec<Ticket> = (0..64).filter_map(|n| gate.admit(ip(n), now).ok()).collect();
        assert_eq!(
            held.len(),
            MAX_HANDSHAKES_IN_FLIGHT,
            "more handshakes ran at once than the cap allows"
        );
        // Busy rather than LockedOut, or a full desktop reads to the user as a mistyped PIN.
        assert_eq!(gate.admit(ip(200), now).err(), Some(Refusal::Busy));
        drop(held);
        assert_eq!(gate.in_flight(), 0, "released handshakes must free their slots");
        assert!(gate.admit(ip(200), now).is_ok(), "a freed slot must be reusable");
    }

    #[test]
    fn lockout_expires() {
        let gate = PairingGate::new();
        let now = Instant::now();
        for _ in 0..MAX_AUTH_FAILURES {
            gate.admit(ip(1), now).unwrap().wrong_pin();
        }
        assert_eq!(gate.admit(ip(1), now).err(), Some(Refusal::LockedOut));
        assert!(gate.admit(ip(1), now + AUTH_LOCKOUT).is_ok());
    }

    #[test]
    fn success_forgives_earlier_failures() {
        let gate = PairingGate::new();
        let now = Instant::now();
        gate.admit(ip(1), now).unwrap().wrong_pin();
        gate.admit(ip(1), now).unwrap().wrong_pin();
        gate.admit(ip(1), now).unwrap().succeeded();
        assert_eq!(gate.tracked(), 0);
        assert!(gate.admit(ip(1), now).is_ok());
    }

    #[test]
    fn one_address_cannot_lock_out_another() {
        let gate = PairingGate::new();
        let now = Instant::now();
        for _ in 0..MAX_AUTH_FAILURES {
            gate.admit(ip(1), now).unwrap().wrong_pin();
        }
        assert_eq!(gate.admit(ip(1), now).err(), Some(Refusal::LockedOut));
        assert!(gate.admit(ip(2), now).is_ok());
    }

    #[test]
    fn tracked_addresses_stay_at_the_ceiling_and_keep_the_newest() {
        let gate = PairingGate::new();
        let now = Instant::now();
        let mut newest = ip(1);
        for n in 0..MAX_TRACKED_ADDRESSES as u128 * 2 {
            newest = IpAddr::from(Ipv6Addr::from(n));
            gate.admit(newest, now).unwrap().wrong_pin();
            assert!(gate.tracked() <= MAX_TRACKED_ADDRESSES, "the map exceeded its ceiling");
        }
        assert_eq!(
            gate.tracked(),
            MAX_TRACKED_ADDRESSES,
            "eviction emptied the map instead of rotating it"
        );
        for _ in 0..MAX_AUTH_FAILURES - 1 {
            gate.admit(newest, now).unwrap().wrong_pin();
        }
        assert_eq!(
            gate.admit(newest, now).err(),
            Some(Refusal::LockedOut),
            "the address that just failed was not remembered"
        );
    }
}
