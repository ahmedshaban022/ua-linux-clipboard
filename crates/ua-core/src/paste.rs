//! The Paste Pipeline — the reliability core (spec §2, ticket 02).
//!
//! Runs after the user picks an entry and the panel has already closed:
//! wait for focus to return to the target app, set the selection, wait for
//! ownership confirmation, then inject Ctrl+V — with bounded retries and a
//! graceful fallback ("clipboard set — press Ctrl+V"). This state machine
//! exists to kill the "sometimes pastes, sometimes not" class of bug
//! (Clipboard Indicator #433): every step is waited-for explicitly, and
//! every timeout has a defined outcome.

/// Environment ports the pipeline drives. All checks are non-blocking;
/// the pipeline polls via `sleep_ms` so tests can script outcomes
/// deterministically with a fake.
pub trait PastePorts {
    /// Has keyboard focus returned to a window other than our panel?
    fn focus_returned(&mut self) -> bool;
    /// Ask the OS to make `entry_id` the clipboard content.
    fn set_selection(&mut self, entry_id: u64) -> bool;
    /// Has our selection ownership been confirmed by the compositor?
    fn selection_confirmed(&mut self) -> bool;
    /// Inject Ctrl+V into whatever has focus.
    fn inject_ctrl_v(&mut self) -> bool;
    /// Wait/poll step (real impl sleeps; tests record or step fakes).
    fn sleep_ms(&mut self, ms: u64);
}

#[derive(Debug, Clone, PartialEq)]
pub struct PastePolicy {
    pub focus_timeout_ms: u64,
    pub confirm_timeout_ms: u64,
    pub retries: u32,
    pub poll_ms: u64,
}

impl Default for PastePolicy {
    fn default() -> Self {
        PastePolicy {
            focus_timeout_ms: 300,
            confirm_timeout_ms: 200,
            retries: 2,
            poll_ms: 25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    /// Ctrl+V injected — the Windows experience.
    Pasted,
    /// Selection set but injection failed: show the fallback toast.
    FallbackClipboardSet,
    /// Could not even set the selection.
    Failed,
}

pub fn run_paste(ports: &mut dyn PastePorts, entry_id: u64, policy: &PastePolicy) -> PasteOutcome {
    // 1. Wait for focus to leave our (now closed) panel and return to the target.
    let mut waited = 0u64;
    while !ports.focus_returned() {
        if waited >= policy.focus_timeout_ms {
            // Panel is closed; focus state unknown. Proceed anyway — the
            // selection step is harmless and injection may still land.
            break;
        }
        ports.sleep_ms(policy.poll_ms);
        waited += policy.poll_ms;
    }

    // 2..n. Set selection, confirm ownership, inject. Bounded retries.
    let mut selection_set = false;
    for attempt in 0..=policy.retries {
        if attempt > 0 {
            ports.sleep_ms(50);
        }
        if !ports.set_selection(entry_id) {
            continue;
        }
        selection_set = true;

        let mut confirmed_wait = 0u64;
        loop {
            if ports.selection_confirmed() {
                if ports.inject_ctrl_v() {
                    return PasteOutcome::Pasted;
                }
                break; // injection failed → retry the whole cycle
            }
            if confirmed_wait >= policy.confirm_timeout_ms {
                break; // confirmation timeout → retry
            }
            ports.sleep_ms(policy.poll_ms);
            confirmed_wait += policy.poll_ms;
        }
    }

    if selection_set {
        PasteOutcome::FallbackClipboardSet
    } else {
        PasteOutcome::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Scriptable fake: each port method pops its next result.
    struct FakePorts {
        focus: Vec<bool>,
        set: Vec<bool>,
        confirm: Vec<bool>,
        inject: Vec<bool>,
        sleeps: RefCell<Vec<u64>>,
    }

    impl FakePorts {
        fn new(focus: Vec<bool>, set: Vec<bool>, confirm: Vec<bool>, inject: Vec<bool>) -> Self {
            FakePorts {
                focus,
                set,
                confirm,
                inject,
                sleeps: RefCell::new(vec![]),
            }
        }
        fn sleep_total(&self) -> u64 {
            self.sleeps.borrow().iter().sum()
        }
    }

    impl PastePorts for FakePorts {
        fn focus_returned(&mut self) -> bool {
            self.focus.pop().unwrap_or(true)
        }
        fn set_selection(&mut self, _id: u64) -> bool {
            self.set.pop().unwrap_or(true)
        }
        fn selection_confirmed(&mut self) -> bool {
            self.confirm.pop().unwrap_or(true)
        }
        fn inject_ctrl_v(&mut self) -> bool {
            self.inject.pop().unwrap_or(true)
        }
        fn sleep_ms(&mut self, ms: u64) {
            self.sleeps.borrow_mut().push(ms);
        }
    }

    const POLICY: PastePolicy = PastePolicy {
        focus_timeout_ms: 300,
        confirm_timeout_ms: 200,
        retries: 2,
        poll_ms: 25,
    };

    #[test]
    fn happy_path_pastes_without_sleeping() {
        // pop() takes from the end, so success values are already defaults.
        let mut p = FakePorts::new(vec![], vec![], vec![], vec![]);
        assert_eq!(run_paste(&mut p, 7, &POLICY), PasteOutcome::Pasted);
        assert_eq!(p.sleep_total(), 0);
    }

    #[test]
    fn focus_eventually_returns_then_pastes() {
        // two "not yet" then default true: [notyet, notyet] popped first
        let mut p = FakePorts::new(vec![false, false], vec![], vec![], vec![]);
        assert_eq!(run_paste(&mut p, 7, &POLICY), PasteOutcome::Pasted);
        assert_eq!(p.sleep_total(), 50);
    }

    #[test]
    fn focus_timeout_still_attempts_selection() {
        // focus never returns; with poll 25 and timeout 300 we do 12 polls.
        let mut p = FakePorts::new(vec![false; 64], vec![], vec![], vec![]);
        assert_eq!(run_paste(&mut p, 7, &POLICY), PasteOutcome::Pasted);
        assert!(p.sleep_total() >= 300);
    }

    #[test]
    fn confirm_timeout_retries_then_falls_back() {
        // confirm: [timeout ×3] → all attempts fail confirm
        let mut p = FakePorts::new(vec![], vec![], vec![false; 64], vec![]);
        assert_eq!(
            run_paste(&mut p, 7, &POLICY),
            PasteOutcome::FallbackClipboardSet
        );
    }

    #[test]
    fn second_attempt_succeeds_after_injection_failure() {
        // inject: [true(later attempt), false(first)]
        let mut p = FakePorts::new(vec![], vec![], vec![], vec![true, false]);
        assert_eq!(run_paste(&mut p, 7, &POLICY), PasteOutcome::Pasted);
    }

    #[test]
    fn selection_never_set_fails() {
        let mut p = FakePorts::new(vec![], vec![false; 8], vec![], vec![]);
        assert_eq!(run_paste(&mut p, 7, &POLICY), PasteOutcome::Failed);
    }
}
