use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use chrono::{Local, TimeZone, Timelike};

use crate::rolling_file::policy::compound::{
    CompoundPolicySizeOption, CompoundPolicyTimeOption,
    roll::Roll,
    trigger::{TriggerKind, time::TimeTriggerInterval},
};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Compression {
    None,
    Gzip,
    Zstd,
}

impl Compression {
    fn compress(&self, src: &str, dst: &str) -> std::io::Result<()> {
        match *self {
            Compression::None => move_file(src, dst),
            Compression::Gzip => {
                use flate2::write::GzEncoder;
                use std::fs::File;

                let mut i = File::open(src)?;

                let o = File::create(dst)?;
                let mut o = GzEncoder::new(o, flate2::Compression::default());

                std::io::copy(&mut i, &mut o)?;
                drop(o.finish()?);
                drop(i); // needs to happen before remove_file call on Windows

                fs::remove_file(src)
            }
            Compression::Zstd => {
                use std::fs::File;
                let mut i = File::open(src)?;
                let mut o = {
                    let target = File::create(dst)?;
                    zstd::Encoder::new(target, zstd::DEFAULT_COMPRESSION_LEVEL)?
                };
                std::io::copy(&mut i, &mut o)?;
                drop(o.finish()?);
                drop(i);
                fs::remove_file(src)
            }
        }
    }
}

#[derive(Debug)]
pub struct FixedWindowRollerTimeConfig {
    pub(crate) secs: u64,
    pub(crate) interval: TimeTriggerInterval,
}
impl FixedWindowRollerTimeConfig {
    fn from_options(o: &CompoundPolicyTimeOption) -> Self {
        Self {
            secs: o.interval.get_secs(o.retention_interval),
            interval: o.interval,
        }
    }
}

#[derive(Debug)]
pub struct FixedWindowRollerSizeConfig {
    pub(crate) count: usize,
}
impl FixedWindowRollerSizeConfig {
    fn from_options(o: &CompoundPolicySizeOption) -> Self {
        Self {
            count: o.files_per_interval,
        }
    }
}

#[derive(Debug)]
pub struct FixedWindowRollerCompoundConfig {
    pub(crate) time: FixedWindowRollerTimeConfig,
    pub(crate) size: FixedWindowRollerSizeConfig,
}
impl FixedWindowRollerCompoundConfig {
    fn from_options(o1: &CompoundPolicyTimeOption, o2: &CompoundPolicySizeOption) -> Self {
        Self {
            time: FixedWindowRollerTimeConfig::from_options(o1),
            size: FixedWindowRollerSizeConfig::from_options(o2),
        }
    }
}

#[derive(Debug)]
pub enum FixedWindowRollerConfigKind {
    Time(FixedWindowRollerTimeConfig),
    Size(FixedWindowRollerSizeConfig),
    Compound(FixedWindowRollerCompoundConfig),
}
impl FixedWindowRollerConfigKind {
    pub(crate) fn time_config(o: &CompoundPolicyTimeOption) -> Self {
        Self::Time(FixedWindowRollerTimeConfig::from_options(o))
    }

    pub(crate) fn size_config(o: &CompoundPolicySizeOption) -> Self {
        Self::Size(FixedWindowRollerSizeConfig::from_options(o))
    }

    pub(crate) fn compound_config(
        o1: &CompoundPolicyTimeOption,
        o2: &CompoundPolicySizeOption,
    ) -> Self {
        Self::Compound(FixedWindowRollerCompoundConfig::from_options(o1, o2))
    }
}

#[derive(Debug)]
pub(crate) struct FixedWindowRollerConfig {
    log_dir: String, // /home/stem/log
    prefix: String,  // STEM_AG_KFK
    kind: FixedWindowRollerConfigKind,
    compression: Compression,
}
impl FixedWindowRollerConfig {
    pub(crate) fn from_options_time(
        log_dir: &str,
        prefix: &str,
        o: &CompoundPolicyTimeOption,
        compression: Compression,
    ) -> Self {
        Self {
            log_dir: log_dir.to_string(),
            prefix: prefix.to_string(),
            kind: FixedWindowRollerConfigKind::time_config(o),
            compression,
        }
    }

    pub(crate) fn from_options_size(
        log_dir: &str,
        prefix: &str,
        o: &CompoundPolicySizeOption,
        compression: Compression,
    ) -> Self {
        Self {
            log_dir: log_dir.to_string(),
            prefix: prefix.to_string(),
            kind: FixedWindowRollerConfigKind::size_config(o),
            compression,
        }
    }

