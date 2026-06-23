use crate::error::SponsorAllocationError;
use crate::pool::SponsorPool;
use uuid::Uuid;

/// Trait for different sponsor allocation strategies.
pub trait AllocationStrategy {
    /// Allocate a sponsor for a new account.
    fn allocate_sponsor(
        &mut self,
        sponsor_pool: &SponsorPool,
        new_account_id: Uuid,
    ) -> Result<Uuid, SponsorAllocationError>;

    /// Get the strategy's name for logging and events.
    fn strategy_name(&self) -> &'static str;
}

/// Round-robin allocation strategy.
/// Distributes sponsorships evenly across available sponsors.
#[derive(Debug, Default, Clone)]
pub struct RoundRobinStrategy {
    last_allocated_index: usize,
}

impl RoundRobinStrategy {
    pub fn new() -> Self {
        Self {
            last_allocated_index: 0,
        }
    }
}

impl AllocationStrategy for RoundRobinStrategy {
    fn allocate_sponsor(
        &mut self,
        sponsor_pool: &SponsorPool,
        _new_account_id: Uuid,
    ) -> Result<Uuid, SponsorAllocationError> {
        let eligible_sponsors = sponsor_pool.get_eligible_sponsors();

        if eligible_sponsors.is_empty() {
            return Err(SponsorAllocationError::NoSponsorsAvailable);
        }

        // Get next sponsor in round-robin fashion
        let sponsor_index = self.last_allocated_index % eligible_sponsors.len();
        let selected_sponsor = &eligible_sponsors[sponsor_index];

        // Update index for next allocation
        self.last_allocated_index = (sponsor_index + 1) % eligible_sponsors.len();

        Ok(selected_sponsor.account_id)
    }

    fn strategy_name(&self) -> &'static str {
        "round_robin"
    }
}

/// Performance-based allocation strategy.
/// Prioritizes sponsors with higher performance metrics.
#[derive(Debug, Default, Clone)]
pub struct PerformanceBasedStrategy;

impl PerformanceBasedStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl AllocationStrategy for PerformanceBasedStrategy {
    fn allocate_sponsor(
        &mut self,
        sponsor_pool: &SponsorPool,
        _new_account_id: Uuid,
    ) -> Result<Uuid, SponsorAllocationError> {
        let mut eligible_sponsors = sponsor_pool.get_eligible_sponsors();

        if eligible_sponsors.is_empty() {
            return Err(SponsorAllocationError::NoSponsorsAvailable);
        }

        // Sort by performance: tier (Ace > King > Queen) then cycle count
        eligible_sponsors.sort_by(|a, b| {
            let tier_order_a = tier_to_order(&a.tier);
            let tier_order_b = tier_to_order(&b.tier);

            tier_order_b
                .cmp(&tier_order_a) // Higher tier first
                .then_with(|| b.cycle_count.cmp(&a.cycle_count)) // Then higher cycle count
        });

        // Select the top performer
        Ok(eligible_sponsors[0].account_id)
    }

    fn strategy_name(&self) -> &'static str {
        "performance_based"
    }
}

/// Convert tier name to numerical order for comparison.
fn tier_to_order(tier: &str) -> u8 {
    match tier {
        "Ace" => 4,
        "King" => 3,
        "Queen" => 2,
        "Jack" => 1,
        "Ten" => 0,
        _ => 0,
    }
}
