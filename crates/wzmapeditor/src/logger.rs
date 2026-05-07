//! Custom `log::Log` implementation that tees to stderr, a file, and the
//! Output panel.
//!
//! The file/stderr sink is a standard [`env_logger`] pipeline (Info default,
//! `wgpu_*`/`naga` at Error, `RUST_LOG` respected). An extra sink forwards
//! warnings and errors from the editor workspace crates (`wzmapeditor`,
//! `wz_maplib`, `wz_pie`, `wz_stats`) into the Output panel so editor
//! problems surface in-app while third-party noise stays on disk.

use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::Sender;

use crate::app::output_log::{LogEntry, LogSeverity, LogSource};

/// Crate prefixes whose warnings and errors are mirrored into the Output panel.
///
/// Matched against `log::Record::target()`, which by default is the emitting
/// module path. Entries are matched as an exact equality or as a prefix of
/// the form `"<prefix>::"`.
const EDITOR_CRATE_PREFIXES: &[&str] = &["wzmapeditor", "wz_maplib", "wz_pie", "wz_stats"];

/// Tees log lines to stderr (visible in dev terminals) and the on-disk log
/// file (useful on release builds where the console is hidden).
struct DualWriter<A: std::io::Write, B: std::io::Write>(A, B);

impl<A: std::io::Write, B: std::io::Write> std::io::Write for DualWriter<A, B> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.0.write_all(buf);
        self.1.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = self.0.flush();
        self.1.flush()
    }
}

struct EditorLogger {
    inner: env_logger::Logger,
    panel_tx: Mutex<Sender<LogEntry>>,
}

impl log::Log for EditorLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.inner.enabled(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        self.inner.log(record);

        if let Some(severity) = classify_for_panel(record) {
            let entry = LogEntry::new(severity, LogSource::Internal, record.args().to_string());
            if let Ok(tx) = self.panel_tx.lock() {
                let _ = tx.send(entry);
            }
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

fn classify_for_panel(record: &log::Record<'_>) -> Option<LogSeverity> {
    let target = record.target();
    let is_internal = EDITOR_CRATE_PREFIXES
        .iter()
        .any(|prefix| target == *prefix || target.starts_with(&format!("{prefix}::")));
    if !is_internal {
        return None;
    }
    match record.level() {
        log::Level::Error => Some(LogSeverity::Error),
        log::Level::Warn => Some(LogSeverity::Warn),
        // Info from internal crates is not mirrored: curated Info entries
        // already reach the panel via `EditorApp::log()`.
        _ => None,
    }
}

/// Install the global logger. Writes to `log_path` + stderr, and tees
/// internal-crate warnings and errors to `panel_tx`.
///
/// If the log file cannot be created, logging falls back to stderr only.
pub fn init(log_path: &Path, panel_tx: Sender<LogEntry>) -> Result<(), log::SetLoggerError> {
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let target: Box<dyn std::io::Write + Send> = match std::fs::File::create(log_path) {
        Ok(file) => Box::new(DualWriter(std::io::stderr(), file)),
        Err(_) => Box::new(std::io::stderr()),
    };

    let inner = env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("naga", log::LevelFilter::Error)
        .parse_default_env()
        .target(env_logger::Target::Pipe(target))
        .build();

    let level = inner.filter();
    let logger = EditorLogger {
        inner,
        panel_tx: Mutex::new(panel_tx),
    };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_internal_warn_as_warn() {
        let record = log::Record::builder()
            .args(format_args!("boom"))
            .level(log::Level::Warn)
            .target("wz_maplib::validate")
            .build();
        assert_eq!(classify_for_panel(&record), Some(LogSeverity::Warn));
    }

    #[test]
    fn classifies_internal_info_as_none() {
        let record = log::Record::builder()
            .args(format_args!("hi"))
            .level(log::Level::Info)
            .target("wzmapeditor::app::map_io")
            .build();
        assert_eq!(classify_for_panel(&record), None);
    }

    #[test]
    fn classifies_third_party_error_as_none() {
        let record = log::Record::builder()
            .args(format_args!("gpu sad"))
            .level(log::Level::Error)
            .target("wgpu_core::device")
            .build();
        assert_eq!(classify_for_panel(&record), None);
    }
}
