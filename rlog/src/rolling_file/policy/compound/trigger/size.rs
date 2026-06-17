use crate::rolling_file::{
    LogWriter,
    policy::compound::trigger::{Trigger, TriggerKind},
};

#[derive(Debug)]
pub(crate) struct SizeTriggerConfig {
    limit: usize,
}

impl SizeTriggerConfig {
    pub(crate) fn from_options(limit: usize) -> Self {
        Self { limit }
    }
}

/// A trigger which rolls the log once it has passed a certain size.
#[derive(Debug)]
pub struct SizeTrigger {
    limit: usize,
}

impl SizeTrigger {
    /// Returns a new trigger which rolls the log once it has passed the
    /// specified size in bytes.
    pub(crate) fn new(config: SizeTriggerConfig) -> SizeTrigger {
        SizeTrigger {
            limit: config.limit,
        }
    }
}

impl Trigger for SizeTrigger {
    fn trigger(&self, w: &LogWriter) -> Option<TriggerKind> {
        if w.len_estimate() > self.limit {
            Some(TriggerKind::Size)
        } else {
            None
        }
    }
}
