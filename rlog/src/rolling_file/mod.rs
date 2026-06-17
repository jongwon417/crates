pub mod policy;

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
};

use log4rs::{
    append::Append,
    encode::{self, Encode},
};
use parking_lot::{Mutex, RwLock};

use crate::rolling_file::policy::compound::{
    CompoundPolicy, CompoundPolicyConfig, CompoundPolicyOption, CompoundPolicyOptionKind,
};

////////////////////////////////////////////////////////////////////////////////
/// Writer
////////////////////////////////////////////////////////////////////////////////

// #[derive(Debug)]
pub struct LogWriter {
    pub(crate) file: BufWriter<File>,
    pub(crate) path: String,
    pub(crate) len: usize,
}

impl LogWriter {
    #[inline]
    pub fn len_estimate(&self) -> usize {
        self.len
    }
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf).map(|n| {
            self.len += n;
            n
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl std::fmt::Debug for LogWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogWriter")
            .field("file", &self.file)
            .finish()
    }
}

impl encode::Write for LogWriter {}

#[derive(Debug)]
struct LogManager {
    writer: Mutex<Option<LogWriter>>,
    policy: Box<dyn policy::Policy>,
}

impl LogManager {
    fn build(option: &RollingFileOption) -> Self {
        Self {
            writer: Mutex::new(None),
            policy: Box::new(CompoundPolicy::new(CompoundPolicyConfig::from_option(
                &option.log_dir,
                &option.prefix,
                &option.policy,
            ))),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
/*                             RollingFileAppender                            */
////////////////////////////////////////////////////////////////////////////////

/// encoder pattern
pub(crate) const PATTERN: &str = "{d(%Y-%m-%d %H:%M:%S.%3f)} [{l}] [{M}, {L}] {m}{n}";

#[derive(Debug, Clone)]
pub struct RollingFileOption {
    pub log_dir: String,
    pub prefix: String,
    pub policy: CompoundPolicyOption,
}

/// 주기는 하루
/// 하루에 최대 64MB/16개 (1GB) 로그가 저장될 수 있다.
/// 90일이 넘은 파일은 삭제된다.
impl Default for RollingFileOption {
    fn default() -> Self {
        Self {
            log_dir: format!("."),
            prefix: format!("rlog"),
            policy: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct RollingFileAppender {
    manager: LogManager,
    log_dir: String, // /home/stem/log
    prefix: String,  // STEM_AG_KFK
    option: RollingFileOption,
    encoder: Box<dyn Encode>,
}

impl Append for RollingFileAppender {
    fn append(&self, record: &log::Record) -> anyhow::Result<()> {
        let mut writer = self.manager.writer.lock();

        // try roll
        self.manager.policy.process(&mut writer)?;

        // write
        let log_writer = self.get_writer(&mut writer)?;
        self.encoder.encode(log_writer, record)?;
        log_writer.flush()?;

        Ok(())
    }

    fn flush(&self) {}
}

use std::os::unix::fs::PermissionsExt;

impl RollingFileAppender {
    pub fn builder(encoder: Box<dyn Encode>) -> RollingFileAppenderBuilder {
        RollingFileAppenderBuilder { encoder }
    }

    fn get_writer<'a>(&self, writer: &'a mut Option<LogWriter>) -> io::Result<&'a mut LogWriter> {
        if writer.is_none() {
            let path = get_path(&self.log_dir, &self.prefix, &self.option.policy.kind);
            let file = OpenOptions::new()
                .append(true)
                .truncate(false)
                .create(true)
                .open(&path)?;

            let _ = std::os::unix::fs::fchown(&file, Some(1000), Some(1000));
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));

            *writer = Some(LogWriter {
                len: file.metadata()?.len() as usize,
                path,
                file: BufWriter::with_capacity(0x1000, file),
            });
        }

        // :( unwrap
        Ok(writer.as_mut().unwrap())
    }
}

pub struct RollingFileAppenderBuilder {
    encoder: Box<dyn Encode>,
}

impl RollingFileAppenderBuilder {
    pub fn build(self, option: &RollingFileOption) -> io::Result<RollingFileAppender> {
        fs::create_dir_all(&option.log_dir)?;

        let appender = RollingFileAppender {
            manager: LogManager::build(&option),
            log_dir: option.log_dir.to_string(),
            prefix: option.prefix.to_string(),
            encoder: self.encoder,
            option: option.clone(),
        };

        // open the log file immediately
        appender.get_writer(&mut appender.manager.writer.lock())?;

        Ok(appender)
    }
}

fn get_path(log_dir: &str, prefix: &str, kind: &CompoundPolicyOptionKind) -> String {
    use CompoundPolicyOptionKind::*;
    match kind {
        Time(time_option) | Compound(time_option, _) => {
            format!(
                "{}/{}.{}.log",
                log_dir,
                prefix,
                chrono::Local::now().format(time_option.interval.get_strfmt())
            )
        }
        Size(..) => {
            format!("{}/{}.log", log_dir, prefix)
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
/*                         ThreadRollingFileAppender                          */
////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct ThreadRollingFileAppender {
    managers: RwLock<HashMap<String, LogManager>>, // <thread_name, file>
    log_dir: String,
    prefix: String,
    option: RollingFileOption,
    encoder: Box<dyn Encode>,
}

impl Append for ThreadRollingFileAppender {
    fn append(&self, record: &log::Record) -> anyhow::Result<()> {
        let key = self.get_key();

        self.ensure_manager(&key);

        let r = self.managers.read();

        // ensure_manager로 키는 반드시 존재한다.
        let manager = r.get(&key).unwrap();
        let mut writer = manager.writer.lock();

        // try roll
        manager.policy.process(&mut writer)?;

        // write
        let log_writer = self.get_writer(&key, &mut writer)?;
        self.encoder.encode(log_writer, record)?;
        log_writer.flush()?;

        Ok(())
    }

    fn flush(&self) {}
}

impl ThreadRollingFileAppender {
    pub fn builder(encoder: Box<dyn Encode>) -> ThreadRollingFileAppenderBuilder {
        ThreadRollingFileAppenderBuilder { encoder }
    }

    #[inline]
    fn get_key(&self) -> String {
        format!(
            "{}.{}",
            self.prefix,
            sanitize_file_part(std::thread::current().name().unwrap_or("unnamed"))
        )
    }

    fn get_writer<'a>(
        &self,
        key: &str,
        writer: &'a mut Option<LogWriter>,
    ) -> io::Result<&'a mut LogWriter> {
        if writer.is_none() {
            let path = get_path(&self.log_dir, key, &self.option.policy.kind);
            let file = OpenOptions::new()
                .append(true)
                .truncate(false)
                .create(true)
                .open(&path)?;

            std::os::unix::fs::fchown(&file, Some(1000), Some(1000))?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;

            *writer = Some(LogWriter {
                len: file.metadata()?.len() as usize,
                path,
                file: BufWriter::with_capacity(0x1000, file),
            });
        }

        // :( unwrap
        Ok(writer.as_mut().unwrap())
    }

    fn ensure_manager(&self, key: &str) {
        let r = self.managers.read();
        if r.contains_key(key) {
            return;
        }
        drop(r);

        let mut w = self.managers.write();
        // 여러 스레드에서 접근하기 때문에 앞에서 contains_key를 확인하는 동안 새 키가 생성되었을 수도 있다.
        if !w.contains_key(key) {
            let mut option = self.option.clone();
            option.prefix = key.to_string();
            w.insert(key.to_string(), LogManager::build(&option));
        }
    }
}

fn sanitize_file_part(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        "unknown".to_string()
    } else {
        out
    }
}

pub struct ThreadRollingFileAppenderBuilder {
    encoder: Box<dyn Encode>,
}

impl ThreadRollingFileAppenderBuilder {
    pub fn build(self, option: &RollingFileOption) -> ThreadRollingFileAppender {
        ThreadRollingFileAppender {
            managers: RwLock::new(HashMap::new()),
            log_dir: option.log_dir.to_string(),
            prefix: option.prefix.to_string(),
            encoder: self.encoder,
            option: option.clone(),
        }
    }
}
