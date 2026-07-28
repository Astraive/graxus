use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::context::CliContext;

/// Debounce state that collects file change events and triggers after a quiet period.
struct Debouncer {
    /// Duration of quiet period before triggering.
    debounce_duration: Duration,
    /// Timestamp of the last received event.
    last_event_time: Option<Instant>,
    /// Set of changed file paths during the debounce window.
    pending_files: HashSet<PathBuf>,
    /// Whether a trigger is pending (we've seen events but haven't triggered yet).
    trigger_pending: bool,
}

impl Debouncer {
    fn new(debounce_duration: Duration) -> Self {
        Self {
            debounce_duration,
            last_event_time: None,
            pending_files: HashSet::new(),
            trigger_pending: false,
        }
    }

    /// Record a file change event. Returns None if still debouncing.
    /// Returns Some(files) when the debounce period has elapsed since the last event.
    fn record(&mut self, path: PathBuf) -> Option<&HashSet<PathBuf>> {
        self.record_at(path, Instant::now())
    }

    fn record_at(&mut self, path: PathBuf, event_time: Instant) -> Option<&HashSet<PathBuf>> {
        self.pending_files.insert(path);
        self.last_event_time = Some(event_time);
        self.trigger_pending = true;
        None
    }

    /// Check if the debounce period has elapsed and we should trigger.
    /// Returns Some(files) if ready, None if still waiting.
    fn check_trigger(&mut self) -> Option<&HashSet<PathBuf>> {
        self.check_trigger_at(Instant::now())
    }

    fn check_trigger_at(&mut self, now: Instant) -> Option<&HashSet<PathBuf>> {
        if !self.trigger_pending {
            return None;
        }
        if let Some(last) = self.last_event_time {
            if now.duration_since(last) >= self.debounce_duration {
                self.trigger_pending = false;
                self.last_event_time = None;
                return Some(&self.pending_files);
            }
        }
        None
    }

    /// Clear pending files after a successful trigger.
    fn clear(&mut self) {
        self.pending_files.clear();
        self.trigger_pending = false;
        self.last_event_time = None;
    }
}

/// Check if a file path is relevant for indexing.
fn is_relevant_path(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();
    // Normalize separators for cross-platform matching
    let normalized = path_str.replace('\\', "/");
    if normalized.contains(".graxus/")
        || normalized.contains("/target/")
        || normalized.contains("/node_modules/")
        || normalized.contains("/.git/")
        || normalized.contains("/vendor/")
        || normalized.starts_with("target/")
        || normalized.starts_with("node_modules/")
        || normalized.starts_with(".git/")
        || normalized.starts_with("vendor/")
    {
        return false;
    }
    path_str.ends_with(".rs")
        || path_str.ends_with(".ts")
        || path_str.ends_with(".tsx")
        || path_str.ends_with(".js")
        || path_str.ends_with(".jsx")
        || path_str.ends_with(".go")
        || path_str.ends_with(".py")
        || path_str.ends_with(".md")
        || path_str.ends_with(".mdx")
}

