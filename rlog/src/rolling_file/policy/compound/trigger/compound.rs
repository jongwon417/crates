use crate::rolling_file::{
        LogWriter,
        policy::compound::trigger::{
            Trigger, TriggerKind,
            size::{SizeTrigger, SizeTriggerConfig},
            time::{TimeTrigger, TimeTriggerConfig, TimeTriggerInterval},
        },
    };

#[derive(Debug)]
pub(crate) struct CompoundTriggerConfig {
    time: TimeTriggerConfig,
    size: SizeTriggerConfig,
}
impl CompoundTriggerConfig {
    pub(crate) fn from_options(interval: TimeTriggerInterval, limit: usize) -> Self {
        Self {
            time: TimeTriggerConfig::from_options(interval),
            size: SizeTriggerConfig::from_options(limit),
        }
    }
}

#[derive(Debug)]
pub struct CompoundTrigger {
    time: TimeTrigger,
    size: SizeTrigger,
}

impl CompoundTrigger {
    /// Returns a new trigger which rolls the log once it has passed the
    /// specified size in bytes.
    pub(crate) fn new(config: CompoundTriggerConfig) -> Self {
        Self {
            time: TimeTrigger::new(config.time),
            size: SizeTrigger::new(config.size),
        }
    }
}

impl Trigger for CompoundTrigger {
    fn trigger(&self, w: &LogWriter) -> Option<TriggerKind> {
        if let Some(kind) = self.time.trigger(w) {
            Some(kind)
        } else {
            self.size.trigger(w)
        }
    }
}