    pub(crate) fn from_options_compound(
        log_dir: &str,
        prefix: &str,
        o1: &CompoundPolicyTimeOption,
        o2: &CompoundPolicySizeOption,
        compression: Compression,
    ) -> Self {
        Self {
            log_dir: log_dir.to_string(),
            prefix: prefix.to_string(),
            kind: FixedWindowRollerConfigKind::compound_config(o1, o2),
            compression,
        }
    }
}

#[derive(Debug)]
pub struct FixedWindowRoller {
    log_dir: String, // /home/stem/log/STEM_AG_KFK
    prefix: String,  // STEM_AG_KFK
    compression: Compression,
    kind: FixedWindowRollerConfigKind,
}

impl FixedWindowRoller {
    pub fn builder() -> FixedWindowRollerBuilder {
        FixedWindowRollerBuilder {}
    }

    // 파일 넘버링(맨 뒤에)
    fn roll_size(&self, path: &str) -> anyhow::Result<()> {
        let config = match &self.kind {
            FixedWindowRollerConfigKind::Time(_) => unreachable!(),
            FixedWindowRollerConfigKind::Size(c) => c,
            FixedWindowRollerConfigKind::Compound(c) => &c.size,
        };

        rotate(self.compression, config.count, path).map_err(Into::into)
    }

    // 파일 메타데이터의 생성날짜로 오래된 파일 삭제
    fn roll_time(&self) -> anyhow::Result<()> {
        let config = match &self.kind {
            FixedWindowRollerConfigKind::Time(c) => c,
            FixedWindowRollerConfigKind::Size(_) => unreachable!(),
            FixedWindowRollerConfigKind::Compound(c) => &c.time,
        };

        // 로그 파일은 롤링 경계 시각이 아니라 경계 이후 첫 로그가 찍힐 때 생성된다.
        // 그래서 현재 시각을 주기보다 조금 보수적으로 내리고, 파일 생성 시각 기준으로
        // 늦게 생성된 파일이 설정한 보관 주기보다 과하게 오래 남지 않도록 한다.
        let now: SystemTime = match config.interval {
            TimeTriggerInterval::Day => {
                let round = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
                Local.from_local_datetime(&round).unwrap().into()
            }
            TimeTriggerInterval::Minute(_) => {
                let now = Local::now();
                let round = now
                    .date_naive()
                    .and_hms_opt(now.hour(), now.minute(), 0)
                    .unwrap();
                Local.from_local_datetime(&round).unwrap().into()
            }
        };
        let ago = now - Duration::from_secs(config.secs);

        for entry in fs::read_dir(&self.log_dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };

            if !file_name.starts_with(&self.prefix) {
                continue;
            }

            let metadata = entry.metadata()?;

            if metadata.is_file() && metadata.created().is_ok_and(|created| created < ago) {
                fs::remove_file(&path)?;
            }
        }
        Ok(())
    }
}

fn move_file<P, Q>(src: P, dst: Q) -> std::io::Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    // first try a rename
    match fs::rename(src.as_ref(), dst.as_ref()) {
        Ok(()) => return Ok(()),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {}
    }

    // fall back to a copy and delete if src and dst are on different mounts
    fs::copy(src.as_ref(), dst.as_ref()).and_then(|_| fs::remove_file(src.as_ref()))
}

fn rotate(compression: Compression, count: usize, file: &str) -> std::io::Result<()> {
    if count == 0 {
        return fs::remove_file(file);
    }

    let archive_path = |idx: usize| {
        let mut archive = OsString::from(file);
        archive.push(".");
        archive.push(idx.to_string());
        PathBuf::from(archive)
    };
    let last = count - 1;

    let _ = fs::remove_file(archive_path(last));

    for idx in (0..last).rev() {
        move_file(archive_path(idx), archive_path(idx + 1))?;
    }

    let dst_0 = archive_path(0);

    compression.compress(&file, dst_0.to_string_lossy().as_ref())?;
    Ok(())
}

impl Roll for FixedWindowRoller {
    fn roll(&self, path: &str, kind: TriggerKind) -> anyhow::Result<()> {
        use TriggerKind::*;
        match kind {
            Time => self.roll_time(),
            Size => self.roll_size(path),
        }
    }
}

#[derive(Debug, Default)]
pub struct FixedWindowRollerBuilder {}

impl FixedWindowRollerBuilder {
    pub(crate) fn build(
        self,
        config: FixedWindowRollerConfig,
    ) -> anyhow::Result<FixedWindowRoller> {
        Ok(FixedWindowRoller {
            log_dir: config.log_dir,
            prefix: config.prefix,
            compression: config.compression,
            kind: config.kind,
        })
    }
}
