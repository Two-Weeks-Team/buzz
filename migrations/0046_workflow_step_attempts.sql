-- No backfill: historical absence is not proof that a step never ran.
CREATE TABLE workflow_step_attempts (
    community_id UUID NOT NULL,
    run_id UUID NOT NULL,
    step_index INT NOT NULL CHECK (step_index >= 0 AND step_index < 4096),
    execution_epoch BIGINT NOT NULL CHECK (execution_epoch > 0),
    step_id TEXT NOT NULL CHECK (octet_length(step_id) BETWEEN 1 AND 256),
    action_digest BYTEA NOT NULL CHECK (octet_length(action_digest) = 32),
    started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    returned_at TIMESTAMPTZ,
    result JSONB CHECK (octet_length(result::text) <= 65536),
    PRIMARY KEY (community_id, run_id, step_index),
    FOREIGN KEY (community_id, run_id) REFERENCES workflow_runs (community_id, id) ON DELETE CASCADE,
    CHECK ((returned_at IS NULL) = (result IS NULL))
);

SELECT attach_community_write_fence('workflow_step_attempts');
