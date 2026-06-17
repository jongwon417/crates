pub mod rolling_file;

use std::{collections::HashSet, sync::OnceLock};

use log4rs::{
    Config,
    append::console::ConsoleAppender,
    config::{Appender, Logger, Root},
    encode::pattern::PatternEncoder,
};

use rolling_file::{PATTERN, RollingFileAppender, RollingFileOption, ThreadRollingFileAppender};

pub use log::{LevelFilter, debug, error, info, max_level, set_max_level, trace, warn};

static LOG_HANDLE: OnceLock<log4rs::Handle> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct ThreadLogOptions {
    pub appender_name: String,
    pub additive: bool,
    pub modules: HashSet<String>,
}

pub struct LogOptions {
    pub rolling: RollingFileOption,
    pub stdout: bool,
    pub panic_hook: bool,
    pub log_off_modules: HashSet<String>,
    pub thread_options: ThreadLogOptions,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            rolling: RollingFileOption::default(),
            stdout: false,
            panic_hook: false,
            log_off_modules: HashSet::new(),
            thread_options: ThreadLogOptions {
                appender_name: format!("thread_logger"),
                additive: false,
                modules: HashSet::new(),
            },
        }
    }
}

pub fn log_init(log_dir: &str, prefix: &str, retention_interval: Option<u64>) {
    let mut options = LogOptions::default();
    options.rolling.log_dir = log_dir.to_string();
    options.rolling.prefix = prefix.to_string();
    if let Some(retention_interval) = retention_interval {
        options
            .rolling
            .policy
            .set_retention_interval(retention_interval);
    }
    log_init_with_options(&options);
}

pub fn log_init_with_stdout(log_dir: &str, prefix: &str, retention_interval: Option<u64>) {
    let mut options = LogOptions::default();
    options.rolling.log_dir = log_dir.to_string();
    options.rolling.prefix = prefix.to_string();
    if let Some(retention_interval) = retention_interval {
        options
            .rolling
            .policy
            .set_retention_interval(retention_interval);
    }
    options.stdout = true;
    log_init_with_options(&options);
}

pub fn log_init_with_default_options(log_dir: &str, prefix: &str) {
    let mut options = LogOptions::default();
    options.rolling.log_dir = log_dir.to_string();
    options.rolling.prefix = prefix.to_string();
    log_init_with_options(&options)
}

pub fn log_init_with_options(options: &LogOptions) {
    LOG_HANDLE.get_or_init(|| init_log4rs(options));
}

fn init_log4rs(options: &LogOptions) -> log4rs::Handle {
    let mut builder = Config::builder().appender(
        Appender::builder().build(
            "rollingfile",
            Box::new(
                RollingFileAppender::builder(Box::new(PatternEncoder::new(PATTERN)))
                    .build(&options.rolling)
                    .unwrap(),
            ),
        ),
    );
    let mut root = Root::builder();
    let mut base_appenders = vec!["rollingfile"];

    // 기본 로그를 stdout에도 같이 출력한다.
    if options.stdout {
        builder = builder.appender(
            Appender::builder().build(
                "stdout",
                Box::new(
                    ConsoleAppender::builder()
                        .encoder(Box::new(PatternEncoder::new(PATTERN)))
                        .build(),
                ),
            ),
        );
        base_appenders.push("stdout");
    }

    // root logger는 기본 로그 파일로 전체 로그를 받는다.
    root = root.appenders(base_appenders.iter().copied());

    // log_off_modules
    // 지정된 모듈의 로그는 출력하지 않는다.
    for module in &options.log_off_modules {
        builder = builder.logger(
            Logger::builder()
                .additive(false)
                .build(module, LevelFilter::Off),
        )
    }

    // module_options가 비어 있으면 root의 전체 로그 정책만 사용한다.
    // 값이 있으면 각 모듈을 전용 logger로 가로채고, additive로 부모 전파 여부를 정한다.
    let thread_appender_name = "thread";
    let mut thread_appender_created = false;

    for module in &options.thread_options.modules {
        if !thread_appender_created {
            builder = builder.appender(
                Appender::builder().build(
                    thread_appender_name,
                    Box::new(
                        ThreadRollingFileAppender::builder(Box::new(PatternEncoder::new(PATTERN)))
                            .build(&options.rolling),
                    ),
                ),
            );
            thread_appender_created = true;
        }

        builder = builder.logger(
            Logger::builder()
                .appender(thread_appender_name)
                .additive(options.thread_options.additive)
                .build(module, LevelFilter::Trace),
        );
    }

    let config = builder.build(root.build(LevelFilter::Trace)).unwrap();

    let handle = log4rs::init_config(config).unwrap();

    if options.panic_hook {
        log_panics::Config::new()
            .backtrace_mode(log_panics::BacktraceMode::Resolved)
            .install_panic_hook();
    }

    handle
}
