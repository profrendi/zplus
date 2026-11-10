//! Bring a link back as soon as the network does.
//!
//! whatsapp-rust waits between failed connection attempts on WhatsApp Web's
//! Fibonacci schedule: 1, 1, 2, 3, 5, 8 seconds and so on, up to 15 minutes.
//! When ZFPlus starts with Windows the network is often not ready yet, so the
//! first attempts fail fast and the wait grows. By the time the network comes
//! up the client can be asleep for several minutes, showing "Connecting to
//! WhatsApp…" while everything else on the machine is already online. Nothing
//! in the library ends that wait early except a pause and resume.
//!
//! So while a link waits, the worker's tick asks this watch whether to probe
//! the network: a short TCP connection to WhatsApp's host (or through the
//! proxy). When the network answers after it did not, or has answered for a
//! while and the link is still down, the worker connects at once. A link that
//! fails only now and then keeps the library's own schedule: nothing is probed
//! until a few attempts in a row have failed.

use std::time::{Duration, Instant};

/// Failed attempts in a row before the network is probed. The library has
/// waited only a few seconds in total by then.
pub(super) const MIN_FAILURES: u32 = 3;

/// Time between probes while the link waits.
pub(super) const PROBE_EVERY: Duration = Duration::from_secs(10);

/// How long one probe may take.
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Shortest time between two early connections, so a network that flaps does
/// not turn into a connection attempt every probe.
pub(super) const MIN_GAP: Duration = Duration::from_secs(15);

/// Longest the link waits while the network answers. WhatsApp's own schedule
/// may still be at 15 minutes; this keeps a reachable network from waiting
/// more than two.
pub(super) const RETRY_EVERY: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
pub(super) struct LinkNudge {
    probing: bool,
    last_probe: Option<Instant>,
    /// What the previous probe found.
    reachable: Option<bool>,
    last_nudge: Option<Instant>,
}

impl LinkNudge {
    /// Whether to start a probe now. `waiting` says the link is down and the
    /// library is trying to bring it back; `failures` is its count of failed
    /// attempts in a row.
    pub(super) fn should_probe(&mut self, now: Instant, waiting: bool, failures: u32) -> bool {
        if !waiting {
            // A connected link starts the next outage from scratch.
            *self = Self {
                probing: self.probing,
                ..Self::default()
            };
            return false;
        }
        if self.probing || failures < MIN_FAILURES {
            return false;
        }
        if self
            .last_probe
            .is_some_and(|last| now.saturating_duration_since(last) < PROBE_EVERY)
        {
            return false;
        }
        self.probing = true;
        self.last_probe = Some(now);
        true
    }

    /// Records what a probe found, and whether to connect now.
    pub(super) fn probed(&mut self, now: Instant, reachable: bool, waiting: bool) -> bool {
        self.probing = false;
        let came_back = reachable && self.reachable != Some(true);
        self.reachable = Some(reachable);
        if !waiting || !reachable {
            return false;
        }
        let since = self
            .last_nudge
            .map(|last| now.saturating_duration_since(last));
        let due = match since {
            None => true,
            Some(since) => (came_back && since >= MIN_GAP) || since >= RETRY_EVERY,
        };
        if due {
            self.last_nudge = Some(now);
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn nothing_is_probed_while_connected_or_after_one_failure() {
        let mut nudge = LinkNudge::default();
        let now = Instant::now();
        assert!(!nudge.should_probe(now, false, 10));
        assert!(!nudge.should_probe(now, true, MIN_FAILURES - 1));
        assert!(nudge.should_probe(now, true, MIN_FAILURES));
    }

    #[test]
    fn one_probe_at_a_time_and_ten_seconds_apart() {
        let mut nudge = LinkNudge::default();
        let start = Instant::now();
        assert!(nudge.should_probe(start, true, 5));
        assert!(
            !nudge.should_probe(start + 20 * S, true, 5),
            "still probing"
        );
        nudge.probed(start + S, false, true);
        assert!(!nudge.should_probe(start + 5 * S, true, 5));
        assert!(nudge.should_probe(start + PROBE_EVERY, true, 5));
    }

    /// The case from the field: Windows starts ZFPlus before the network is
    /// up, the library's wait grows, and the network comes back while it
    /// sleeps.
    #[test]
    fn the_network_coming_back_connects_at_once() {
        let mut nudge = LinkNudge::default();
        let start = Instant::now();
        let mut now = start;
        for _ in 0..6 {
            assert!(nudge.should_probe(now, true, 8));
            assert!(!nudge.probed(now, false, true), "no network, no attempt");
            now += PROBE_EVERY;
        }
        assert!(nudge.should_probe(now, true, 9));
        assert!(nudge.probed(now, true, true), "the network is back");
    }

    #[test]
    fn a_reachable_network_retries_every_two_minutes_not_every_probe() {
        let mut nudge = LinkNudge::default();
        let start = Instant::now();
        assert!(nudge.probed(start, true, true));
        assert!(!nudge.probed(start + PROBE_EVERY, true, true));
        assert!(!nudge.probed(start + RETRY_EVERY - S, true, true));
        assert!(nudge.probed(start + RETRY_EVERY, true, true));
    }

    #[test]
    fn a_flapping_network_waits_between_attempts() {
        let mut nudge = LinkNudge::default();
        let start = Instant::now();
        assert!(nudge.probed(start, true, true));
        nudge.probed(start + 5 * S, false, true);
        assert!(!nudge.probed(start + 10 * S, true, true), "too soon");
        nudge.probed(start + 12 * S, false, true);
        assert!(nudge.probed(start + MIN_GAP, true, true));
    }

    #[test]
    fn a_link_that_came_up_meanwhile_is_left_alone() {
        let mut nudge = LinkNudge::default();
        let now = Instant::now();
        assert!(nudge.should_probe(now, true, 5));
        assert!(!nudge.probed(now, true, false));
    }

    #[test]
    fn connecting_starts_the_next_outage_fresh() {
        let mut nudge = LinkNudge::default();
        let start = Instant::now();
        assert!(nudge.probed(start, true, true));
        nudge.should_probe(start + S, false, 0);
        assert!(nudge.should_probe(start + 2 * S, true, 5));
        assert!(nudge.probed(start + 3 * S, true, true), "a new outage");
    }
}
