use crate::rolling_file::LogWriter;

pub mod compound;
pub mod size;
pub mod time;

pub enum TriggerKind {
    Time,
    Size,
}

pub trait Trigger: std::fmt::Debug + Send + Sync + 'static {
    fn trigger(&self, w: &LogWriter) -> Option<TriggerKind>;
}
