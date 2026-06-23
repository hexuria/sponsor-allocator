use sponsor_allocator::events::{FlushlineGraduated, MatrixCycled};
use sponsor_allocator::{
    AllocationStrategy, PerformanceBasedStrategy, RoundRobinStrategy, SponsorCandidate,
    SponsorEvent, SponsorPool, SponsorService,
};
use uuid::Uuid;

#[test]
fn test_sponsor_candidate_eligibility_and_scoring() {
    let sponsor_id = Uuid::now_v7();

    // Only "King" tier is eligible for sponsorship
    let ten = SponsorCandidate::new(sponsor_id, "Ten".to_string(), 0);
    assert!(!ten.is_top_performer());
    assert!(!ten.is_eligible_for_sponsorship());
    assert_eq!(ten.sponsor_score(), 0);

    let king = SponsorCandidate::new(sponsor_id, "King".to_string(), 5);
    assert!(king.is_top_performer());
    assert!(king.is_eligible_for_sponsorship());
    assert_eq!(king.sponsor_score(), 1005); // 1000 + 5

    // Capacity limit of 10 sponsored accounts
    let mut busy_king = king.clone();
    busy_king.sponsored_count = 10;
    assert!(!busy_king.is_eligible_for_sponsorship());
    assert_eq!(busy_king.sponsor_score(), 985); // 1000 + 5 - (10 * 2)
}

#[test]
fn test_sponsor_pool_operations_and_bounds() {
    let mut pool = SponsorPool::new(3);

    let id1 = Uuid::now_v7();
    let id2 = Uuid::now_v7();
    let id3 = Uuid::now_v7();
    let id4 = Uuid::now_v7();

    let s1 = SponsorCandidate::new(id1, "King".to_string(), 5);
    let s2 = SponsorCandidate::new(id2, "King".to_string(), 15);
    let s3 = SponsorCandidate::new(id3, "King".to_string(), 10);
    let s4 = SponsorCandidate::new(id4, "King".to_string(), 1);

    // Adding candidates
    assert!(pool.add_sponsor(s1.clone()).is_ok());
    assert!(pool.add_sponsor(s2.clone()).is_ok());
    assert!(pool.add_sponsor(s3.clone()).is_ok());

    // Pool size is 3 (at max)
    assert_eq!(pool.pool_stats().total_sponsors, 3);

    // Adding a 4th sponsor will trigger removal of the lowest-performing sponsor (which is s1 with 5 cycles, since s4 is not in the pool yet, wait, let's see. If we add s4, s4 has 1 cycle, s1 has 5, s2 has 15, s3 has 10. The lowest of the existing 3 is s1. So s1 is removed, s4 is added. Let's assert on this!)
    assert!(pool.add_sponsor(s4.clone()).is_ok());
    assert_eq!(pool.pool_stats().total_sponsors, 3);

    // s1 should have been removed because it was the lowest performer among s1, s2, s3 when s4 was added.
    assert!(pool.remove_sponsor(id1).is_err());
    assert!(pool.remove_sponsor(id2).is_ok());
    assert!(pool.remove_sponsor(id3).is_ok());
    assert!(pool.remove_sponsor(id4).is_ok());
}

#[test]
fn test_round_robin_allocation() {
    let mut strategy = RoundRobinStrategy::new();
    let mut pool = SponsorPool::new(10);

    let id1 = Uuid::now_v7();
    let id2 = Uuid::now_v7();
    let id3 = Uuid::now_v7();

    pool.add_sponsor(SponsorCandidate::new(id1, "King".to_string(), 0))
        .unwrap();
    pool.add_sponsor(SponsorCandidate::new(id2, "King".to_string(), 0))
        .unwrap();
    pool.add_sponsor(SponsorCandidate::new(id3, "King".to_string(), 0))
        .unwrap();

    let mut eligible = pool.get_eligible_sponsors();
    eligible.sort_by_key(|s| s.account_id);
    assert_eq!(eligible.len(), 3);

    // Test allocations cycle through available sponsors in pool order
    let a1 = strategy.allocate_sponsor(&pool, Uuid::now_v7()).unwrap();
    let a2 = strategy.allocate_sponsor(&pool, Uuid::now_v7()).unwrap();
    let a3 = strategy.allocate_sponsor(&pool, Uuid::now_v7()).unwrap();
    let a4 = strategy.allocate_sponsor(&pool, Uuid::now_v7()).unwrap();

    assert_eq!(a1, eligible[0].account_id);
    assert_eq!(a2, eligible[1].account_id);
    assert_eq!(a3, eligible[2].account_id);
    assert_eq!(a4, eligible[0].account_id); // Back to first
}