/// Watch for file changes and automatically re-index the project.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `debounce_ms` - Debounce interval in milliseconds between re-index runs
pub fn run(ctx: &CliContext, debounce_ms: u64) -> Result<()> {
    let root = ctx.resolve_root()?;

    println!("{}", "=== Watch Mode ===".green().bold());
    println!("  Watching for changes in: {}", root.display());
    println!("  Debounce: {}ms", debounce_ms);
    println!("  Press Ctrl+C to stop");
    println!();

    // Use notify crate for filesystem events
    use notify::{Event, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;

    watcher.watch(&root, RecursiveMode::Recursive)?;

    let debounce_duration = Duration::from_millis(debounce_ms);
    let mut debouncer = Debouncer::new(debounce_duration);
    let mut consecutive_failures: u32 = 0;
    let max_failures: u32 = 3;
    let failure_cooldown = Duration::from_secs(30);

    // Set up Ctrl+C handler
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        eprintln!("\n{} Shutting down watch mode...", "⊙".cyan());
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    }) {
        tracing::warn!("Could not set Ctrl+C handler: {}", e);
    }

    println!("  {} Watching for changes... (Ctrl+C to stop)", "👁".cyan());

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        // Drain all available events
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(event)) => {
                    for path in &event.paths {
                        if is_relevant_path(path) {
                            debouncer.record(path.clone());
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Watch error: {}", e);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Ok(());
                }
            }
        }

        // Check if debounce period has elapsed
        if let Some(changed_files) = debouncer.check_trigger() {
            // Check if we're in failure cooldown
            if consecutive_failures >= max_failures {
                eprintln!(
                    "{} Too many failures ({}), pausing for {}s...",
                    "⚠".yellow(),
                    consecutive_failures,
                    failure_cooldown.as_secs()
                );
                std::thread::sleep(failure_cooldown);
                consecutive_failures = 0;
            }

            let file_list: Vec<_> = changed_files
                .iter()
                .filter_map(|p| p.strip_prefix(&root).ok().map(|r| r.to_path_buf()))
                .collect();

            println!(
                "\n{} Change detected ({} files), re-indexing...",
                "⟳".cyan(),
                file_list.len()
            );
            if file_list.len() <= 10 {
                for f in &file_list {
                    println!("    {}", f.display());
                }
            } else {
                for f in file_list.iter().take(5) {
                    println!("    {}", f.display());
                }
                println!("    ... and {} more", file_list.len() - 5);
            }

            match super::index::run(
                ctx,
                false,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                "ripex".to_string(),
            ) {
                Ok(_) => {
                    println!("{} Re-index complete.", "✓".green());
                    consecutive_failures = 0;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    eprintln!(
                        "{} Re-index failed ({}/{}): {}",
                        "✗".red(),
                        consecutive_failures,
                        max_failures,
                        e
                    );
                }
            }

            debouncer.clear();

            println!(
                "\n  {} Watching for changes... (Ctrl+C to stop)",
                "👁".cyan()
            );
        }
    }

    println!("{} Watch mode stopped.", "✓".green());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debouncer_triggers_after_quiet_period() {
        let mut debouncer = Debouncer::new(Duration::from_millis(50));
        let start = Instant::now();

        debouncer.record_at(PathBuf::from("src/main.rs"), start);
        assert!(debouncer
            .check_trigger_at(start + Duration::from_millis(49))
            .is_none());

        let files = debouncer
            .check_trigger_at(start + Duration::from_millis(50))
            .unwrap();
        assert!(files.contains(&PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_debouncer_resets_on_new_event() {
        let mut debouncer = Debouncer::new(Duration::from_millis(100));
        let start = Instant::now();

        debouncer.record_at(PathBuf::from("src/a.rs"), start);
        debouncer.record_at(PathBuf::from("src/b.rs"), start + Duration::from_millis(60));

        assert!(debouncer
            .check_trigger_at(start + Duration::from_millis(159))
            .is_none());

        let files = debouncer
            .check_trigger_at(start + Duration::from_millis(160))
            .unwrap();
        assert!(files.contains(&PathBuf::from("src/a.rs")));
        assert!(files.contains(&PathBuf::from("src/b.rs")));
    }

    #[test]
    fn test_debouncer_collects_multiple_files() {
        let mut debouncer = Debouncer::new(Duration::from_millis(50));
        let start = Instant::now();

        debouncer.record_at(PathBuf::from("src/a.rs"), start);
        debouncer.record_at(PathBuf::from("src/b.rs"), start);
        debouncer.record_at(PathBuf::from("src/c.rs"), start);

        let files = debouncer
            .check_trigger_at(start + Duration::from_millis(50))
            .unwrap();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_debouncer_clear_resets_state() {
        let mut debouncer = Debouncer::new(Duration::from_millis(50));

        debouncer.record(PathBuf::from("src/a.rs"));
        debouncer.clear();

        assert!(debouncer.check_trigger().is_none());
        assert!(debouncer.pending_files.is_empty());
    }

    #[test]
    fn test_is_relevant_path() {
        assert!(is_relevant_path(std::path::Path::new("src/main.rs")));
        assert!(is_relevant_path(std::path::Path::new("docs/readme.md")));
        assert!(is_relevant_path(std::path::Path::new("app.tsx")));
        assert!(!is_relevant_path(std::path::Path::new(
            ".graxus/index.json"
        )));
        assert!(!is_relevant_path(std::path::Path::new("target/debug/main")));
        assert!(!is_relevant_path(std::path::Path::new(
            "node_modules/foo/index.js"
        )));
        assert!(!is_relevant_path(std::path::Path::new(".git/config")));
        assert!(!is_relevant_path(std::path::Path::new("image.png")));
    }

    #[test]
    fn test_is_relevant_path_go() {
        assert!(is_relevant_path(std::path::Path::new("cmd/main.go")));
        assert!(!is_relevant_path(std::path::Path::new("vendor/pkg/mod.go")));
    }
}
