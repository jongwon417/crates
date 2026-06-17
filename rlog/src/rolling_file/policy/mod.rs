pub mod compound;

use crate::rolling_file::LogWriter;

pub trait Policy: Sync + Send + 'static + std::fmt::Debug {
    fn process(&self, writer: &mut Option<LogWriter>) -> anyhow::Result<()>;
}
