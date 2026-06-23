use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SponsorAllocationError {
    #[error("No sponsors available in the pool")]
    NoSponsorsAvailable,

    #[error("Sponsor {sponsor_id} not found in pool")]
    SponsorNotFound { sponsor_id: Uuid },

    #[error("Account {account_id} is not eligible to be a sponsor")]
    AccountNotEligible { account_id: Uuid },

    #[error("Account {account_id} already has maximum sponsored accounts")]
    MaxSponsoredAccountsReached { account_id: Uuid },

    #[error("Invalid allocation strategy configuration")]
    InvalidStrategyConfiguration,
}
