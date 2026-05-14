//! Thin wrappers around `std::io::IsTerminal` so command handlers can guard
//! against accidental TTY output of secret values without re-importing the
//! trait everywhere.

use std::io::IsTerminal;

pub fn is_stdin_tty() -> bool {
    std::io::stdin().is_terminal()
}

pub fn is_stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}
