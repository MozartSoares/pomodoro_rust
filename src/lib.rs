use chrono::{DateTime, Local, Utc}; // deal with timestamps
use serde::{Deserialize, Serialize};// deal with json structs (or ohter formats, for this app only json)
use std::fs::{self, File}; // file I/O
use std::io::{Read, Write};// file I/O
use std::path::{Path, PathBuf}; // paths
use std::time::{Duration, SystemTime, UNIX_EPOCH}; // deal with duration and timestamps
use thiserror::Error; //Make error handling easier

// one json file per completed Pomodoro will be created inside the data dir.

// declaring the errors the application can throw //
#[derive(Error, Debug)]
pub enum PomodoroError {
    #[error("invalid duration: must be > 0 minutes")] 
    InvalidDuration,
    #[error("a pomodoro is already running with approximately {remaining_secs} seconds remaining. Use --force to overwrite.")]
    ActiveSessionRunning { remaining_secs: u64 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

// Main app struct //
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PomodoroState {
    pub start_unix: i64,
    pub end_unix: i64,
    pub minutes: u64,
    pub note: Option<String>,
    #[serde(rename = "log_file")]
    pub log_path: PathBuf,
    #[serde(default, alias = "logged")]
    pub completed_logged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionLog {
    minutes: u64,
    note: Option<String>,
    started_at: String,
    ends_at: String,
    completed: bool,
    completed_at: Option<String>,
    canceled: bool,
    canceled_at: Option<String>,
}

impl PomodoroState {
    pub fn is_complete_at(&self, now: SystemTime) -> bool {
        let now_unix = system_time_to_unix(now);
        now_unix >= self.end_unix
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    NoActive,
    Running { elapsed_secs: u64, remaining_secs: u64 },
    Completed { over_secs: u64, just_logged: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOutcome {
    Completed,
    Canceled,
}

// datetime helpers //
fn system_time_to_unix(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn unix_to_system_time(unix: i64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_secs(unix as u64)
}

fn format_timestamp(time: SystemTime) -> String {
    let utc: DateTime<Utc> = time.into();
    utc.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string()
}


// Path / filesystem helpers //
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn app_root_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn data_dir() -> PathBuf {
    app_root_dir().join("data")
}

fn ensure_data_dir() -> Result<PathBuf, PomodoroError> {
    let dir = data_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn state_path() -> Result<PathBuf, PomodoroError> {
    let dir = ensure_data_dir()?;
    Ok(dir.join("state.json"))
}


fn resolve_session_log_path(note: &Option<String>, start_unix: i64) -> Result<PathBuf, PomodoroError> {
    let dir = ensure_data_dir()?;
    let sanitized = note
        .as_ref()
        .map(|n| sanitize_filename(n))
        .filter(|s| !s.is_empty());

    let timestamp = {
        let ts: DateTime<Utc> = unix_to_system_time(start_unix).into();
        ts.format("%Y%m%dT%H%M%SZ").to_string()
    };

    let base_name = sanitized.unwrap_or_else(|| timestamp.clone());
    let mut candidate = dir.join(format!("{}.json", base_name));
    if candidate.exists() {
        candidate = dir.join(format!("{}-{}.json", base_name, timestamp));
    }
    Ok(candidate)
}

// log handling //
fn initialize_session_log(
    path: &Path,
    minutes: u64,
    note: &Option<String>,
    start_time: SystemTime,
    end_time: SystemTime,
) -> Result<(), PomodoroError> {
    let log = SessionLog {
        minutes,
        note: note.clone(),
        started_at: format_timestamp(start_time),
        ends_at: format_timestamp(end_time),
        completed: false,
        completed_at: None,
        canceled: false,
        canceled_at: None,
    };
    write_session_log(path, &log)
}

fn read_session_log(path: &Path) -> Result<SessionLog, PomodoroError> {
    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;
    Ok(serde_json::from_str(&contents)?)
}

fn write_session_log(path: &Path, log: &SessionLog) -> Result<(), PomodoroError> {
    let json = serde_json::to_string_pretty(log)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

fn mark_log_completed(path: &Path, completed_at: SystemTime) -> Result<(), PomodoroError> {
    let mut log = read_session_log(path)?;
    if log.completed {
        return Ok(());
    }
    log.completed = true;
    log.completed_at = Some(format_timestamp(completed_at));
    log.canceled = false;
    log.canceled_at = None;
    write_session_log(path, &log)
}

fn mark_log_canceled(path: &Path, canceled_at: SystemTime) -> Result<(), PomodoroError> {
    let mut log = read_session_log(path)?;
    if log.canceled {
        return Ok(());
    }
    log.canceled = true;
    log.canceled_at = Some(format_timestamp(canceled_at));
    log.completed = false;
    log.completed_at = None;
    write_session_log(path, &log)
}


//state handling helpers
fn load_state() -> Result<Option<PomodoroState>, PomodoroError> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let mut s = String::new();
    File::open(&path)?.read_to_string(&mut s)?;
    let state: PomodoroState = serde_json::from_str(&s)?;
    Ok(Some(state))
}

fn save_state(state: &PomodoroState) -> Result<(), PomodoroError> {
    let path = state_path()?;
    let json = serde_json::to_string_pretty(state)?;
    let mut f = File::create(&path)?;
    f.write_all(json.as_bytes())?;
    Ok(())
}

fn remove_state() -> Result<(), PomodoroError> {
    let path = state_path()?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn finalize_completed_session(state: &mut PomodoroState, now: SystemTime) -> Result<bool, PomodoroError> {
    if state.completed_logged {
        return Ok(false);
    }
    if !state.is_complete_at(now) {
        return Ok(false);
    }
    mark_log_completed(&state.log_path, now)?;
    state.completed_logged = true;
    save_state(state)?;
    Ok(true)
}

pub fn start_timer(
    now: SystemTime,
    minutes: u64,
    note: Option<String>,
    force: bool,
) -> Result<PomodoroState, PomodoroError> {
    if minutes == 0 {
        return Err(PomodoroError::InvalidDuration);
    }

    let now_unix = system_time_to_unix(now);

    if let Some(mut existing) = load_state()? {
        let _ = finalize_completed_session(&mut existing, now)?;
        let still_running = now_unix < existing.end_unix;
        if still_running && !force {
            let remaining = if existing.end_unix > now_unix {
                (existing.end_unix - now_unix) as u64
            } else {
                0
            };
            return Err(PomodoroError::ActiveSessionRunning { remaining_secs: remaining });
        }
        if still_running && force {
            mark_log_canceled(&existing.log_path, now)?;
        }
    }

    let start_unix = now_unix;
    let end_unix = start_unix + (minutes as i64) * 60;
    let log_path = resolve_session_log_path(&note, start_unix)?;
    let end_time = unix_to_system_time(end_unix);
    initialize_session_log(&log_path, minutes, &note, now, end_time)?;

    let state = PomodoroState {
        start_unix,
        end_unix,
        minutes,
        note,
        log_path: log_path.clone(),
        completed_logged: false,
    };
    save_state(&state)?;
    Ok(state)
}

pub fn run_session_loop<F>(
    state: &mut PomodoroState,
    mut should_cancel: F,
) -> Result<SessionOutcome, PomodoroError>
where
    F: FnMut() -> bool,
{
    loop {
        let now = SystemTime::now();
        let now_unix = system_time_to_unix(now);

        if should_cancel() {
            let remaining = if state.end_unix > now_unix {
                (state.end_unix - now_unix) as u64
            } else {
                0
            };
            let minutes = remaining / 60;
            let seconds = remaining % 60;
            print!("\rTime remaining: {:02}m{:02}s (canceling)", minutes, seconds);
            std::io::stdout().flush()?;
            println!();
            mark_log_canceled(&state.log_path, now)?;
            remove_state()?;
            return Ok(SessionOutcome::Canceled);
        }

        if now_unix >= state.end_unix {
            finalize_completed_session(state, now)?;
            print!("\rTime remaining: 00m00s");
            std::io::stdout().flush()?;
            println!();
            remove_state()?;
            return Ok(SessionOutcome::Completed);
        }

        let remaining = (state.end_unix - now_unix) as u64;
        let minutes = remaining / 60;
        let seconds = remaining % 60;
        print!("\rTime remaining: {:02}m{:02}s", minutes, seconds);
        std::io::stdout().flush()?;
        std::thread::sleep(Duration::from_secs(1));
    }
}

pub fn current_status(now: SystemTime) -> Result<Status, PomodoroError> {
    let mut state = match load_state()? {
        None => return Ok(Status::NoActive),
        Some(s) => s,
    };
    let now_unix = system_time_to_unix(now);
    if now_unix < state.end_unix {
        let elapsed = now_unix.saturating_sub(state.start_unix) as u64;
        let total = (state.end_unix - state.start_unix) as u64;
        let remaining = total.saturating_sub(elapsed);
        Ok(Status::Running { elapsed_secs: elapsed, remaining_secs: remaining })
    } else {
        let was_logged = finalize_completed_session(&mut state, now)?;
        let over = (now_unix - state.end_unix) as u64;
        Ok(Status::Completed { over_secs: over, just_logged: was_logged })
    }
}

pub fn stop_timer() -> Result<(), PomodoroError> {
    if let Some(mut state) = load_state()? {
        let now = SystemTime::now();
        if state.is_complete_at(now) {
            let _ = finalize_completed_session(&mut state, now)?;
        } else {
            mark_log_canceled(&state.log_path, now)?;
        }
    }
    remove_state()
}
