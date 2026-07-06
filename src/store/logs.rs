//! Log-file storage for captured output, with head+tail truncation at the
//! storage cap (spec §21.4). Inputs are already redacted.

use crate::paths::CacheLayout;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct StoredLogs {
    pub stdout_path: Option<PathBuf>,
    pub stderr_path: Option<PathBuf>,
    pub normalized_path: Option<PathBuf>,
    /// True if any stream was truncated to the storage cap.
    pub truncated_raw: bool,
}

/// Write (already-redacted) stdout/stderr and, when provided, the normalized
/// text. Each raw stream is truncated head+tail if it exceeds `max_bytes`.
pub fn write_logs(
    layout: &CacheLayout,
    run_id: &str,
    stdout: &[u8],
    stderr: &[u8],
    normalized: Option<&str>,
    max_bytes: usize,
) -> std::io::Result<StoredLogs> {
    std::fs::create_dir_all(layout.logs_dir())?;
    let mut stored = StoredLogs::default();

    let (out_bytes, out_trunc) = truncate_head_tail(stdout, max_bytes);
    let out_path = layout.stdout_log(run_id);
    std::fs::write(&out_path, &out_bytes)?;
    stored.stdout_path = Some(out_path);

    let (err_bytes, err_trunc) = truncate_head_tail(stderr, max_bytes);
    let err_path = layout.stderr_log(run_id);
    std::fs::write(&err_path, &err_bytes)?;
    stored.stderr_path = Some(err_path);

    if let Some(norm) = normalized {
        let norm_path = layout.normalized_log(run_id);
        std::fs::write(&norm_path, norm)?;
        stored.normalized_path = Some(norm_path);
    }

    stored.truncated_raw = out_trunc || err_trunc;
    Ok(stored)
}

/// Keep the first and last `max/2` bytes, snapping cut points to newline
/// boundaries, with a marker between. Returns `(bytes, truncated)`.
fn truncate_head_tail(data: &[u8], max: usize) -> (Vec<u8>, bool) {
    if data.len() <= max || max == 0 {
        return (data.to_vec(), false);
    }
    let half = max / 2;
    let head_end = snap_back(data, half);
    let tail_start = snap_forward(data, data.len() - half);

    let head_bytes = head_end;
    let tail_bytes = data.len() - tail_start;
    let marker =
        format!("\n<TRUNCATED: dejavu stored first {head_bytes} and last {tail_bytes} bytes>\n");

    let mut out = Vec::with_capacity(head_end + marker.len() + (data.len() - tail_start));
    out.extend_from_slice(&data[..head_end]);
    out.extend_from_slice(marker.as_bytes());
    out.extend_from_slice(&data[tail_start..]);
    (out, true)
}

/// Largest index `<= idx` just after a newline (or `idx` if none nearby).
fn snap_back(data: &[u8], idx: usize) -> usize {
    match data[..idx.min(data.len())]
        .iter()
        .rposition(|&b| b == b'\n')
    {
        Some(pos) => pos + 1,
        None => idx.min(data.len()),
    }
}

/// Smallest index `>= idx` just after a newline (or `idx` if none nearby).
fn snap_forward(data: &[u8], idx: usize) -> usize {
    match data[idx.min(data.len())..].iter().position(|&b| b == b'\n') {
        Some(pos) => idx + pos + 1,
        None => idx.min(data.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_truncation_under_cap() {
        let (out, trunc) = truncate_head_tail(b"short output\n", 1024);
        assert!(!trunc);
        assert_eq!(out, b"short output\n");
    }

    #[test]
    fn truncates_over_cap_with_marker() {
        let data: Vec<u8> = (0..1000)
            .flat_map(|i| format!("line {i}\n").into_bytes())
            .collect();
        let (out, trunc) = truncate_head_tail(&data, 200);
        assert!(trunc);
        assert!(out.len() < data.len());
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("<TRUNCATED"));
        assert!(text.starts_with("line 0"));
        assert!(text.trim_end().ends_with("line 999"));
    }
}
