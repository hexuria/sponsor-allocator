-- 1. Sponsor service global configuration and state (holds a single configuration row)
CREATE TABLE sponsor_service_state (
    id INTEGER PRIMARY KEY DEFAULT 1 CONSTRAINT check_single_row CHECK (id = 1),
    active_strategy VARCHAR(50) NOT NULL DEFAULT 'round_robin',
    last_allocated_index INTEGER NOT NULL DEFAULT 0 CONSTRAINT check_non_negative_index CHECK (last_allocated_index >= 0),
    max_pool_size INTEGER NOT NULL DEFAULT 10,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Initialize global state
INSERT INTO sponsor_service_state (id, active_strategy, last_allocated_index, max_pool_size) 
VALUES (1, 'round_robin', 0, 10) 
ON CONFLICT DO NOTHING;

-- 2. Known account stats (local Read-Model for fast eligibility updates)
CREATE TABLE sponsor_account_stats (
    account_id UUID PRIMARY KEY,
    tier VARCHAR(20) NOT NULL DEFAULT 'Ten',
    cycle_count INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- 3. Active Sponsor Pool members
CREATE TABLE sponsor_pool (
    account_id UUID PRIMARY KEY REFERENCES sponsor_account_stats(account_id) ON DELETE CASCADE,
    sponsored_count INTEGER NOT NULL DEFAULT 0 CONSTRAINT check_capacity CHECK (sponsored_count >= 0 AND sponsored_count <= 10),
    joined_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Index to quickly query and sort pool members during allocation
CREATE INDEX idx_sponsor_pool_capacity ON sponsor_pool (sponsored_count);
