use anchor_lang::prelude::*;

#[error_code]
pub enum GigError {
    #[msg("Signer is not authorized to perform this action")]
    Unauthorized,
    #[msg("Title exceeds maximum length")]
    TitleTooLong,
    #[msg("Description exceeds maximum length")]
    DescriptionTooLong,
    #[msg("Skills string exceeds maximum length")]
    SkillsTooLong,
    #[msg("Category exceeds maximum length")]
    CategoryTooLong,
    #[msg("Deadline is too soon or in the past")]
    InvalidDeadline,
    #[msg("Budget must be greater than zero")]
    InvalidBudget,
    #[msg("Freelancer is already assigned to this gig")]
    FreelancerAlreadyAssigned,
    #[msg("Gig must be in Draft status")]
    NotDraftStatus,
    #[msg("Gig must be in Published status")]
    NotPublishedStatus,
    #[msg("Gig must be in Assigned status")]
    NotAssignedStatus,
    #[msg("Gig must be in InProgress status")]
    NotInProgressStatus,
    #[msg("Gig must be in Completed status")]
    NotCompletedStatus,
    #[msg("Gig is in a terminal or otherwise invalid status for this transition")]
    InvalidStatus,
    #[msg("Arithmetic overflow")]
    Overflow,
}
