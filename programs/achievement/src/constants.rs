use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const ACHIEVEMENT_SEED: &[u8] = b"achievement";

/// Fallback base URI, used only when `init_collection` is called with an empty
/// string. The live value lives in `AchievementConfig::base_uri` so it can be
/// corrected without redeploying the program -- see `set_base_uri`.
pub const DEFAULT_METADATA_BASE_URI: &str = "https://paygig.app/achievements";

pub fn badge_name(badge_type: reputation::BadgeType) -> &'static str {
    use reputation::BadgeType::*;
    match badge_type {
        FirstGig => "First Gig",
        TenCompletedJobs => "Ten Gigs",
        HundredCompletedJobs => "Hundred Gigs",
        FiveStarPerformer => "Five Star",
        TrustedFreelancer => "Trusted Freelancer",
        FastDeliverer => "Fast Deliverer",
        TopRated => "Top Rated",
    }
}

pub fn badge_slug(badge_type: reputation::BadgeType) -> &'static str {
    use reputation::BadgeType::*;
    match badge_type {
        FirstGig => "first-gig",
        TenCompletedJobs => "ten-gigs",
        HundredCompletedJobs => "hundred-gigs",
        FiveStarPerformer => "five-star",
        TrustedFreelancer => "trusted-freelancer",
        FastDeliverer => "fast-deliverer",
        TopRated => "top-rated",
    }
}

/// Builds a badge's metadata URI from the config's stored base and the badge
/// slug. Any trailing slash on `base_uri` is trimmed so both `https://host/x`
/// and `https://host/x/` produce the same result.
pub fn badge_uri(base_uri: &str, badge_type: reputation::BadgeType) -> String {
    format!(
        "{}/{}.json",
        base_uri.trim_end_matches('/'),
        badge_slug(badge_type)
    )
}
