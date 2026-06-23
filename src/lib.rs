pub mod error;
pub mod events;
pub mod pool;
pub mod service;
pub mod strategy;

#[cfg(feature = "db")]
pub mod repository;

// Re-export key types for convenience
pub use error::SponsorAllocationError;
pub use events::SponsorEvent;
pub use pool::{PoolStats, SponsorCandidate, SponsorPool};
pub use service::SponsorService;
pub use strategy::{AllocationStrategy, PerformanceBasedStrategy, RoundRobinStrategy};

#[cfg(feature = "db")]
pub use repository::{PgSponsorRepository, SponsorRepository};
