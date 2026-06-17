use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike};

use crate::rolling_file::{
    LogWriter,
    policy::compound::trigger::{Trigger, TriggerKind},
};
use std::sync::atomic::{AtomicI64, Ordering};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TimeTriggerInterval {
    Minute(u64),
    Day,
}

impl Default for TimeTriggerInterval {
    fn default() -> Self {
        TimeTriggerInterval::Minute(1)
    }
}

impl TimeTriggerInterval {
    pub fn get_strfmt(&self) -> &'static str {
        match self {
            TimeTriggerInterval::Minute(_) => "%Y-%m-%dT%H:%M",
            TimeTriggerInterval::Day => "%Y-%m-%d",
        }
    }

    pub fn get_secs(&self, count: u64) -> u64 {
        // 로그 파일은 주기 경계 시각(00:00, 12:34:00 등)에 생성되는 것이 아니라,
        // 해당 주기에서 첫 로그가 실제로 기록되는 시점에 생성된다.
        // 그래서 보관 기준을 정확한 1분/1일보다 조금 짧게 잡아 파일 생성 시각이 아닌
        // 파일명에 들어간 주기 기준으로 오래된 로그가 정리되도록 한다.
        let secs = match self {
            TimeTriggerInterval::Minute(s) => (*s).clamp(1, 60) * 59,
            TimeTriggerInterval::Day => 60 * 60 * 23,
        };
        count * secs
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub(crate) struct TimeTriggerConfig {
    interval: TimeTriggerInterval,
}
impl TimeTriggerConfig {
    pub(crate) fn from_options(interval: TimeTriggerInterval) -> Self {
        Self { interval }
    }
}
#[derive(Debug)]
pub struct TimeTrigger {
    config: TimeTriggerConfig,
    next_roll_time: AtomicI64,
}

impl TimeTrigger {
    pub(crate) fn new(config: TimeTriggerConfig) -> TimeTrigger {
        let current = Local::now();
        let next_roll_time = get_next_time(config, current);

        TimeTrigger {
            config,
            next_roll_time: AtomicI64::new(next_roll_time.timestamp_millis()),
        }
    }
}

impl Trigger for TimeTrigger {
    fn trigger(&self, _w: &LogWriter) -> Option<TriggerKind> {
        let current = Local::now();
        let next_roll_time = self.next_roll_time.load(Ordering::Relaxed);

        if current.timestamp_millis() < next_roll_time {
            None
        } else {
            self.next_roll_time.store(
                get_next_time(self.config, current).timestamp_millis(),
                Ordering::Relaxed,
            );
            Some(TriggerKind::Time)
        }
    }
}

fn get_next_time(config: TimeTriggerConfig, current: DateTime<Local>) -> DateTime<Local> {
    use TimeTriggerInterval::*;
    let year = current.year();
    let month = current.month();
    let day = current.day();
    let hour = current.hour();
    let min = current.minute();

    match &config.interval {
        Day => Local.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap() + Duration::days(1),
        Minute(n) => {
            let n = (*n).clamp(1, 60);
            let add = n - (min as u64 % n);
            Local
                .with_ymd_and_hms(year, month, day, hour, min, 0)
                .unwrap()
                + Duration::minutes(add as i64)
        }
    }
}
