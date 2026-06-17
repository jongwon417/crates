use crate::rolling_file::policy::compound::trigger::TriggerKind;

pub mod fixed_window;

pub trait Roll: std::fmt::Debug + Send + Sync + 'static {
    fn roll(&self, file: &str, kind: TriggerKind) -> anyhow::Result<()>;
}
