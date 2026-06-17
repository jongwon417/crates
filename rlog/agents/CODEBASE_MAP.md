# Codebase Map

This document summarizes the current `rlog` crate as implemented.

Use `../AGENTS.md` for working rules. This file is for code wiring, logging behavior, and known design constraints.

## Summary

`rlog` is a shared logging crate built on top of `log` and `log4rs`.

The crate provides:

- process-wide initialization through `log_init*` functions
- a root rolling file appender
- optional stdout logging
- per-module logger overrides
- module-off behavior
- thread-split rolling files for selected modules

The active implementation is centered on `src/lib.rs` and `src/rolling_file`.

## Public Entry Points

`src/lib.rs` exposes the app-facing API.

- `log_init(log_dir, prefix, retention_interval)`: initializes default rolling file logging.
- `log_init_with_stdout(log_dir, prefix, retention_interval)`: initializes rolling file logging plus stdout.
- `log_init_with_default_options(log_dir, prefix)`: initializes with `LogOptions::default()`.
- `log_init_with_options(options)`: initializes with a caller-provided `LogOptions`.

Initialization is guarded by:

```rust
static LOG_HANDLE: OnceLock<log4rs::Handle>
```

Only the first successful call installs the logger. Later calls return the existing handle and do not rebuild configuration.

## Top-Level Options

`LogOptions` owns:

- `rolling: RollingFileOption`
- `stdout: bool`
- `module_options: HashMap<String, ModuleLogOptions>`

`ModuleLogOptions` currently has two modes:

- `Off`: registers a logger for the module with no appenders and `additive(false)`.
- `Thread(ThreadLogOptions)`: registers a module logger that writes through `ThreadRollingFileAppender`.

`ThreadLogOptions` currently contains:

- `additive: bool`

When `additive` is true, log4rs also propagates the module log record to parent/root loggers. When false, only the module appender path handles the record.

## log4rs Wiring

`init_log4rs` builds one root appender named `rollingfile` using `RollingFileAppender`.

If `stdout` is enabled, a `stdout` appender is also registered and attached to root.

For each non-root module entry in `module_options`:

- `Off` creates a logger with no appenders and no propagation.
- `Thread` creates one `ThreadRollingFileAppender` appender and attaches it to the module logger.

The module name is the log target prefix used by the `log` crate, for example `rdkafka` or `kfk_core::hipc`.

## RollingFileOption

`RollingFileOption` lives in `src/rolling_file/mod.rs`.

It contains:

- `log_dir`: directory for log files, for example `/home/stem/log`
- `prefix`: base file prefix, for example `STEM_AG_KFK`
- `policy: CompoundPolicyOption`

The default policy is compound time + size rolling.

## RollingFileAppender

`RollingFileAppender` implements `log4rs::append::Append`.

It owns:

- one `LogManager`
- shared appender metadata (`log_dir`, `prefix`, `option`)
- one encoder

On every append:

1. Lock the manager writer.
2. Run `policy.process(&mut writer)`.
3. Open a writer if needed.
4. Encode the record.
5. Flush the writer.

The current `flush()` implementation is empty by design. This follows the referenced log4rs-style behavior used by this crate, and should not be treated as a bug unless requirements change.

## LogManager

`LogManager` is the internal unit that owns mutable log-file state.

It contains:

- `writer: Mutex<Option<LogWriter>>`
- `policy: Box<dyn Policy>`

This separates file state from the log4rs appender object.

This split matters for `ThreadRollingFileAppender`: thread-specific output can keep per-thread `LogManager` state without constructing a full `RollingFileAppender` for every thread.

## LogWriter

`LogWriter` wraps the active file writer.

It contains:

- `file: BufWriter<File>`
- `path: String`
- `len: usize`

`len` is updated by the `Write` implementation and is used by size triggers.

The `path` is captured before rolling so the roller can rename or compress the active file after the writer is dropped.

## ThreadRollingFileAppender

`ThreadRollingFileAppender` implements `Append` and splits records by current thread name.

It owns:

- `managers: RwLock<HashMap<String, LogManager>>`
- shared `log_dir`, `prefix`, `option`, and encoder

The key is:

```rust
format!(
    "{}.{}",
    self.prefix,
    sanitize_file_part(std::thread::current().name().unwrap_or("unnamed"))
)
```

For example:

```text
STEM_AG_KFK.worker-1
```

Each thread key gets its own `LogManager`, so each thread keeps independent writer and policy state.

Current append flow first ensures a manager exists for the thread key, then reads the manager map and locks only that manager's writer for rolling and writing.

Manager creation uses a read-then-write pattern:

1. Acquire a read lock and return early if the key exists.
2. Drop the read lock.
3. Acquire a write lock.
4. Check the key again because another thread may have inserted it while the write lock was pending.
5. Insert a new `LogManager` only if the key is still missing.

The thread name portion is sanitized so only ASCII alphanumeric characters, `-`, and `_` remain. Other characters are replaced with `_`.

## File Path Rules

`get_path(log_dir, prefix, kind)` chooses the active log file path.

For time and compound policies:

```text
{log_dir}/{prefix}.{interval}.log
```

Example:

```text
/home/stem/log/STEM_AG_KFK.2026-06-15.log
```

For size-only policies:

```text
{log_dir}/{prefix}.log
```

Thread logging changes `prefix` by adding the thread key, so a thread file can become:

```text
/home/stem/log/STEM_AG_KFK.worker-1.2026-06-15.log
```

## Compound Policy

Policy code lives under `src/rolling_file/policy/compound`.

`CompoundPolicyOption` is caller-facing policy configuration.

It contains:

- `kind: CompoundPolicyOptionKind`
- `compression: Compression`

Policy kinds:

- `Time(CompoundPolicyTimeOption)`
- `Size(CompoundPolicySizeOption)`
- `Compound(CompoundPolicyTimeOption, CompoundPolicySizeOption)`

`CompoundPolicyConfig::from_option` converts caller-facing options into runtime trigger and roller config.

`CompoundPolicy` owns:

- `trigger: Box<dyn Trigger>`
- `roller: Box<dyn Roll>`

On `process`:

1. If no writer exists, do nothing.
2. Ask the trigger whether rolling is needed.
3. If rolling is needed, clone the current path.
4. Set writer to `None` so the file is closed.
5. Call the roller with the path and trigger kind.

## Triggers

Triggers live under `src/rolling_file/policy/compound/trigger`.

`Trigger::trigger(&self, writer: &LogWriter) -> Option<TriggerKind>` returns:

- `None` if no roll is needed
- `Some(TriggerKind::Time)` for time-based rolling
- `Some(TriggerKind::Size)` for size-based rolling

`TimeTrigger` stores `next_roll_time` in an `AtomicI64`.

`SizeTrigger` compares `LogWriter::len_estimate()` with the configured size limit.

`CompoundTrigger` checks time first, then size.

## FixedWindowRoller

`FixedWindowRoller` lives in `src/rolling_file/policy/compound/roll/fixed_window.rs`.

It handles:

- time retention cleanup
- size-based fixed-window rotation
- optional compression mode

Time rolling deletes files under `log_dir` whose names start with `prefix` and whose creation time is older than the retention cutoff.

Retention intentionally uses file creation metadata plus a rounded current time. Log files are created when the first record is written after a roll boundary, not exactly at the boundary. This design keeps files that were created late in an interval from being retained much longer than the configured interval count.

Size rolling uses syslog/log4rs-like fixed-window rotation:

```text
active.log.(count - 1) is removed
active.log.0 -> active.log.1
active.log.1 -> active.log.2
active.log -> active.log.0
```

For example:

```text
/home/stem/log/STEM_AG_KFK.2026-06-15.log
/home/stem/log/STEM_AG_KFK.2026-06-15.log.0
/home/stem/log/STEM_AG_KFK.2026-06-15.log.1
```

`Compression::None` moves the file. `Gzip` and `Zstd` write a compressed destination and remove the source.

## Important Constraints

- There can be only one process-wide logger installed through the `log` facade.
- The first `log_init_with_options` call wins because `LOG_HANDLE` is a `OnceLock`.
- Module filtering is implemented with log4rs `Logger` entries, not inside appenders.
- Thread file splitting is implemented inside `ThreadRollingFileAppender`.
- Each thread key must keep independent `LogManager` state because time triggers are stateful.
- Rolling closes the writer before renaming the active file.

## Known Review Points

- `TimeTrigger::get_next_time` currently uses calendar fields directly. Month/day/minute overflow behavior should be reviewed for edge cases.

## Intentional Decisions

- `RollingFileAppender::flush()` and `ThreadRollingFileAppender::flush()` are intentionally kept empty, matching the referenced log4rs-style implementation.
- `get_writer()` uses `unwrap()` after inserting a writer. This is intentionally retained because the control flow guarantees the writer exists at that point, and it matches the referenced log4rs-style implementation.
- File ownership is hardcoded with `fchown(..., Some(1000), Some(1000))`. This is intentional for the target deployment systems and should not be generalized unless the deployment assumption changes.
- Time retention intentionally uses file creation time metadata because active log files are created on first write after a roll boundary, not at the exact boundary time.

## Verification

Do not run `cargo fmt` unless explicitly requested.

Use narrow checks when changing this crate:

```sh
cargo check -p rlog
cargo test -p rlog
```