#[test]
fn test_performance_based_allocation() {
    let mut strategy = PerformanceBasedStrategy::new();
    let mut pool = SponsorPool::new(10);

    let id1 = Uuid::now_v7();
    let id2 = Uuid::now_v7();
    let id3 = Uuid::now_v7();

    pool.add_sponsor(SponsorCandidate::new(id1, "King".to_string(), 5))
        .unwrap();
    pool.add_sponsor(SponsorCandidate::new(id2, "King".to_string(), 20))
        .unwrap(); // Highest cycles
    pool.add_sponsor(SponsorCandidate::new(id3, "King".to_string(), 10))
        .unwrap();

    // Performance strategy should always select the highest-scoring candidate (id2)
    let selected = strategy.allocate_sponsor(&pool, Uuid::now_v7()).unwrap();
    assert_eq!(selected, id2);
}

#[test]
fn test_service_event_ingestion_and_outbox() {
    let strategy = Box::new(RoundRobinStrategy::new());
    let mut service = SponsorService::new(strategy, 5);

    let id1 = Uuid::now_v7();

    // 1. Ingest FlushlineGraduated for id1. This moves them to "Ace" tier.
    // Ace tier is NOT a top performer (only King is), so no SponsorAdded event should occur.
    let ev1 = FlushlineGraduated { account_id: id1 };
    let outbox1 = service.handle_flushline_graduated(&ev1);
    assert!(outbox1.is_empty());
    assert_eq!(service.get_pool_stats().total_sponsors, 0);

    // 2. Update id1 to "King" tier.
    // King tier with 0 cycles is a top performer, so they should be added and emit SponsorAdded.
    let outbox2 = service.update_account_tier(id1, "King".to_string());
    assert_eq!(outbox2.len(), 1);
    match &outbox2[0] {
        SponsorEvent::SponsorAdded {
            sponsor_id,
            tier,
            cycle_count,
        } => {
            assert_eq!(*sponsor_id, id1);
            assert_eq!(tier, "King");
            assert_eq!(*cycle_count, 0);
        }
        _ => panic!("Expected SponsorAdded event"),
    }
    assert_eq!(service.get_pool_stats().total_sponsors, 1);

    // 3. Ingest MatrixCycled for id1. This increments its cycle count to 1 and keeps them in King.
    // They are already in the pool, but we update them and re-add them. Let's see.
    let cycle_ev = MatrixCycled {
        account_id: id1,
        matrix_id: Uuid::now_v7(),
    };
    let outbox3 = service.handle_matrix_cycled(&cycle_ev);
    assert_eq!(outbox3.len(), 1);
    match &outbox3[0] {
        SponsorEvent::SponsorAdded {
            sponsor_id,
            tier,
            cycle_count,
        } => {
            assert_eq!(*sponsor_id, id1);
            assert_eq!(tier, "King");
            assert_eq!(*cycle_count, 1);
        }
        _ => panic!("Expected SponsorAdded event"),
    }

    // 4. Allocate a sponsor.
    let new_acct = Uuid::now_v7();
    let (allocated_id, outbox4) = service.allocate_sponsor(new_acct).unwrap();
    assert_eq!(allocated_id, id1);
    assert_eq!(outbox4.len(), 1);
    match &outbox4[0] {
        SponsorEvent::SponsorAllocated {
            sponsor_id,
            sponsored_account_id,
            allocation_strategy,
        } => {
            assert_eq!(*sponsor_id, id1);
            assert_eq!(*sponsored_account_id, new_acct);
            assert_eq!(allocation_strategy, "round_robin");
        }
        _ => panic!("Expected SponsorAllocated event"),
    }
}
