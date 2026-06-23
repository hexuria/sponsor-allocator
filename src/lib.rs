pub mod error;
pub mod events;
pub mod pool;
pub mod service;
pub mod strategy;

// Re-export key types for convenience
pub use error::SponsorAllocationError;
pub use events::SponsorEvent;
pub use pool::{PoolStats, SponsorCandidate, SponsorPool};
pub use service::SponsorService;
pub use strategy::{AllocationStrategy, PerformanceBasedStrategy, RoundRobinStrategy};
