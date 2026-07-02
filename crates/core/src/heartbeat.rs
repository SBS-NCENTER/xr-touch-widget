use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum LinkStatus {
    Unknown,
    Connected,
    Lost,
}

struct Entry {
    misses: u32,
    status: LinkStatus,
}

/// Pure state machine — no clocks inside. The engine loop owns timing and
/// calls on_tick once per heartbeat interval.
pub struct Heartbeat {
    timeout_misses: u32,
    entries: HashMap<String, Entry>,
}

impl Heartbeat {
    pub fn new(timeout_misses: u32) -> Self {
        Self { timeout_misses, entries: HashMap::new() }
    }

    pub fn on_tick(&mut self, target_ips: &[String]) {
        self.entries.retain(|ip, _| target_ips.contains(ip));
        let mut seen = HashSet::new();
        for ip in target_ips {
            if seen.insert(ip) {
                let entry = self
                    .entries
                    .entry(ip.clone())
                    .or_insert(Entry { misses: 0, status: LinkStatus::Unknown });
                entry.misses = entry.misses.saturating_add(1);
                if entry.misses >= self.timeout_misses {
                    entry.status = LinkStatus::Lost;
                }
            }
        }
    }

    pub fn on_pong(&mut self, ip: &str) {
        if let Some(entry) = self.entries.get_mut(ip) {
            entry.misses = 0;
            entry.status = LinkStatus::Connected;
        }
    }

    pub fn status(&self, ip: &str) -> LinkStatus {
        self.entries.get(ip).map(|e| e.status).unwrap_or(LinkStatus::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ips(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn starts_unknown() {
        let hb = Heartbeat::new(3);
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Unknown);
    }

    #[test]
    fn three_silent_ticks_mean_lost() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1"]);
        hb.on_tick(&t); // miss 1
        hb.on_tick(&t); // miss 2
        assert_ne!(hb.status("10.0.0.1"), LinkStatus::Lost, "not lost yet at 2 misses");
        hb.on_tick(&t); // miss 3 -> Lost
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Lost);
    }

    #[test]
    fn pong_connects_and_resets_misses() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1"]);
        hb.on_tick(&t);
        hb.on_tick(&t);
        hb.on_pong("10.0.0.1");
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Connected);
        hb.on_tick(&t); // miss count restarts from here
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Connected);
    }

    #[test]
    fn lost_target_recovers_on_pong() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1"]);
        for _ in 0..3 {
            hb.on_tick(&t);
        }
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Lost);
        hb.on_pong("10.0.0.1");
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Connected);
    }

    #[test]
    fn removed_target_is_forgotten() {
        let mut hb = Heartbeat::new(3);
        hb.on_tick(&ips(&["10.0.0.1"]));
        hb.on_pong("10.0.0.1");
        hb.on_tick(&ips(&[])); // target removed from config
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Unknown);
    }

    #[test]
    fn pong_from_unregistered_ip_is_ignored() {
        let mut hb = Heartbeat::new(3);
        hb.on_pong("99.9.9.9"); // never ticked -> not tracked
        assert_eq!(hb.status("99.9.9.9"), LinkStatus::Unknown);
    }

    #[test]
    fn duplicate_ips_count_one_miss_per_tick() {
        let mut hb = Heartbeat::new(3);
        let t = ips(&["10.0.0.1", "10.0.0.1"]); // same ip twice
        hb.on_tick(&t);
        hb.on_tick(&t);
        assert_ne!(hb.status("10.0.0.1"), LinkStatus::Lost, "2 ticks must not reach Lost");
        hb.on_tick(&t);
        assert_eq!(hb.status("10.0.0.1"), LinkStatus::Lost);
    }
}
