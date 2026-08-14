use anchor_lang::prelude::*;

use crate::constants::{
    CANCELLATION_PENALTY_PER_JOB, EARNINGS_SCORE_CAP, MAX_REPUTATION_SCORE, RATING_SCALE,
    VOLUME_SCORE_CAP,
};
use crate::errors::ReputationError;
use crate::state::{BadgeType, UserProfile};

pub fn checked_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(|| error!(ReputationError::MathOverflow))
}

pub fn checked_sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or_else(|| error!(ReputationError::MathOverflow))
}

pub fn checked_mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(|| error!(ReputationError::MathOverflow))
}

pub fn checked_div(a: u64, b: u64) -> Result<u64> {
    a.checked_div(b).ok_or_else(|| error!(ReputationError::MathOverflow))
}

/// Recomputes the scaled average rating from the running sum/count.
/// Recomputing from totals (rather than blending in the new sample
/// incrementally) keeps the result exact regardless of submission order.
pub fn average_rating(rating_sum: u64, rating_count: u64) -> Result<u32> {
    if rating_count == 0 {
        return Ok(0);
    }
    let scaled = checked_mul(rating_sum, RATING_SCALE)?;
    let avg = checked_div(scaled, rating_count)?;
    u32::try_from(avg).map_err(|_| error!(ReputationError::MathOverflow))
}

/// Deterministic, reproducible reputation score in `[0, MAX_REPUTATION_SCORE]`.
///
/// Weighted sum of four capped components (success rate, average rating,
/// completed-job volume, lifetime earnings), minus a penalty for cancelled
/// jobs. No randomness, no off-chain inputs: identical profile stats always
/// yield the identical score, and can be recomputed by anyone from the
/// stored fields alone.
pub fn compute_reputation_score(profile: &UserProfile) -> Result<u64> {
    let success_rate = if profile.completed_jobs == 0 {
        0
    } else {
        checked_div(checked_mul(profile.successful_jobs, 100)?, profile.completed_jobs)?
    };

    // average_rating is scaled by RATING_SCALE (100) and caps at 500 (5.00 stars);
    // dividing by 5 maps that range onto a 0-100 score.
    let rating_score = checked_div(profile.average_rating as u64, 5)?.min(100);

    let volume_score = profile.completed_jobs.min(VOLUME_SCORE_CAP);

    let earnings_score = checked_div(
        checked_mul(profile.total_earnings.min(EARNINGS_SCORE_CAP), 100)?,
        EARNINGS_SCORE_CAP,
    )?;

    let cancellation_penalty =
        checked_mul(profile.cancelled_jobs, CANCELLATION_PENALTY_PER_JOB)?.min(200);

    let weighted = checked_add(
        checked_add(
            checked_mul(success_rate, 3)?,
            checked_mul(rating_score, 3)?,
        )?,
        checked_add(
            checked_mul(volume_score, 2)?,
            checked_mul(earnings_score, 2)?,
        )?,
    )?;

    let penalty = checked_mul(cancellation_penalty, 2)?;
    let score = weighted.saturating_sub(penalty);

    Ok(score.min(MAX_REPUTATION_SCORE))
}

/// Deterministic badge eligibility, checked against the profile's own public
/// fields -- `award_badge` is permissionless, so eligibility must never
/// depend on anything the caller could assert unverified.
///
/// `FastDeliverer` and `TrustedFreelancer` depend on signals this program
/// does not track on-chain (delivery timing, external endorsements) and are
/// not awardable yet; they return `false` until that data source exists.
pub fn is_eligible_for_badge(profile: &UserProfile, badge_type: BadgeType) -> bool {
    match badge_type {
        BadgeType::FirstGig => profile.completed_jobs >= 1,
        BadgeType::TenCompletedJobs => profile.completed_jobs >= 10,
        BadgeType::HundredCompletedJobs => profile.completed_jobs >= 100,
        BadgeType::FiveStarPerformer => {
            profile.rating_count >= 5 && profile.average_rating >= 500
        }
        BadgeType::TopRated => profile.rating_count >= 10 && profile.average_rating >= 450,
        BadgeType::TrustedFreelancer | BadgeType::FastDeliverer => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(completed: u64, successful: u64, cancelled: u64, earnings: u64, avg: u32, count: u64) -> UserProfile {
        UserProfile {
            authority: Pubkey::default(),
            completed_jobs: completed,
            successful_jobs: successful,
            cancelled_jobs: cancelled,
            total_earnings: earnings,
            rating_sum: 0,
            rating_count: count,
            average_rating: avg,
            reputation_score: 0,
            badges_earned: 0,
            created_at: 0,
            updated_at: 0,
            bump: 0,
        }
    }

    #[test]
    fn checked_add_overflows() {
        assert!(checked_add(u64::MAX, 1).is_err());
    }

    #[test]
    fn checked_sub_underflows() {
        assert!(checked_sub(0, 1).is_err());
    }

    #[test]
    fn average_rating_computes_exact_mean() {
        assert_eq!(average_rating(15, 3).unwrap(), 500); // 5.00
        assert_eq!(average_rating(0, 0).unwrap(), 0);
        assert_eq!(average_rating(7, 2).unwrap(), 350); // 3.50
    }

    #[test]
    fn reputation_score_is_zero_for_empty_profile() {
        let p = profile(0, 0, 0, 0, 0, 0);
        assert_eq!(compute_reputation_score(&p).unwrap(), 0);
    }

    #[test]
    fn reputation_score_is_maxed_for_ideal_profile() {
        let p = profile(200, 200, 0, EARNINGS_SCORE_CAP, 500, 20);
        let score = compute_reputation_score(&p).unwrap();
        assert_eq!(score, MAX_REPUTATION_SCORE);
    }

    #[test]
    fn reputation_score_is_penalized_by_cancellations() {
        let clean = profile(50, 50, 0, 0, 500, 10);
        let cancelled = profile(50, 50, 10, 0, 500, 10);
        assert!(
            compute_reputation_score(&cancelled).unwrap()
                < compute_reputation_score(&clean).unwrap()
        );
    }

    #[test]
    fn reputation_score_never_underflows_below_zero() {
        let p = profile(1, 1, 1000, 0, 0, 0);
        assert_eq!(compute_reputation_score(&p).unwrap(), 0);
    }

    #[test]
    fn badge_eligibility_first_gig() {
        let none = profile(0, 0, 0, 0, 0, 0);
        let one = profile(1, 1, 0, 0, 0, 0);
        assert!(!is_eligible_for_badge(&none, BadgeType::FirstGig));
        assert!(is_eligible_for_badge(&one, BadgeType::FirstGig));
    }

    #[test]
    fn badge_eligibility_five_star_requires_min_samples() {
        let few_samples = profile(10, 10, 0, 0, 500, 2);
        let enough_samples = profile(10, 10, 0, 0, 500, 5);
        assert!(!is_eligible_for_badge(&few_samples, BadgeType::FiveStarPerformer));
        assert!(is_eligible_for_badge(&enough_samples, BadgeType::FiveStarPerformer));
    }
}
