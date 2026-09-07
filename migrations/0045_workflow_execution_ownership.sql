-- Additive execution ownership. Existing Running rows have no provable owner;
-- do not manufacture leases or replay those historical effects.
ALTER TABLE workflow_runs
    ADD COLUMN execution_token UUID,
    ADD COLUMN execution_epoch BIGINT NOT NULL DEFAULT 0 CHECK (execution_epoch >= 0),
    ADD COLUMN execution_lease_until TIMESTAMPTZ,
    ADD COLUMN execution_uncertain BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX idx_workflow_runs_execution_lease
    ON workflow_runs (execution_lease_until)
    WHERE status = 'running' AND execution_token IS NOT NULL;
