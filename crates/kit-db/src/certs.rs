//! Mozilla CA bundle for Turso's sync TLS.
//!
//! Turso's sync IO thread calls `hyper_rustls::with_native_roots()`, which
//! panics on Android (no system CA store). Point rustls-native-certs at a
//! Mozilla PEM bundle via `SSL_CERT_FILE` before any synced DB is opened.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;

use crate::error::Result;

/// The env var rustls-native-certs reads.
pub const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";

/// Name of the bundle written under the data dir.
pub const CA_FILE_NAME: &str = "cacert.pem";

/// Write `cacert.pem` under `dir`, then set `SSL_CERT_FILE` to it.
///
/// Takes the data dir, not an `AppHandle`: a cold Android background job has
/// none.
///
/// **Process-global hazard.** This sets an env var that every later Turso
/// sync IO thread reads. Call it once, from the app's single state open,
/// before any synced DB is opened and before other threads start; never from
/// a test (use [`write_mozilla_ca_file`]). A missing or empty file there makes
/// Turso's IO thread panic and every sync future hang.
pub fn install_mozilla_ca_file_in(dir: &Path) -> Result<PathBuf> {
    let path = write_mozilla_ca_file(dir)?;
    // SAFETY: see the hazard above. The app calls this once at startup, before
    // it spawns threads that read the environment.
    unsafe {
        std::env::set_var(SSL_CERT_FILE_ENV, &path);
    }
    tracing::info!(path = %path.display(), "SSL_CERT_FILE set for Turso TLS");
    Ok(path)
}

/// Write `cacert.pem` under `dir` without touching the environment.
///
/// Written to a temp file next to it, synced to disk, then renamed over it,
/// so a reader (a sync IO thread, or another process such as a background
/// job) never sees a truncated or half-written bundle.
pub fn write_mozilla_ca_file(dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(CA_FILE_NAME);
    let temp = dir.join(temp_name());
    write_synced(&temp, mozilla_root_pem().as_bytes())
        .and_then(|()| std::fs::rename(&temp, &path))
        .inspect_err(|_| {
            // Best effort: don't leave the temp file behind.
            let _cleanup = std::fs::remove_file(&temp);
        })?;
    Ok(path)
}

/// `cacert.pem.<pid>.<n>.tmp`: unique per call, so concurrent writers (other
/// processes, or other threads of this one) never share a temp file.
fn temp_name() -> String {
    static CALLS: AtomicU64 = AtomicU64::new(0);
    let call = CALLS.fetch_add(1, Ordering::Relaxed);
    format!("{CA_FILE_NAME}.{}.{call}.tmp", std::process::id())
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn mozilla_root_pem() -> String {
    let mut out = String::new();
    for cert in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
        write_pem_cert(&mut out, cert.as_ref());
    }
    out
}

fn write_pem_cert(out: &mut String, der: &[u8]) {
    out.push_str("-----BEGIN CERTIFICATE-----\n");
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    // Base64 is ASCII, so each byte is one char: no UTF-8 check needed.
    for line in encoded.as_bytes().chunks(64) {
        out.extend(line.iter().copied().map(char::from));
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
}

#[cfg(test)]
mod tests;
