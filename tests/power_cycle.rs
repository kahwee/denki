#[path = "support/power_cycle.rs"]
mod power_cycle;
use anyhow::{Result, bail};
use power_cycle::{Plug, State, round_trip, wait_for};
use std::time::Duration;

#[cfg(test)]
mod offline {
    use super::*;
    use std::collections::VecDeque;

    struct Fake {
        on: bool,
        writes: Vec<bool>,
        reads: VecDeque<Result<State>>,
        fail_write: Option<usize>,
        auth_failure: bool,
        read_count: usize,
        apply_before_error: bool,
    }
    impl Fake {
        fn new(on: bool) -> Self {
            Self {
                on,
                writes: vec![],
                reads: VecDeque::new(),
                fail_write: None,
                auth_failure: false,
                read_count: 0,
                apply_before_error: false,
            }
        }
    }
    impl Plug for Fake {
        async fn read(&mut self) -> Result<State> {
            self.read_count += 1;
            self.reads.pop_front().unwrap_or_else(|| {
                Ok(State {
                    id: "fixture".into(),
                    on: self.on,
                })
            })
        }
        async fn set(&mut self, on: bool) -> Result<()> {
            self.writes.push(on);
            if self.fail_write == Some(self.writes.len()) {
                if self.apply_before_error {
                    self.on = on;
                }
                if self.auth_failure {
                    return Err(denki::error::error(
                        "tpap_auth_attempts_limit",
                        "login locked",
                    ));
                }
                bail!("simulated write failure");
            }
            self.on = on;
            Ok(())
        }
    }

    #[tokio::test]
    async fn verifies_both_states_and_preserves_initial_state() {
        for on in [false, true] {
            let mut plug = Fake::new(on);
            round_trip(&mut plug, Duration::ZERO).await.unwrap();
            assert_eq!(plug.writes, vec![!on, on]);
            assert_eq!(plug.on, on);
        }
    }

    #[tokio::test]
    async fn restores_after_failed_return_write_but_still_fails_test() {
        let mut plug = Fake::new(false);
        plug.fail_write = Some(2);
        let error = round_trip(&mut plug, Duration::ZERO).await.unwrap_err();
        assert!(error.to_string().contains("Original state restored"));
        assert!(!plug.on);
        assert_eq!(plug.writes, vec![true, false, false]);
    }

    #[tokio::test]
    async fn never_writes_after_initial_read_failure() {
        let mut plug = Fake::new(false);
        plug.reads.push_back(Err(anyhow::anyhow!("offline")));
        assert!(round_trip(&mut plug, Duration::ZERO).await.is_err());
        assert!(plug.writes.is_empty());
    }

    #[tokio::test]
    async fn auth_failure_stops_without_cleanup_login() {
        let mut plug = Fake::new(false);
        plug.fail_write = Some(1);
        plug.auth_failure = true;
        let error = round_trip(&mut plug, Duration::ZERO).await.unwrap_err();
        assert!(error.to_string().contains("Restoration skipped"));
        assert_eq!(plug.writes, vec![true]);
        assert_eq!(plug.read_count, 1);
    }

    #[tokio::test]
    async fn identity_change_prevents_cleanup_write() {
        let mut plug = Fake::new(false);
        for id in ["fixture", "replacement", "replacement"] {
            plug.reads.push_back(Ok(State {
                id: id.into(),
                on: false,
            }));
        }
        let error = round_trip(&mut plug, Duration::ZERO).await.unwrap_err();
        assert!(error.to_string().contains("identity changed"));
        assert_eq!(plug.writes, vec![true]);
    }

    #[tokio::test]
    async fn polling_is_bounded() {
        let mut plug = Fake::new(false);
        let original = State {
            id: "fixture".into(),
            on: false,
        };
        assert!(
            wait_for(&mut plug, &original, true, Duration::ZERO)
                .await
                .is_err()
        );
        assert!(plug.writes.is_empty());
        assert_eq!(plug.read_count, 3);
    }
    #[tokio::test]
    async fn uncertain_write_is_read_before_restoration() {
        let mut plug = Fake::new(false);
        plug.fail_write = Some(1);
        plug.apply_before_error = true;
        let error = round_trip(&mut plug, Duration::ZERO).await.unwrap_err();
        assert!(error.to_string().contains("Original state restored"));
        assert_eq!(plug.writes, vec![true, false]);
        assert_eq!(plug.read_count, 3);
        assert!(!plug.on);
    }

    #[tokio::test]
    async fn disconnected_cleanup_reports_failure_without_blind_write() {
        let mut plug = Fake::new(false);
        plug.fail_write = Some(1);
        plug.apply_before_error = true;
        plug.reads.push_back(Ok(State {
            id: "fixture".into(),
            on: false,
        }));
        plug.reads.push_back(Err(anyhow::anyhow!("disconnected")));
        let error = round_trip(&mut plug, Duration::ZERO).await.unwrap_err();
        assert!(error.to_string().contains("Restoration failed"));
        assert_eq!(plug.writes, vec![true]);
        assert!(plug.on);
    }

    #[tokio::test]
    async fn stale_read_is_polled_without_repeating_write() {
        let mut plug = Fake::new(false);
        for on in [false, false, true, false] {
            plug.reads.push_back(Ok(State {
                id: "fixture".into(),
                on,
            }));
        }
        round_trip(&mut plug, Duration::ZERO).await.unwrap();
        assert_eq!(plug.writes, vec![true, false]);
        assert_eq!(plug.read_count, 4);
    }
}
