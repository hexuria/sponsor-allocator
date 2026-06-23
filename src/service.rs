use crate::error::SponsorAllocationError;
use crate::events::{FlushlineGraduated, MatrixCycled, SponsorEvent};
use crate::pool::{PoolStats, SponsorCandidate, SponsorPool};
use crate::strategy::AllocationStrategy;
use std::collections::HashMap;
use uuid::Uuid;

/// Service aggregate for managing sponsor allocation, pool maintenance,
/// and processing events synchronously.
pub struct SponsorService {
    sponsor_pool: SponsorPool,
    allocation_strategy: Box<dyn AllocationStrategy + Send + Sync>,

    // Tracks account tier and cycle information
    account_tiers: HashMap<Uuid, String>,
    account_cycles: HashMap<Uuid, u32>,
}

impl SponsorService {
    pub fn new(
        allocation_strategy: Box<dyn AllocationStrategy + Send + Sync>,
        max_pool_size: usize,
    ) -> Self {
        Self {
            sponsor_pool: SponsorPool::new(max_pool_size),
            allocation_strategy,
            account_tiers: HashMap::new(),
            account_cycles: HashMap::new(),
        }
    }

    /// Allocate a sponsor for a new account.
    ///
    /// Returns a tuple containing the allocated sponsor's UUID and the outbox events emitted.
    pub fn allocate_sponsor(
        &mut self,
        new_account_id: Uuid,
    ) -> Result<(Uuid, Vec<SponsorEvent>), SponsorAllocationError> {
        let sponsor_id = self
            .allocation_strategy
            .allocate_sponsor(&self.sponsor_pool, new_account_id)?;

        // Record the sponsorship in the pool
        self.sponsor_pool.record_sponsorship(sponsor_id)?;

        let event = SponsorEvent::SponsorAllocated {
            sponsor_id,
            sponsored_account_id: new_account_id,
            allocation_strategy: self.allocation_strategy.strategy_name().to_string(),
        };

        Ok((sponsor_id, vec![event]))
    }

    /// Process a flushline graduation event to update sponsor eligibility.
    ///
    /// Returns any resulting events (like `SponsorAdded`).
    pub fn handle_flushline_graduated(&mut self, event: &FlushlineGraduated) -> Vec<SponsorEvent> {
        let account_id = event.account_id;
        // Graduated accounts are placed in the "Ace" tier by default.
        self.account_tiers.insert(account_id, "Ace".to_string());

        // Update pool eligibility
        self.update_sponsor_eligibility(account_id)
    }

    /// Process a matrix cycled event to update sponsor eligibility.
    ///
    /// Returns any resulting events (like `SponsorAdded`).
    pub fn handle_matrix_cycled(&mut self, event: &MatrixCycled) -> Vec<SponsorEvent> {
        let account_id = event.account_id;
        // Increment cycle count for this account
        let cycle_count = self.account_cycles.get(&account_id).unwrap_or(&0) + 1;
        self.account_cycles.insert(account_id, cycle_count);

        // Update pool eligibility
        self.update_sponsor_eligibility(account_id)
    }

    /// Update sponsor eligibility for an account based on its current tier and cycles.
    fn update_sponsor_eligibility(&mut self, account_id: Uuid) -> Vec<SponsorEvent> {
        let tier = self
            .account_tiers
            .get(&account_id)
            .cloned()
            .unwrap_or_else(|| "Ten".to_string());
        let cycle_count = self.account_cycles.get(&account_id).cloned().unwrap_or(0);

        let candidate = SponsorCandidate::new(account_id, tier.clone(), cycle_count);

        if candidate.is_top_performer() {
            match self.sponsor_pool.add_sponsor(candidate) {
                Ok(()) => {
                    vec![SponsorEvent::SponsorAdded {
                        sponsor_id: account_id,
                        tier,
                        cycle_count,
                    }]
                }
                Err(_) => Vec::new(),
            }
        } else {
            // If it is already in the pool but no longer qualifies, remove it
            if self.sponsor_pool.remove_sponsor(account_id).is_ok() {
                vec![SponsorEvent::SponsorRemoved {
                    sponsor_id: account_id,
                    reason: "No longer eligible".to_string(),
                }]
            } else {
                Vec::new()
            }
        }
    }

    /// Manually add a sponsor candidate to the pool.
    pub fn add_sponsor(
        &mut self,
        candidate: SponsorCandidate,
    ) -> Result<Vec<SponsorEvent>, SponsorAllocationError> {
        self.account_tiers
            .insert(candidate.account_id, candidate.tier.clone());
        self.account_cycles
            .insert(candidate.account_id, candidate.cycle_count);
        self.sponsor_pool.add_sponsor(candidate.clone())?;

        let event = SponsorEvent::SponsorAdded {
            sponsor_id: candidate.account_id,
            tier: candidate.tier,
            cycle_count: candidate.cycle_count,
        };

        Ok(vec![event])
    }

    /// Manually remove a sponsor from the pool.
    pub fn remove_sponsor(
        &mut self,
        account_id: Uuid,
        reason: String,
    ) -> Result<Vec<SponsorEvent>, SponsorAllocationError> {
        self.sponsor_pool.remove_sponsor(account_id)?;

        let event = SponsorEvent::SponsorRemoved {
            sponsor_id: account_id,
            reason,
        };

        Ok(vec![event])
    }

    /// Refresh the sponsor pool based on current top performers.
    pub fn refresh_sponsor_pool(
        &mut self,
        top_performers: Vec<SponsorCandidate>,
    ) -> Vec<SponsorEvent> {
        let old_count = self.sponsor_pool.pool_stats().total_sponsors;

        self.sponsor_pool.refresh_pool(top_performers);

        let new_count = self.sponsor_pool.pool_stats().total_sponsors;

        vec![SponsorEvent::SponsorPoolRefreshed {
            new_sponsor_count: new_count,
            removed_sponsor_count: old_count.saturating_sub(new_count),
        }]
    }

    /// Get current sponsor pool statistics.
    pub fn get_pool_stats(&self) -> PoolStats {
        self.sponsor_pool.pool_stats()
    }

    /// Get eligible sponsors.
    pub fn get_eligible_sponsors(&self) -> Vec<SponsorCandidate> {
        self.sponsor_pool.get_eligible_sponsors()
    }

    /// Update account tier information manually.
    pub fn update_account_tier(&mut self, account_id: Uuid, tier: String) -> Vec<SponsorEvent> {
        self.account_tiers.insert(account_id, tier);
        self.update_sponsor_eligibility(account_id)
    }

    /// Get top N sponsors by performance.
    pub fn get_top_sponsors(&self, count: usize) -> Vec<SponsorCandidate> {
        self.sponsor_pool.get_top_sponsors(count)
    }

    /// Manually record a sponsorship for an account (useful for testing and bootstrapping).
    pub fn record_sponsorship(&mut self, sponsor_id: Uuid) -> Result<(), SponsorAllocationError> {
        self.sponsor_pool.record_sponsorship(sponsor_id)
    }
}
