use crate::error::SponsorAllocationError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Represents a potential sponsor account in the Royal Flush Network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SponsorCandidate {
    pub account_id: Uuid,
    pub tier: String, // "Ace", "King", "Queen", "Jack", "Ten"
    pub cycle_count: u32,
    pub sponsored_count: u32, // Number of accounts currently sponsored by this sponsor
}

impl SponsorCandidate {
    pub fn new(account_id: Uuid, tier: String, cycle_count: u32) -> Self {
        Self {
            account_id,
            tier,
            cycle_count,
            sponsored_count: 0,
        }
    }

    /// Check if this candidate is eligible to sponsor more accounts.
    pub fn is_eligible_for_sponsorship(&self) -> bool {
        // Business rule: sponsors can handle up to 10 sponsored accounts,
        // and must be top performers (King tier).
        self.sponsored_count < 10 && self.is_top_performer()
    }

    /// Check if this candidate qualifies as a "top performer".
    pub fn is_top_performer(&self) -> bool {
        // Business rule: ONLY King tier accounts can be sponsors.
        self.tier == "King"
    }

    /// Calculate sponsor score for ranking (higher is better).
    pub fn sponsor_score(&self) -> u32 {
        let tier_score = match self.tier.as_str() {
            "King" => 1000,
            _ => 0, // Other tiers cannot be sponsors
        };

        // Combine tier score with cycle count, but reduce by sponsored count
        tier_score + self.cycle_count - (self.sponsored_count * 2)
    }
}

/// Manages the pool of available sponsors.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct SponsorPool {
    sponsors: HashMap<Uuid, SponsorCandidate>,
    max_pool_size: usize,
}

impl SponsorPool {
    pub fn new(max_pool_size: usize) -> Self {
        Self {
            sponsors: HashMap::new(),
            max_pool_size,
        }
    }

    /// Create sponsor pool from a list of candidates.
    pub fn from_candidates(candidates: Vec<SponsorCandidate>, max_pool_size: usize) -> Self {
        let mut pool = Self::new(max_pool_size);
        for candidate in candidates {
            if candidate.is_top_performer() {
                let _ = pool.add_sponsor(candidate);
            }
        }
        pool
    }

    /// Add a potential sponsor to the pool.
    pub fn add_sponsor(
        &mut self,
        candidate: SponsorCandidate,
    ) -> Result<(), SponsorAllocationError> {
        if !candidate.is_top_performer() {
            return Err(SponsorAllocationError::AccountNotEligible {
                account_id: candidate.account_id,
            });
        }

        // If pool is full, remove lowest performing sponsor first
        if self.sponsors.len() >= self.max_pool_size {
            self.remove_lowest_performing_sponsor();
        }

        self.sponsors.insert(candidate.account_id, candidate);
        Ok(())
    }

    /// Remove a sponsor from the pool.
    pub fn remove_sponsor(
        &mut self,
        account_id: Uuid,
    ) -> Result<SponsorCandidate, SponsorAllocationError> {
        self.sponsors
            .remove(&account_id)
            .ok_or(SponsorAllocationError::SponsorNotFound {
                sponsor_id: account_id,
            })
    }

    /// Update sponsor information using an update function.
    pub fn update_sponsor(
        &mut self,
        account_id: Uuid,
        update_fn: impl FnOnce(&mut SponsorCandidate),
    ) -> Result<(), SponsorAllocationError> {
        let sponsor =
            self.sponsors
                .get_mut(&account_id)
                .ok_or(SponsorAllocationError::SponsorNotFound {
                    sponsor_id: account_id,
                })?;

        update_fn(sponsor);

        // Remove sponsor if they're no longer eligible
        if !sponsor.is_eligible_for_sponsorship() {
            self.sponsors.remove(&account_id);
        }

        Ok(())
    }

    /// Get all eligible sponsors for allocation.
    pub fn get_eligible_sponsors(&self) -> Vec<SponsorCandidate> {
        self.sponsors
            .values()
            .filter(|sponsor| sponsor.is_eligible_for_sponsorship())
            .cloned()
            .collect()
    }

    /// Get top N sponsors by performance.
    pub fn get_top_sponsors(&self, count: usize) -> Vec<SponsorCandidate> {
        let mut sponsors: Vec<_> = self.get_eligible_sponsors();
        sponsors.sort_by_key(|b| std::cmp::Reverse(b.sponsor_score()));
        sponsors.into_iter().take(count).collect()
    }

    /// Record that a sponsor has been allocated to an account.
    pub fn record_sponsorship(&mut self, sponsor_id: Uuid) -> Result<(), SponsorAllocationError> {
        self.update_sponsor(sponsor_id, |sponsor| {
            sponsor.sponsored_count += 1;
        })
    }

    /// Record that a sponsored account is no longer active.
    pub fn record_sponsorship_ended(
        &mut self,
        sponsor_id: Uuid,
    ) -> Result<(), SponsorAllocationError> {
        self.update_sponsor(sponsor_id, |sponsor| {
            if sponsor.sponsored_count > 0 {
                sponsor.sponsored_count -= 1;
            }
        })
    }

    /// Refresh the pool by updating sponsor performance data.
    pub fn refresh_pool(&mut self, updated_candidates: Vec<SponsorCandidate>) {
        self.sponsors.clear();

        for candidate in updated_candidates {
            if candidate.is_top_performer() {
                self.sponsors.insert(candidate.account_id, candidate);
            }
        }

        while self.sponsors.len() > self.max_pool_size {
            self.remove_lowest_performing_sponsor();
        }
    }

    /// Get pool statistics.
    pub fn pool_stats(&self) -> PoolStats {
        let total_sponsors = self.sponsors.len();
        let eligible_sponsors = self.get_eligible_sponsors().len();
        let ace_sponsors = self.sponsors.values().filter(|s| s.tier == "Ace").count();
        let king_sponsors = self.sponsors.values().filter(|s| s.tier == "King").count();

        PoolStats {
            total_sponsors,
            eligible_sponsors,
            ace_sponsors,
            king_sponsors,
            max_pool_size: self.max_pool_size,
        }
    }

    /// Remove the lowest performing sponsor from the pool.
    fn remove_lowest_performing_sponsor(&mut self) {
        if let Some(lowest_sponsor_id) = self
            .sponsors
            .values()
            .min_by_key(|sponsor| sponsor.sponsor_score())
            .map(|sponsor| sponsor.account_id)
        {
            self.sponsors.remove(&lowest_sponsor_id);
        }
    }
}

/// Statistics about the sponsor pool.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PoolStats {
    pub total_sponsors: usize,
    pub eligible_sponsors: usize,
    pub ace_sponsors: usize,
    pub king_sponsors: usize,
    pub max_pool_size: usize,
}
