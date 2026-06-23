use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SponsorEvent {
    /// A new sponsor has been added to the pool
    SponsorAdded {
        sponsor_id: Uuid,
        tier: String,
        cycle_count: u32,
    },

    /// A sponsor has been removed from the pool
    SponsorRemoved { sponsor_id: Uuid, reason: String },

    /// A sponsor has been allocated to a new account
    SponsorAllocated {
        sponsor_id: Uuid,
        sponsored_account_id: Uuid,
        allocation_strategy: String,
    },

    /// Sponsor pool has been refreshed based on current top performers
    SponsorPoolRefreshed {
        new_sponsor_count: usize,
        removed_sponsor_count: usize,
    },
}

/// Ingestion event emitted when an account graduates from flushline
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlushlineGraduated {
    pub account_id: Uuid,
}

/// Ingestion event emitted when a matrix cycles
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MatrixCycled {
    pub account_id: Uuid,
    pub matrix_id: Uuid,
}
