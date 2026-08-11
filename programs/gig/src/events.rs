use anchor_lang::prelude::*;

#[event]
pub struct GigCreated {
    pub gig: Pubkey,
    pub id: u64,
    pub client: Pubkey,
    pub freelancer: Pubkey,
    pub created_at: i64,
}

#[event]
pub struct GigUpdated {
    pub gig: Pubkey,
    pub id: u64,
    pub title: String,
    pub description: String,
    pub skills: String,
    pub category: String,
    pub budget: u64,
    pub deadline: i64,
}

#[event]
pub struct GigPublished {
    pub gig: Pubkey,
    pub id: u64,
}

#[event]
pub struct FreelancerAssigned {
    pub gig: Pubkey,
    pub id: u64,
    pub freelancer: Pubkey,
}

#[event]
pub struct GigInProgress {
    pub gig: Pubkey,
    pub id: u64,
}

#[event]
pub struct GigCompleted {
    pub gig: Pubkey,
    pub id: u64,
}

#[event]
pub struct GigArchived {
    pub gig: Pubkey,
    pub id: u64,
}

#[event]
pub struct GigCancelled {
    pub gig: Pubkey,
    pub id: u64,
}
