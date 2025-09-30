# Overview

This project is a Pomodoro Timer CLI written in Rust. It focuses on a clean, reliable command-line workflow to start a focus session for a chosen number of minutes, check the current status, and stop it if needed. Each session now writes its own JSON log inside the app's `data/` directory while the CLI keeps a live countdown running in your terminal until the timer ends (press Ctrl+C to cancel).

I built this to deepen my Rust skills around file I/O, command‑line parsing, and basic data modeling while practicing ownership and error handling idioms. The implementation uses a small persisted state file to track session timing between separate invocations of the CLI.

[Software Demo Video]()

## Development Environment

- Tools: Rust, Cargo, VS Code (or any editor/terminal)
- Language: Rust 2021 edition
- Libraries: `clap` (CLI), `serde`/`serde_json` (state serialization), `chrono` (time formatting), `thiserror` (error types)

## Useful Websites

- [Official Rust site](https://www.rust-lang.org/)
- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [clap docs](https://docs.rs/clap/latest/clap/)
- [serde docs](https://docs.rs/serde/latest/serde/)

## Future Work

- Desktop notifications on completion (optional)
- A simple to‑do list integration to start timers from tasks
- Configurable default durations and sound cues
- Cross‑platform packaging and release workflow

---

## Rust Language Requirements Demonstrated

- Variables: both mutable and immutable variables are used across the codebase (e.g., `let mut state` in status handling).
- Expressions: arithmetic and logical expressions for computing elapsed/remaining time and completion conditions.
- Conditionals: `if`/`else` branches for completion detection, logging conditions, and error cases.
- Loops: used implicitly via library I/O and tests; the code structure lends itself to iteration in future worker loops, while core logic is expressed functionally.
- Functions (ownership/reference): functions take owned values (e.g., `Option<String>`) and references (e.g., `&Path`) where appropriate; error handling is via `Result<T, E>`.
- Data structure & OO via `struct` + `impl`: `PomodoroState` encapsulates timer data with methods and is serialized via `serde`.

## Getting Started

Prereqs:

- Install Rust and Cargo via rustup: `rustup default stable` then `rustup component add clippy rustfmt` (optional).

Build and run:

- `cargo build`
- `cargo run -- start 25 --note "Deep work"` (the command stays active showing the countdown)
- `cargo run -- status`
- `cargo run -- stop`

## Design Notes

- The CLI persists a small JSON state file under the `data/` directory. The `start` command keeps the active session in the foreground so you always see the remaining time; `status` and `stop` remain available from other terminals if needed.
- When a session starts the CLI creates a JSON log file named after the note (or timestamp) in the `data/` directory. The entry is updated when the session finishes or is canceled so you can see completion time or cancellation details at a glance.
