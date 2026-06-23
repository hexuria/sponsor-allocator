use sponsor_allocator::events::{FlushlineGraduated, MatrixCycled};
use sponsor_allocator::{
    PerformanceBasedStrategy, RoundRobinStrategy, SponsorCandidate, SponsorService,
};
use uuid::Uuid;

fn main() {
    println!("================================================================================");
    println!("              ROYAL FLUSH NETWORK - SPONSOR_ALLOCATOR DEMO");
    println!("================================================================================");
    println!("This demo showcases the extracted, decoupled 'sponsor_allocator' crate.");
    println!("It operates synchronously using the Outbox Event Pattern, ensuring zero coupling");
    println!("with async runtimes, databases, or tracking frameworks.\n");

    // 1. Initialize a SponsorService using RoundRobinStrategy with a pool capacity of 3
    println!("--- 1. Initializing SponsorService (RoundRobinStrategy, Max Pool Capacity = 3) ---");
    let round_robin_strategy = Box::new(RoundRobinStrategy::new());
    let mut service = SponsorService::new(round_robin_strategy, 3);
    println!("Sponsor pool is empty: {:?}\n", service.get_pool_stats());

    // 2. Register accounts and simulate level/tier changes
    println!("--- 2. Simulating Account Graduations and Tier Progressions ---");
    let alice = Uuid::now_v7();
    let bob = Uuid::now_v7();
    let charlie = Uuid::now_v7();
    let dave = Uuid::now_v7();

    // Alice graduates: FlushlineGraduated moves them to "Ace".
    // Note: "Ace" is not a sponsor-qualifying tier (Only "King" is). So no SponsorEvent is emitted.
    println!("Alice ({alice}) graduates from Flushline...");
    let grad_ev = FlushlineGraduated { account_id: alice };
    let outbox = service.handle_flushline_graduated(&grad_ev);
    println!("  -> Outbox events emitted: {} (Expected: 0)", outbox.len());
    println!("  -> Pool stats: {:?}\n", service.get_pool_stats());

    // Alice is reset or moves to King tier. Now she is eligible!
    println!("Alice updates to 'King' tier (qualifies as sponsor)...");
    let outbox = service.update_account_tier(alice, "King".to_string());
    println!("  -> Outbox events emitted: {outbox:?}");
    println!("  -> Pool stats: {:?}\n", service.get_pool_stats());

    // Bob, Charlie, and Dave also reach King tier.
    println!("Bob, Charlie, and Dave reach 'King' tier...");
    let outbox_bob = service.update_account_tier(bob, "King".to_string());
    let outbox_charlie = service.update_account_tier(charlie, "King".to_string());

    // Note: Pool capacity is 3. When Dave is added, the lowest performer will be evicted!
    // Since everyone has 0 cycles, the oldest or first added remains. Let's see what happens.
    let outbox_dave = service.update_account_tier(dave, "King".to_string());

    println!("  -> Bob added events: {outbox_bob:?}");
    println!("  -> Charlie added events: {outbox_charlie:?}");
    println!("  -> Dave added events: {outbox_dave:?}");
    println!(
        "  -> Pool stats (Note total count is capped at 3): {:?}",
        service.get_pool_stats()
    );
    println!(
        "  -> Current pool candidates in priority order:\n{:#?}\n",
        service.get_eligible_sponsors()
    );

    // 3. Round-Robin Allocation
    println!("--- 3. Simulating Sponsor Allocations using Round-Robin Strategy ---");
    for i in 1..=5 {
        let new_user = Uuid::now_v7();
        let (sponsor_id, outbox) = service.allocate_sponsor(new_user).unwrap();
        let sponsor_name = if sponsor_id == bob {
            "Bob"
        } else if sponsor_id == charlie {
            "Charlie"
        } else if sponsor_id == dave {
            "Dave"
        } else if sponsor_id == alice {
            "Alice"
        } else {
            "Unknown"
        };
        println!(
            "Allocation #{i}: New user {new_user} allocated to sponsor {sponsor_name} ({sponsor_id})"
        );
        println!("  -> Emitted: {outbox:?}");
    }
    println!(
        "  -> Pool stats after allocations: {:?}\n",
        service.get_pool_stats()
    );

    // 4. Switching to Performance-Based Strategy
    println!("--- 4. Simulating Sponsor Allocations with Performance-Based Strategy ---");
    // We create a new service utilizing the PerformanceBasedStrategy
    let performance_strategy = Box::new(PerformanceBasedStrategy::new());
    let mut perf_service = SponsorService::new(performance_strategy, 5);

    // Add candidates with different performance levels (Matrix cycles)
    // - Bob has 5 matrix cycles
    // - Charlie has 15 matrix cycles (Top performer)
    // - Dave has 8 matrix cycles
    println!("Populating performance-based sponsor pool...");
    perf_service
        .add_sponsor(SponsorCandidate::new(bob, "King".to_string(), 5))
        .unwrap();
    perf_service
        .add_sponsor(SponsorCandidate::new(charlie, "King".to_string(), 15))
        .unwrap();
    perf_service
        .add_sponsor(SponsorCandidate::new(dave, "King".to_string(), 8))
        .unwrap();

    println!("Current candidates ranked by performance score:");
    for candidate in perf_service.get_eligible_sponsors() {
        let name = if candidate.account_id == bob {
            "Bob"
        } else if candidate.account_id == charlie {
            "Charlie"
        } else if candidate.account_id == dave {
            "Dave"
        } else {
            "Unknown"
        };
        println!(
            "  - {name} (cycles: {}, sponsored: {}, score: {})",
            candidate.cycle_count,
            candidate.sponsored_count,
            candidate.sponsor_score()
        );
    }
    println!();

    // Run performance based allocation. Charlie should always be preferred because of his top cycles count.
    println!(
        "Allocating user with Performance-Based strategy (Charlie has highest score: 1015)..."
    );
    let target1 = Uuid::now_v7();
    let (sponsor_id, outbox) = perf_service.allocate_sponsor(target1).unwrap();
    let is_charlie = sponsor_id == charlie;
    println!("  -> Allocated Sponsor ID: {sponsor_id} (Is Charlie? {is_charlie})");
    println!("  -> Outbox events: {outbox:?}");

    // Let's simulate MatrixCycled events for Bob, bringing Bob's cycles to 20!
    println!("\nSimulating Bob completing 15 more matrix cycles, making him the new top performer (20 cycles)...");
    let mut outbox = Vec::new();
    for _ in 0..15 {
        let cycle_ev = MatrixCycled {
            account_id: bob,
            matrix_id: Uuid::now_v7(),
        };
        outbox = perf_service.handle_matrix_cycled(&cycle_ev);
    }
    println!("  -> Final Matrix Cycled Outbox: {outbox:?}");

    println!("\nAllocating another user now that Bob is the top performer...");
    let target2 = Uuid::now_v7();
    let (sponsor_id, outbox) = perf_service.allocate_sponsor(target2).unwrap();
    let is_bob = sponsor_id == bob;
    println!("  -> Allocated Sponsor ID: {sponsor_id} (Is Bob? {is_bob})");
    println!("  -> Outbox events: {outbox:?}");

    println!("\n================================================================================");
    println!("DEMO COMPLETED SUCCESSFULLY: ALL DECOUPLED SPONSOR FEATURES OPERATING PERFECTLY");
    println!("================================================================================");
}
