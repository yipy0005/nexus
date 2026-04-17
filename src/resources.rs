use std::fs;

/// Thresholds for CPU usage relative to the cgroup quota.
/// Warning at 60%, Critical at 80%.
const WARN_PCT: f64 = 60.0;
const CRIT_PCT: f64 = 80.0;

#[derive(Debug, Clone, PartialEq)]
pub enum CpuLevel {
    Ok,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct CpuStatus {
    /// Current CPU usage as a percentage of the cgroup quota (0–100+).
    pub usage_pct: f64,
    /// Quota in cores (e.g. 2.0 means 2 cores).
    pub quota_cores: f64,
    /// Colour-coded level.
    pub level: CpuLevel,
    /// Whether cgroup data was available at all.
    pub available: bool,
}

impl CpuStatus {
    /// Returns a placeholder used when cgroup data is unavailable.
    pub fn unavailable() -> Self {
        Self {
            usage_pct: 0.0,
            quota_cores: 0.0,
            level: CpuLevel::Ok,
            available: false,
        }
    }
}

/// Read the cgroup v1 CPU quota for the current user.
/// Returns `None` if the files don't exist (non-HPC systems).
fn read_quota_cores() -> Option<f64> {
    let uid = get_uid();
    let quota_path = format!(
        "/sys/fs/cgroup/cpu/user.slice/user-{uid}.slice/cpu.cfs_quota_us"
    );
    let period_path = format!(
        "/sys/fs/cgroup/cpu/user.slice/user-{uid}.slice/cpu.cfs_period_us"
    );

    let quota: i64 = fs::read_to_string(&quota_path)
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let period: i64 = fs::read_to_string(&period_path)
        .ok()?
        .trim()
        .parse()
        .ok()?;

    // quota == -1 means unlimited
    if quota <= 0 || period <= 0 {
        return None;
    }

    Some(quota as f64 / period as f64)
}

/// Read current CPU usage from /proc/stat for the whole system,
/// then narrow it down to this user's cgroup usage_usec.
/// Falls back to cgroup cpuacct if available.
fn read_usage_cores() -> Option<f64> {
    let uid = get_uid();

    // Try cgroup v1 cpuacct first (most HPC systems)
    let cpuacct_path = format!(
        "/sys/fs/cgroup/cpuacct/user.slice/user-{uid}.slice/cpuacct.usage"
    );

    // We need two samples to compute a rate. Take two samples 200ms apart.
    let sample = |path: &str| -> Option<u64> {
        fs::read_to_string(path).ok()?.trim().parse().ok()
    };

    let t1 = sample(&cpuacct_path)?;
    std::thread::sleep(std::time::Duration::from_millis(200));
    let t2 = sample(&cpuacct_path)?;

    // cpuacct.usage is in nanoseconds of CPU time
    let delta_ns = t2.saturating_sub(t1) as f64;
    // Over 200ms wall time = 200_000_000 ns
    let wall_ns = 200_000_000.0_f64;
    // Cores used = CPU ns / wall ns
    Some(delta_ns / wall_ns)
}

/// Sample CPU status: quota + current usage + level.
/// This does a 200ms blocking sample — call from a background context or
/// accept the brief pause on refresh.
pub fn sample() -> CpuStatus {
    let quota_cores = match read_quota_cores() {
        Some(q) => q,
        None => return CpuStatus::unavailable(),
    };

    let usage_cores = match read_usage_cores() {
        Some(u) => u,
        None => return CpuStatus::unavailable(),
    };

    let usage_pct = (usage_cores / quota_cores) * 100.0;

    let level = if usage_pct >= CRIT_PCT {
        CpuLevel::Critical
    } else if usage_pct >= WARN_PCT {
        CpuLevel::Warning
    } else {
        CpuLevel::Ok
    };

    CpuStatus {
        usage_pct,
        quota_cores,
        level,
        available: true,
    }
}

/// Would adding one more idle shell process push us into Critical?
/// An idle shell uses ~0% CPU itself, but we use a conservative headroom
/// check: if we're already at or above Warning, block new sessions.
pub fn would_exceed_on_new_session(status: &CpuStatus) -> bool {
    status.available && status.level == CpuLevel::Critical
}

fn get_uid() -> u32 {
    // SAFETY: getuid() is always safe to call
    unsafe { libc::getuid() }
}
