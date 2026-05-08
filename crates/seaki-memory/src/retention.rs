//! RetentionScheduler: Ebbinghaus forgetting-curve based review scheduling.

/// 艾宾浩斯遗忘曲线调度器。
pub struct RetentionScheduler;

impl RetentionScheduler {
    /// 计算 retention(t) = exp(-elapsed_days / stability_days)
    #[must_use]
    pub fn retention(elapsed_days: f64, stability_days: f64) -> f64 {
        if !stability_days.is_finite() || stability_days <= 0.0 {
            return 0.0;
        }
        let ratio = -elapsed_days / stability_days;
        if !ratio.is_finite() {
            return 0.0;
        }
        ratio.exp().clamp(0.0, 1.0)
    }

    /// 判断卡片是否到期（retention <= threshold）。
    #[must_use]
    pub fn is_due(last_reviewed_at: u64, stability_days: f64, threshold: f64, now: u64) -> bool {
        if threshold <= 0.0 {
            return false;
        }
        if threshold >= 1.0 {
            return true;
        }
        let elapsed = (now.saturating_sub(last_reviewed_at)) as f64 / 86400.0;
        Self::retention(elapsed, stability_days) <= threshold
    }

    /// 计算下一次复习时间。
    #[must_use]
    pub fn next_review_at(last_reviewed_at: u64, stability_days: f64) -> u64 {
        let days = if stability_days.is_finite() {
            stability_days.max(0.0)
        } else {
            0.0
        };
        let seconds = (days * 86400.0) as u64;
        last_reviewed_at.saturating_add(seconds)
    }

    /// 预估 retention 下降到 threshold 所需天数。
    #[must_use]
    pub fn days_to_threshold(stability_days: f64, threshold: f64) -> f64 {
        if !stability_days.is_finite() || stability_days <= 0.0 {
            return 0.0;
        }
        if threshold <= 0.0 {
            return f64::MAX;
        }
        if threshold >= 1.0 {
            return 0.0;
        }
        -stability_days * threshold.ln()
    }
}
