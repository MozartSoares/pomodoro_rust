use clap::{Parser, Subcommand};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::SystemTime;

use pomodoro_cli::{
    current_status,
    run_session_loop,
    start_timer,
    stop_timer,
    PomodoroError,
    SessionOutcome,
    Status,
};

#[derive(Parser, Debug)]
#[command(name = "pomodoro")] 
#[command(about = "A simple Pomodoro timer CLI with automatic session logs.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start a new Pomodoro for the given number of minutes
    Start {
        /// Duration in minutes (must be > 0)
        minutes: u64,
        /// Optional note to include in logs
        #[arg(long)]
        note: Option<String>,
        /// force a new active session if one exists
        #[arg(long)]
        force: bool,
    },
    /// Show status of current Pomodoro
    Status,
    /// Stop the current Pomodoro and mark it as canceled when appropriate
    Stop,
}

fn main() {
    let cli = Cli::parse();
    let now = SystemTime::now();

    match cli.command {
        Commands::Start { minutes, note, force } => {
            match start_timer(now, minutes, note, force) {
                Ok(mut state) => {
                    println!(
                        "Started Pomodoro: {} minutes (ends at {})",
                        state.minutes,
                        chrono::DateTime::<chrono::Utc>::from(
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(state.end_unix as u64)
                        )
                        .to_rfc3339()
                    );
                    if let Some(ref note) = state.note {
                        println!("Note: {}", note);
                    }
                    println!("Log file: {}", state.log_path.display());
                    println!("Press Ctrl+C to cancel this session.");

                    let cancel_flag = Arc::new(AtomicBool::new(false));
                    {
                        let handler_flag = Arc::clone(&cancel_flag);
                        ctrlc::set_handler(move || {
                            handler_flag.store(true, Ordering::SeqCst);
                        })
                        .expect("failed to set Ctrl+C handler");
                    }

                    let outcome = match run_session_loop(&mut state, {
                        let loop_flag = Arc::clone(&cancel_flag);
                        move || loop_flag.load(Ordering::SeqCst)
                    }) {
                        Ok(outcome) => outcome,
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(2);
                        }
                    };

                    match outcome {
                        SessionOutcome::Completed => {
                            println!(
                                "Session completed! Log: {}",
                                state.log_path.display()
                            );
                        }
                        SessionOutcome::Canceled => {
                            println!(
                                "Session canceled. Log: {}",
                                state.log_path.display()
                            );
                        }
                    }
                }
                Err(PomodoroError::ActiveSessionRunning { remaining_secs }) => {
                    let minutes = remaining_secs / 60;
                    let seconds = remaining_secs % 60;
                    println!("A Pomodoro session is already running (about {minutes}m{seconds}s remaining). Use --force to overwrite.");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(2);
                }
            }
        }
        Commands::Status => match current_status(now) {
            Ok(Status::NoActive) => println!("No active Pomodoro."),
            Ok(Status::Running { elapsed_secs, remaining_secs }) => {
                println!(
                    "Running. Elapsed: {}m{}s, Remaining: {}m{}s",
                    elapsed_secs / 60,
                    elapsed_secs % 60,
                    remaining_secs / 60,
                    remaining_secs % 60
                );
            }
            Ok(Status::Completed { over_secs, just_logged }) => {
                println!(
                    "Completed. Finished {}m{}s ago.{}",
                    over_secs / 60,
                    over_secs % 60,
                    if just_logged { " Log entry recorded." } else { "" }
                );
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(2);
            }
        },
        Commands::Stop => match stop_timer() {
            Ok(()) => println!("Stopped and cleared active Pomodoro."),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(2);
            }
        },
    }
}
