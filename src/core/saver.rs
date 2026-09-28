use std::time::{Duration, Instant};

/// What the UI should do with the disk right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Nothing,
    Write(String),
    Delete,
}

/// Decides when and whether to save. Pure: the UI feeds it edits, time and
/// results, and carries out the returned `Action`.
///
/// `on_disk` mirrors the file: `Some(text)` when a file exists with that text,
/// `None` when there is no file. A pending edit stays pending until the UI
/// reports `written` or `deleted`, so a failed write is retried on the next
/// `flush` or `tick`.
#[derive(Debug)]
pub struct Saver {
    on_disk: Option<String>,
    pending: Option<(String, Instant)>,
    delay: Duration,
}

impl Saver {
    pub fn new(on_disk: Option<String>, delay: Duration) -> Saver {
        Saver {
            on_disk,
            pending: None,
            delay,
        }
    }

    /// The buffer changed. Restarts the delay.
    pub fn edited(&mut self, text: String, now: Instant) {
        self.pending = Some((text, now));
    }

    pub fn is_dirty(&self) -> bool {
        self.pending.is_some()
    }

    #[cfg(test)]
    pub fn has_file(&self) -> bool {
        self.on_disk.is_some()
    }

    /// Time left until the pending edit is due. `None` when nothing is pending.
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.pending.as_ref().map(|(_, at)| {
            self.delay
                .saturating_sub(now.saturating_duration_since(*at))
        })
    }

    /// Called when the timer fires. Acts only when the delay has passed.
    pub fn tick(&mut self, now: Instant) -> Action {
        match self.remaining(now) {
            Some(left) if left.is_zero() => self.decide(),
            _ => Action::Nothing,
        }
    }

    /// Act now, regardless of the delay.
    pub fn flush(&mut self) -> Action {
        if self.pending.is_some() {
            self.decide()
        } else {
            Action::Nothing
        }
    }

    /// The UI wrote `text` to disk.
    pub fn written(&mut self, text: String) {
        self.on_disk = Some(text);
        self.pending = None;
    }

    /// The UI removed the file.
    pub fn deleted(&mut self) {
        self.on_disk = None;
        self.pending = None;
    }

    fn decide(&mut self) -> Action {
        let Some((text, _)) = self.pending.as_ref() else {
            return Action::Nothing;
        };
        let action = if text.trim().is_empty() {
            if self.on_disk.is_some() {
                Action::Delete
            } else {
                Action::Nothing
            }
        } else if self.on_disk.as_deref() == Some(text.as_str()) {
            Action::Nothing
        } else {
            Action::Write(text.clone())
        };
        if action == Action::Nothing {
            self.pending = None;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELAY: Duration = Duration::from_secs(2);

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn nothing_pending_means_nothing_to_do() {
        let mut s = Saver::new(None, DELAY);
        let t0 = Instant::now();
        assert_eq!(s.tick(t0), Action::Nothing);
        assert_eq!(s.flush(), Action::Nothing);
        assert!(!s.is_dirty());
        assert!(!s.has_file());
        assert_eq!(s.remaining(t0), None);
    }

    #[test]
    fn write_only_after_the_delay() {
        let mut s = Saver::new(None, DELAY);
        let t0 = Instant::now();
        s.edited("hello".into(), t0);
        assert!(s.is_dirty());
        assert_eq!(s.remaining(t0 + secs(1)), Some(secs(1)));
        assert_eq!(s.tick(t0 + secs(1)), Action::Nothing);
        assert!(s.is_dirty());
        assert_eq!(s.remaining(t0 + secs(3)), Some(Duration::ZERO));
        assert_eq!(s.tick(t0 + secs(2)), Action::Write("hello".into()));
    }

    #[test]
    fn later_edit_restarts_the_delay() {
        let mut s = Saver::new(None, DELAY);
        let t0 = Instant::now();
        s.edited("a".into(), t0);
        s.edited("ab".into(), t0 + secs(1));
        assert_eq!(s.tick(t0 + secs(2)), Action::Nothing);
        assert_eq!(s.tick(t0 + secs(3)), Action::Write("ab".into()));
    }

    #[test]
    fn unchanged_text_is_not_written() {
        let mut s = Saver::new(Some("same".into()), DELAY);
        let t0 = Instant::now();
        s.edited("same".into(), t0);
        assert_eq!(s.tick(t0 + secs(2)), Action::Nothing);
        assert!(!s.is_dirty());
    }

    #[test]
    fn flush_writes_at_once() {
        let mut s = Saver::new(None, DELAY);
        s.edited("now".into(), Instant::now());
        assert_eq!(s.flush(), Action::Write("now".into()));
    }

    #[test]
    fn pending_stays_until_written() {
        let mut s = Saver::new(None, DELAY);
        s.edited("x".into(), Instant::now());
        assert_eq!(s.flush(), Action::Write("x".into()));
        assert!(s.is_dirty(), "a failed write must be retried later");
        assert_eq!(s.flush(), Action::Write("x".into()));
        s.written("x".into());
        assert!(!s.is_dirty());
        assert!(s.has_file());
        assert_eq!(s.flush(), Action::Nothing);
    }

    #[test]
    fn whitespace_deletes_an_existing_file_only() {
        let mut s = Saver::new(Some("old".into()), DELAY);
        s.edited("  \n".into(), Instant::now());
        assert_eq!(s.flush(), Action::Delete);
        s.deleted();
        assert!(!s.has_file());
        assert!(!s.is_dirty());

        let mut s = Saver::new(None, DELAY);
        s.edited("   ".into(), Instant::now());
        assert_eq!(s.flush(), Action::Nothing);
        assert!(!s.is_dirty());
    }
}
