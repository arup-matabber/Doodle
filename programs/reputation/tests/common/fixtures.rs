use super::*;

/// Standard review hashes used across tests.
pub const ALT_REVIEW_HASH_1: [u8; 32] = [42u8; 32];
pub const ALT_REVIEW_HASH_2: [u8; 32] = [99u8; 32];
pub const ALT_REVIEW_HASH_3: [u8; 32] = [1u8; 32];
pub const ZERO_REVIEW_HASH: [u8; 32] = [0u8; 32];

/// Compute the expected reputation score for a given profile configuration.
/// Mirrors the on-chain algorithm for verification.
pub fn expected_reputation_score(profile: &UserProfile) -> u64 {
    let success_rate = if profile.completed_jobs == 0 {
        0
    } else {
        (profile.successful_jobs as u128)
            .checked_mul(100)
            .unwrap()
            .checked_div(profile.completed_jobs as u128)
            .unwrap() as u64
    };

    let rating_score = (profile.average_rating as u64).checked_div(5).unwrap_or(0).min(100);
    let volume_score = profile.completed_jobs.min(100);
    let earnings_score = (profile.total_earnings.min(1_000_000_000) as u128)
        .checked_mul(100)
        .unwrap()
        .checked_div(1_000_000_000)
        .unwrap() as u64;

    let cancellation_penalty = (profile.cancelled_jobs as u128)
        .checked_mul(5)
        .unwrap()
        .min(200) as u64;

    let weighted = (success_rate as u128)
        .checked_mul(3)
        .unwrap()
        .checked_add(
            (rating_score as u128).checked_mul(3).unwrap()
        )
        .unwrap()
        .checked_add(
            (volume_score as u128).checked_mul(2).unwrap()
        )
        .unwrap()
        .checked_add(
            (earnings_score as u128).checked_mul(2).unwrap()
        )
        .unwrap();

    let penalty = (cancellation_penalty as u128).checked_mul(2).unwrap();
    let score = (weighted).saturating_sub(penalty);
    score.min(1000) as u64
}

/// Expected average rating given sum and count.
pub fn expected_average_rating(sum: u64, count: u64) -> u32 {
    if count == 0 {
        return 0;
    }
    ((sum as u128).checked_mul(100).unwrap().checked_div(count as u128).unwrap()) as u32
}

/// Profile configurations for known expected scores.
pub mod known_scores {
    use super::*;

    pub fn empty_profile() -> UserProfile {
        UserProfile {
            authority: Pubkey::default(),
            completed_jobs: 0,
            successful_jobs: 0,
            cancelled_jobs: 0,
            total_earnings: 0,
            rating_sum: 0,
            rating_count: 0,
            average_rating: 0,
            reputation_score: 0,
            badges_earned: 0,
            created_at: 0,
            updated_at: 0,
            bump: 0,
        }
    }

    pub fn ideal_profile() -> UserProfile {
        UserProfile {
            authority: Pubkey::default(),
            completed_jobs: 200,
            successful_jobs: 200,
            cancelled_jobs: 0,
            total_earnings: 1_000_000_000,
            rating_sum: 100,
            rating_count: 20,
            average_rating: 500,
            reputation_score: 0,
            badges_earned: 0,
            created_at: 0,
            updated_at: 0,
            bump: 0,
        }
    }

    pub fn half_profile() -> UserProfile {
        UserProfile {
            authority: Pubkey::default(),
            completed_jobs: 50,
            successful_jobs: 25,
            cancelled_jobs: 25,
            total_earnings: 500_000_000,
            rating_sum: 60,
            rating_count: 15,
            average_rating: 400,
            reputation_score: 0,
            badges_earned: 0,
            created_at: 0,
            updated_at: 0,
            bump: 0,
        }
    }

    pub fn min_viable_profile() -> UserProfile {
        UserProfile {
            authority: Pubkey::default(),
            completed_jobs: 1,
            successful_jobs: 1,
            cancelled_jobs: 0,
            total_earnings: 1,
            rating_sum: 3,
            rating_count: 1,
            average_rating: 300,
            reputation_score: 0,
            badges_earned: 0,
            created_at: 0,
            updated_at: 0,
            bump: 0,
        }
    }
}

/// Expected scores for known configurations.
/// Computed with the same algorithm as on-chain.
pub fn expected_known_scores() -> Vec<(UserProfile, u64, &'static str)> {
    vec![
        (known_scores::empty_profile(), 0, "empty"),
        (known_scores::ideal_profile(), 1000, "ideal"),
        (known_scores::half_profile(), 370, "half"),
        (known_scores::min_viable_profile(), 108, "min_viable"),
    ]
}

/// All 7 badge types for exhaustive testing.
pub const ALL_BADGE_TYPES: [BadgeType; 7] = [
    BadgeType::FirstGig,
    BadgeType::TenCompletedJobs,
    BadgeType::HundredCompletedJobs,
    BadgeType::FiveStarPerformer,
    BadgeType::TopRated,
    BadgeType::TrustedFreelancer,
    BadgeType::FastDeliverer,
];

/// Badge eligibility requirements (min completed, min successful, min ratings, min avg rating).
pub struct BadgeRequirement {
    pub min_completed: u64,
    pub min_successful: u64,
    pub min_ratings: u64,
    pub min_avg_rating: u32,
    pub authority_attested: bool,
}

pub fn badge_requirements(bt: BadgeType) -> BadgeRequirement {
    match bt {
        BadgeType::FirstGig => BadgeRequirement {
            min_completed: 1, min_successful: 0, min_ratings: 0, min_avg_rating: 0, authority_attested: false,
        },
        BadgeType::TenCompletedJobs => BadgeRequirement {
            min_completed: 10, min_successful: 0, min_ratings: 0, min_avg_rating: 0, authority_attested: false,
        },
        BadgeType::HundredCompletedJobs => BadgeRequirement {
            min_completed: 100, min_successful: 0, min_ratings: 0, min_avg_rating: 0, authority_attested: false,
        },
        BadgeType::FiveStarPerformer => BadgeRequirement {
            min_completed: 0, min_successful: 0, min_ratings: 5, min_avg_rating: 500, authority_attested: false,
        },
        BadgeType::TopRated => BadgeRequirement {
            min_completed: 0, min_successful: 0, min_ratings: 10, min_avg_rating: 450, authority_attested: false,
        },
        BadgeType::TrustedFreelancer => BadgeRequirement {
            min_completed: 0, min_successful: 0, min_ratings: 0, min_avg_rating: 0, authority_attested: true,
        },
        BadgeType::FastDeliverer => BadgeRequirement {
            min_completed: 0, min_successful: 0, min_ratings: 0, min_avg_rating: 0, authority_attested: true,
        },
    }
}
