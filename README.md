# sponsor_allocator

**Sponsor allocation engine** with **Round-Robin** and **Performance-Based** strategies for the Royal Flush Network (RFN).

This crate handles the registration, scoring, capacity checking, and selection of sponsor candidates when a new user signs up. It operates under a strict, size-bounded pool and produces synchronous outbox events.

Like all sister crates, `sponsor_allocator` is pure domain logic — **no database, no async runtime, no network**. It is completely framework-agnostic.

---

## Core Concepts

### 1. Sponsor Eligibility
To ensure that sponsors remain active and high-performing, candidates are checked against multiple rules:
- **Tier Check**: Only **King** tier accounts (or higher) are eligible to enter the sponsor pool.
- **Sponsorship Capacity**: Each sponsor is limited to a maximum of **10 direct sponsorships**. Once they reach this threshold, they are automatically excluded from the pool.

### 2. Sponsor Scoring (Performance Weights)
Sponsor candidates are ranked using a dynamic performance-based scoring system:
$$\text{Sponsor Score} = \text{Tier Weight} + \text{Matrix Cycles} - (\text{Sponsored Count} \times 2)$$
- **King Tier Weight**: $1000$ points.
- **Sponsored Count Penalty**: Each successfully sponsored account deducts $2$ points from the sponsor's current score to distribute work evenly.

### 3. Allocation Strategies
- **Round-Robin**: Rotates through eligible candidates sequentially. Excellent for fair distribution of new signups.
- **Performance-Based**: Allocates new signups to the candidate with the highest current Sponsor Score. Excellent for rewarding top performers.

### 4. Synchronous Outbox Pattern
Instead of dispatching events through external messaging brokers or async channels directly, the services inside this crate record state changes in a synchronous event queue (the outbox). Methods that mutate state return a `Vec<SponsorEvent>`, which the parent orchestrator can persist, broadcast, or log.

---

## Quick Start

```rust
use sponsor_allocator::events::{FlushlineGraduated, MatrixCycled};
use sponsor_allocator::{
    PerformanceBasedStrategy, RoundRobinStrategy, SponsorCandidate, SponsorService,
};
use uuid::Uuid;

// 1. Initialize Sponsor Service with Round-Robin strategy and a pool capacity of 3
let strategy = Box::new(RoundRobinStrategy::new());
let mut service = SponsorService::new(strategy, 3);

// 2. Simulate account progressions to reach King tier (making them eligible)
let alice = Uuid::now_v7();
let bob = Uuid::now_v7();

service.update_account_tier(alice, "King".to_string());
service.update_account_tier(bob, "King".to_string());

// 3. Allocate a sponsor for a newly signing-up user
let new_signup_acct = Uuid::now_v7();
let (sponsor_id, events) = service.allocate_sponsor(new_signup_acct).unwrap();

// 4. Ingest outbox events to handle side-effects
for event in events {
    println!("Sponsor event generated: {:?}", event);
}
```

---

## Event Ingestion

`sponsor_allocator` is kept in sync with other crates (such as `flushline` or `matrix`) by ingesting their generated events:

```rust
use sponsor_allocator::events::{FlushlineGraduated, MatrixCycled};
use sponsor_allocator::SponsorService;

// When an account graduates from flushline (moves to "Ace" tier)
let grad_event = FlushlineGraduated { account_id: some_account };
let events = service.handle_flushline_graduated(&grad_event);

// When a matrix cycle completes
let cycle_event = MatrixCycled { account_id: some_account, matrix_id: some_matrix };
let events = service.handle_matrix_cycled(&cycle_event);
```

## Setup Requirements

```toml
[dependencies]
sponsor_allocator = { path = "../sponsor_allocator" }
```

```bash
cargo run --example demo           # run the demonstration scenario
cargo test                         # run unit & integration tests
```

## WebAssembly (WASM) & WASI Support

`sponsor_allocator` is fully compatible with WebAssembly **out of the box**. It supports compilation for both browser environments (Leptos frontend clients) and server-side WASM sandboxes (such as **Leptos Spin** or **Leptos Wasmtime**).

### 1. Browser-Side WebAssembly (`wasm32-unknown-unknown`)
Pre-configured with `uuid/js` feature enabled, so generating secure `v7` UUIDs requests secure entropy from browser-native JavaScript APIs (`window.crypto.getRandomValues`).
```bash
cargo check --target wasm32-unknown-unknown
```

### 2. Server-Side WASM / WASI (`wasm32-wasip1`)
Compiles seamlessly to WASI for deployments like Spin and Wasmtime. WASI system calls provide entropy natively.
```bash
cargo check --target wasm32-wasip1
```

## License

MIT.

