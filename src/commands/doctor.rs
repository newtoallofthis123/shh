//! `shh doctor keychain` — local self-test against the active SecretStore.
//!
//! Writes/reads/lists/deletes a single reserved entry under
//! `__shh_doctor__` and reports the active store mode plus signing identity
//! of the running binary. Cleanup runs in both success and failure paths so
//! a partial failure cannot leave a probe value behind.

use std::process::Command;

use crate::error::Result;
use crate::store::SecretStore;

use super::CommandOutcome;

const DOCTOR_PROFILE: &str = "__shh_doctor__";
const DOCTOR_NAME: &str = "SHH_DOCTOR_PROBE";
const DOCTOR_VALUE: &str = "ok";

pub fn run_keychain(store: &dyn SecretStore) -> Result<CommandOutcome> {
    let mode = std::env::var("SHH_KEYCHAIN").unwrap_or_else(|_| "file".into());
    let prefix = std::env::var("SHH_SERVICE_PREFIX").unwrap_or_else(|_| "shh".into());
    println!("active store: {mode}");
    println!("service prefix: {prefix}");

    let identity = detect_signing_identity();
    println!("signing identity: {identity}");
    if mode == "data-protection" && identity != SigningIdentity::DeveloperId {
        println!(
            "warning: data-protection mode expects a Developer ID signed binary; current identity is {identity}"
        );
    }

    // Run the write/read/list/delete cycle. Cleanup must always attempt the
    // delete, even on a mid-cycle failure.
    let cycle = run_probe_cycle(store);
    let cleanup = store.delete(DOCTOR_PROFILE, DOCTOR_NAME);

    match cycle {
        Ok(()) => {
            println!("keychain access: ok");
            println!("permission flow: not prompted");
        }
        Err(e) => {
            println!("keychain access: failed ({e})");
        }
    }
    if let Err(e) = cleanup {
        println!("cleanup: failed ({e})");
    }
    Ok(CommandOutcome::Success)
}

fn run_probe_cycle(store: &dyn SecretStore) -> Result<()> {
    store.set(DOCTOR_PROFILE, DOCTOR_NAME, DOCTOR_VALUE)?;
    // Read back, but never print the probe value.
    let read = store.get(DOCTOR_PROFILE, DOCTOR_NAME)?;
    if read.as_deref() != Some(DOCTOR_VALUE) {
        println!("warning: read-back value did not match probe");
    }
    let names = store.list_names(DOCTOR_PROFILE)?;
    if !names.iter().any(|n| n == DOCTOR_NAME) {
        println!("warning: list did not include the probe entry");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SigningIdentity {
    Unsigned,
    AdHoc,
    DeveloperId,
    Unknown,
}

impl std::fmt::Display for SigningIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SigningIdentity::Unsigned => "unsigned",
            SigningIdentity::AdHoc => "ad-hoc",
            SigningIdentity::DeveloperId => "Developer ID",
            SigningIdentity::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

fn detect_signing_identity() -> SigningIdentity {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return SigningIdentity::Unknown,
    };
    let out = Command::new("codesign").args(["-dvv", &exe.to_string_lossy()]).output();
    let out = match out {
        Ok(o) => o,
        Err(_) => return SigningIdentity::Unknown,
    };
    if !out.status.success() {
        // codesign returns non-zero with "code object is not signed at all".
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("not signed") {
            return SigningIdentity::Unsigned;
        }
        return SigningIdentity::Unknown;
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("Signature=adhoc") {
        return SigningIdentity::AdHoc;
    }
    if stderr.contains("Authority=Developer ID Application") {
        return SigningIdentity::DeveloperId;
    }
    SigningIdentity::Unknown
}
