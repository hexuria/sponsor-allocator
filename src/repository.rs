//! Repository for persisting and loading SponsorService aggregates to/from PostgreSQL.

use crate::pool::{SponsorCandidate, SponsorPool};
use crate::service::SponsorService;
use crate::strategy::{AllocationStrategy, PerformanceBasedStrategy, RoundRobinStrategy};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

/// Repository interface for SponsorService aggregate persistence.
#[async_trait]
pub trait SponsorRepository: Send + Sync {
    /// Load the complete SponsorService state from the database.
    async fn load(&self) -> Result<SponsorService, String>;

    /// Save the complete SponsorService state to the database transactionally.
    async fn save(&self, service: &SponsorService) -> Result<(), String>;
}

/// Postgres-backed implementation of [`SponsorRepository`].
#[derive(Debug, Clone)]
pub struct PgSponsorRepository {
    pool: PgPool,
}

impl PgSponsorRepository {
    /// Create a new PostgreSQL repository instance.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SponsorRepository for PgSponsorRepository {
    async fn load(&self) -> Result<SponsorService, String> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| format!("Failed to acquire DB connection: {e}"))?;

        // 1. Fetch global sponsor service state
        let state_row = sqlx::query(
            "SELECT active_strategy, last_allocated_index, max_pool_size \
             FROM sponsor_service_state WHERE id = 1",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| format!("Failed to fetch service state: {e}"))?;

        let (active_strategy, last_allocated_index, max_pool_size) = match state_row {
            Some(row) => {
                let strat: String = row.get("active_strategy");
                let idx: i32 = row.get("last_allocated_index");
                let max_size: i32 = row.get("max_pool_size");
                (strat, idx as usize, max_size as usize)
            }
            None => ("round_robin".to_string(), 0, 10),
        };

        // 2. Fetch all known account stats
        let stats_rows =
            sqlx::query("SELECT account_id, tier, cycle_count FROM sponsor_account_stats")
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| format!("Failed to fetch account stats: {e}"))?;

        let mut account_tiers = HashMap::new();
        let mut account_cycles = HashMap::new();

        for r in stats_rows {
            let acct_uuid: Uuid = r.get("account_id");
            let tier: String = r.get("tier");
            let cycles: i32 = r.get("cycle_count");

            account_tiers.insert(acct_uuid, tier);
            account_cycles.insert(acct_uuid, cycles as u32);
        }

        // 3. Fetch active sponsor pool members
        let pool_rows = sqlx::query("SELECT account_id, sponsored_count FROM sponsor_pool")
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| format!("Failed to fetch pool members: {e}"))?;

        let mut sponsors = HashMap::new();
        for r in pool_rows {
            let acct_id: Uuid = r.get("account_id");
            let sponsored_count: i32 = r.get("sponsored_count");

            // Reconstruct the full candidate using local account stats
            let tier = account_tiers
                .get(&acct_id)
                .cloned()
                .unwrap_or_else(|| "King".to_string());
            let cycle_count = account_cycles.get(&acct_id).cloned().unwrap_or(0);

            sponsors.insert(
                acct_id,
                SponsorCandidate {
                    account_id: acct_id,
                    tier,
                    cycle_count,
                    sponsored_count: sponsored_count as u32,
                },
            );
        }

        // 4. Reconstruct Strategy
        let strategy: Box<dyn AllocationStrategy + Send + Sync> = match active_strategy.as_str() {
            "performance_based" => Box::new(PerformanceBasedStrategy::new()),
            _ => {
                let mut rr = RoundRobinStrategy::new();
                rr.set_last_allocated_index(last_allocated_index);
                Box::new(rr)
            }
        };

        let sponsor_pool = SponsorPool {
            sponsors,
            max_pool_size,
        };

        Ok(SponsorService {
            sponsor_pool,
            allocation_strategy: strategy,
            account_tiers,
            account_cycles,
        })
    }

    async fn save(&self, service: &SponsorService) -> Result<(), String> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Failed to begin transaction: {e}"))?;

        let strat_name = service.allocation_strategy.strategy_name();
        let last_idx = service
            .allocation_strategy
            .last_allocated_index()
            .unwrap_or(0) as i32;
        let max_size = service.sponsor_pool.max_pool_size as i32;

        // 1. Upsert global service state
        sqlx::query(
            "INSERT INTO sponsor_service_state (id, active_strategy, last_allocated_index, max_pool_size) \
             VALUES (1, $1, $2, $3) \
             ON CONFLICT (id) DO UPDATE SET \
                active_strategy = EXCLUDED.active_strategy, \
                last_allocated_index = EXCLUDED.last_allocated_index, \
                max_pool_size = EXCLUDED.max_pool_size, \
                updated_at = NOW()"
        )
        .bind(strat_name)
        .bind(last_idx)
        .bind(max_size)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to upsert service state: {e}"))?;

        // 2. Gather and upsert all account stats
        let mut all_accounts = std::collections::HashSet::new();
        all_accounts.extend(service.account_tiers.keys().cloned());
        all_accounts.extend(service.account_cycles.keys().cloned());

        for acct_id in all_accounts {
            let tier = service
                .account_tiers
                .get(&acct_id)
                .cloned()
                .unwrap_or_else(|| "Ten".to_string());
            let cycle_count = service.account_cycles.get(&acct_id).cloned().unwrap_or(0) as i32;

            sqlx::query(
                "INSERT INTO sponsor_account_stats (account_id, tier, cycle_count) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (account_id) DO UPDATE SET \
                    tier = EXCLUDED.tier, \
                    cycle_count = EXCLUDED.cycle_count, \
                    updated_at = NOW()",
            )
            .bind(acct_id)
            .bind(tier)
            .bind(cycle_count)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to upsert account stats: {e}"))?;
        }

        // 3. Clear existing active pool records (the cascading FK deletes or simple clear)
        sqlx::query("DELETE FROM sponsor_pool")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to clear old sponsor pool members: {e}"))?;

        // 4. Insert current active pool members
        for candidate in service.sponsor_pool.sponsors.values() {
            // Ensure a parent stats row exists for this pool member (should already, but let's be fully robust)
            sqlx::query(
                "INSERT INTO sponsor_account_stats (account_id, tier, cycle_count) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (account_id) DO UPDATE SET \
                    tier = EXCLUDED.tier, \
                    cycle_count = EXCLUDED.cycle_count, \
                    updated_at = NOW()",
            )
            .bind(candidate.account_id)
            .bind(&candidate.tier)
            .bind(candidate.cycle_count as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to ensure parent stats for pool member: {e}"))?;

            // Insert into sponsor pool
            sqlx::query(
                "INSERT INTO sponsor_pool (account_id, sponsored_count) \
                 VALUES ($1, $2)",
            )
            .bind(candidate.account_id)
            .bind(candidate.sponsored_count as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to insert pool member: {e}"))?;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit transaction: {e}"))?;

        Ok(())
    }
}
