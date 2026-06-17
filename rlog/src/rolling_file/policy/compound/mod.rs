pub mod roll;
pub mod trigger;

use crate::rolling_file::{
        LogWriter,
        policy::{
            Policy,
            compound::{
                roll::fixed_window::{Compression, FixedWindowRoller, FixedWindowRollerConfig},
                trigger::{
                    Trigger,
                    compound::{CompoundTrigger, CompoundTriggerConfig},
                    size::{SizeTrigger, SizeTriggerConfig},
                    time::{TimeTrigger, TimeTriggerConfig, TimeTriggerInterval},
                },
            },
        },
    };

use roll::Roll;

const DEFAULT_RETENTION_INTERVAL: u64 = 90;
#[derive(Debug, Clone)]
pub struct CompoundPolicyTimeOption {
    pub interval: TimeTriggerInterval, // 주기
    pub retention_interval: u64,       // 유지할 주기
}
impl Default for CompoundPolicyTimeOption {
    fn default() -> Self {
        Self {
            interval: TimeTriggerInterval::Day,
            retention_interval: DEFAULT_RETENTION_INTERVAL,
        }
    }
}

const DEFAULT_MAX_FILE_SIZE: usize = 0x0400_0000;
const DEFAULT_FILES_PER_INTERVAL: usize = 0x10;
#[derive(Debug, Clone)]
pub struct CompoundPolicySizeOption {
    pub max_file_size: usize, // 파일 최대 크기, 마지막 출력 로그 크기에 따라 초과 될 수 있음
    pub files_per_interval: usize, // 한 주기에 존재할 수 있는 파일 개수
}
impl Default for CompoundPolicySizeOption {
    fn default() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            files_per_interval: DEFAULT_FILES_PER_INTERVAL,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CompoundPolicyOptionKind {
    Time(CompoundPolicyTimeOption),
    Size(CompoundPolicySizeOption),
    Compound(CompoundPolicyTimeOption, CompoundPolicySizeOption),
}

#[derive(Debug, Clone)]
pub struct CompoundPolicyOption {
    pub kind: CompoundPolicyOptionKind,
    pub compression: Compression,
}
impl Default for CompoundPolicyOption {
    fn default() -> Self {
        Self {
            kind: CompoundPolicyOptionKind::Compound(Default::default(), Default::default()),
            compression: Compression::None,
        }
    }
}

impl CompoundPolicyOption {
    pub fn set_retention_interval(&mut self, retention_interval: u64) {
        use CompoundPolicyOptionKind::*;
        match &mut self.kind {
            Time(option) | Compound(option, _) => option.retention_interval = retention_interval,
            Size(..) => {}
        }
    }
}

////////////////////////////////////////
/*               Config               */
////////////////////////////////////////

#[derive(Debug)]
pub(crate) enum TriggerConfig {
    Time(TimeTriggerConfig),
    Size(SizeTriggerConfig),
    Compound(CompoundTriggerConfig),
}
impl TriggerConfig {
    fn time_config(o: &CompoundPolicyTimeOption) -> Self {
        Self::Time(TimeTriggerConfig::from_options(o.interval))
    }

    fn size_config(o: &CompoundPolicySizeOption) -> Self {
        Self::Size(SizeTriggerConfig::from_options(o.max_file_size))
    }

    fn compound_config(o1: &CompoundPolicyTimeOption, o2: &CompoundPolicySizeOption) -> Self {
        Self::Compound(CompoundTriggerConfig::from_options(
            o1.interval,
            o2.max_file_size,
        ))
    }
}

#[derive(Debug)]
pub(crate) struct CompoundPolicyConfig {
    pub trigger: TriggerConfig,
    pub roller: FixedWindowRollerConfig,
}

impl CompoundPolicyConfig {
    pub(crate) fn from_option(log_dir: &str, prefix: &str, option: &CompoundPolicyOption) -> Self {
        use CompoundPolicyOptionKind::*;
        let (trigger, roller) = match &option.kind {
            Time(o) => (
                TriggerConfig::time_config(o),
                FixedWindowRollerConfig::from_options_time(log_dir, prefix, o, option.compression),
            ),
            Size(o) => (
                TriggerConfig::size_config(o),
                FixedWindowRollerConfig::from_options_size(log_dir, prefix, o, option.compression),
            ),
            Compound(o1, o2) => (
                TriggerConfig::compound_config(o1, o2),
                FixedWindowRollerConfig::from_options_compound(
                    log_dir,
                    prefix,
                    o1,
                    o2,
                    option.compression,
                ),
            ),
        };

        Self { trigger, roller }
    }
}

////////////////////////////////////////
/*           CompoundPolicy           */
////////////////////////////////////////

#[derive(Debug)]
pub struct CompoundPolicy {
    trigger: Box<dyn Trigger>,
    roller: Box<dyn Roll>,
}

impl CompoundPolicy {
    pub(crate) fn new(config: CompoundPolicyConfig) -> Self {
        use TriggerConfig::*;
        Self {
            trigger: match config.trigger {
                Time(c) => Box::new(TimeTrigger::new(c)),
                Size(c) => Box::new(SizeTrigger::new(c)),
                Compound(c) => Box::new(CompoundTrigger::new(c)),
            },
            roller: Box::new(FixedWindowRoller::builder().build(config.roller).unwrap()),
        }
    }
}

impl Policy for CompoundPolicy {
    fn process(&self, writer: &mut Option<LogWriter>) -> anyhow::Result<()> {
        if let Some(w) = writer {
            match self.trigger.trigger(w) {
                Some(kind) => {
                    let path = w.path.clone();
                    *writer = None;
                    self.roller.roll(&path, kind)?;
                }
                None => {}
            }
        }
        Ok(())
    }
}
