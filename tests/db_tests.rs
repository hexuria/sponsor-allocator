#![cfg(feature = "db")]

use sponsor_allocator::{
    PerformanceBasedStrategy, PgSponsorRepository, RoundRobinStrategy, SponsorCandidate,
    SponsorRepository, SponsorService,
};
use sqlx::PgPool;
use uuid::Uuid;

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/rfn_dev".to_string());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    // Recreate clean database state for the tests
    sqlx::query(
        "DROP TABLE IF EXISTS \
         sponsor_pool, \
         sponsor_account_stats, \
         sponsor_service_state CASCADE",
    )
    .execute(&pool)
    .await
    .expect("Failed to drop old tables");

    let migration_sql =
        include_str!("../migrations/20260623000000_create_sponsor_allocator_tables.sql");
    for statement in migration_sql.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed)
                .execute(&pool)
                .await
                .expect("Failed to run migration statement");
        }
    }

    pool
}

#[tokio::test]
async fn test_empty_sponsor_service_roundtrip() {
    let _lock = DB_LOCK.lock().await;
    let pool = setup_test_db().await;
    let repo = PgSponsorRepository::new(pool);

    let strategy = Box::new(RoundRobinStrategy::new());
    let service = SponsorService::new(strategy, 5);

    // Save
    repo.save(&service)
        .await
        .expect("Failed to save SponsorService");

    // Load back and verify
    let loaded = repo.load().await.expect("Failed to load SponsorService");
    assert_eq!(loaded.get_pool_stats().total_sponsors, 0);
    assert_eq!(loaded.get_pool_stats().max_pool_size, 5);
}

#[tokio::test]
async fn test_sponsor_pool_registration_and_rotation_persistence() {
    let _lock = DB_LOCK.lock().await;
    let pool = setup_test_db().await;
    let repo = PgSponsorRepository::new(pool);

    let strategy = Box::new(RoundRobinStrategy::new());
    let mut service = SponsorService::new(strategy, 3);

    let alice = Uuid::now_v7();
    let bob = Uuid::now_v7();

    // Add eligible candidates
    service
        .add_sponsor(SponsorCandidate::new(alice, "King".to_string(), 0))
        .unwrap();
    service
        .add_sponsor(SponsorCandidate::new(bob, "King".to_string(), 1))
        .unwrap();

    // Allocate first sponsor (should be alice or bob depending on pool ordering)
    let (allocated_1, _) = service.allocate_sponsor(Uuid::now_v7()).unwrap();

    // Save state to database
    repo.save(&service)
        .await
        .expect("Failed to save SponsorService");

    // Load state from database and verify reconstruction
    let mut loaded = repo.load().await.expect("Failed to load SponsorService");
    assert_eq!(loaded.get_pool_stats().total_sponsors, 2);
    assert_eq!(loaded.get_pool_stats().eligible_sponsors, 2);

    // Allocate second sponsor - should rotate to the other sponsor because of the persisted index
    let (allocated_2, _) = loaded.allocate_sponsor(Uuid::now_v7()).unwrap();
    assert_ne!(allocated_1, allocated_2);
}

#[tokio::test]
async fn test_capacity_constraint_and_strategy_switch() {
    let _lock = DB_LOCK.lock().await;
    let pool = setup_test_db().await;
    let repo = PgSponsorRepository::new(pool);

    let strategy = Box::new(RoundRobinStrategy::new());
    let mut service = SponsorService::new(strategy, 2);

    let candidate_id = Uuid::now_v7();
    let mut cand = SponsorCandidate::new(candidate_id, "King".to_string(), 5);
    cand.sponsored_count = 9; // Near capacity

    service.add_sponsor(cand).unwrap();

    // Allocate sponsor (brings capacity to 10/10)
    let (allocated, _) = service.allocate_sponsor(Uuid::now_v7()).unwrap();
    assert_eq!(allocated, candidate_id);

    // Verify it is now ineligible due to full capacity (10/10)
    assert_eq!(service.get_pool_stats().eligible_sponsors, 0);

    // Save to DB
    repo.save(&service)
        .await
        .expect("Failed to save SponsorService at full capacity");

    // Load and switch strategy to performance_based
    let loaded = repo.load().await.expect("Failed to load SponsorService");
    assert_eq!(loaded.get_pool_stats().eligible_sponsors, 0); // Still 0 eligible

    // Create a new service with PerformanceBased strategy
    let perf_strategy = Box::new(PerformanceBasedStrategy::new());
    let mut perf_service = SponsorService::new(perf_strategy, 2);

    // Add eligible candidate
    perf_service
        .add_sponsor(SponsorCandidate::new(candidate_id, "King".to_string(), 5))
        .unwrap();

    // Save Performance-based service
    repo.save(&perf_service)
        .await
        .expect("Failed to save Performance-based service");

    // Load back and verify strategy name matches
    let loaded_perf = repo
        .load()
        .await
        .expect("Failed to load Performance-based service");
    assert_eq!(loaded_perf.get_eligible_sponsors().len(), 1);
}
