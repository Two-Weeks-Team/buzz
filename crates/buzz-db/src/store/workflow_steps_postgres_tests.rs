use super::*;
use sqlx::{AssertSqlSafe, PgPool};

async fn fixture(pool: &PgPool) -> (CommunityId, Uuid) {
    let community = CommunityId::from_uuid(Uuid::new_v4());
    let channel = Uuid::new_v4();
    let workflow = Uuid::new_v4();
    let owner = [0x82u8; 32];
    sqlx::query("INSERT INTO communities(id,host) VALUES($1,$2)")
        .bind(community.as_uuid())
        .bind(format!("{}.example", community.as_uuid()))
        .execute(pool)
        .await
        .unwrap();
    crate::user::ensure_user(pool, community, &owner)
        .await
        .unwrap();
    sqlx::query("INSERT INTO channels(id,community_id,name,created_by) VALUES($1,$2,'journal',$3)")
        .bind(channel)
        .bind(community.as_uuid())
        .bind(&owner[..])
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO workflows(id,community_id,name,owner_pubkey,channel_id,definition,definition_hash,status,enabled)
        VALUES($1,$2,'journal',$3,$4,'{}',$5,'active',true)")
        .bind(workflow).bind(community.as_uuid()).bind(&owner[..]).bind(channel).bind(&[0u8;32][..]).execute(pool).await.unwrap();
    let run = crate::workflow::create_workflow_run(pool, community, workflow, None, None)
        .await
        .unwrap();
    (community, run)
}

async fn exercise_journal() {
    let pool = PgPool::connect(&crate::test_support::database_url())
        .await
        .unwrap();
    let db = Db::from_pool(pool.clone());
    let fenced: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid='workflow_step_attempts'::regclass AND tgname='community_write_fence_workflow_step_attempts' AND NOT tgisinternal)")
        .fetch_one(&pool).await.unwrap();
    assert!(
        fenced,
        "step journal must participate in community write fencing"
    );
    let (community, run) = fixture(&pool).await;
    let claim = db
        .claim_workflow_execution(community, run, Uuid::new_v4(), None)
        .await
        .unwrap()
        .unwrap();
    let foreign = CommunityId::from_uuid(Uuid::new_v4());
    let wrong = WorkflowExecutionClaim {
        token: Uuid::new_v4(),
        ..claim
    };
    for (scope, owner) in [(foreign, claim), (community, wrong)] {
        assert!(!db
            .begin_workflow_step_attempt(scope, run, owner, 0, "a", &[1; 32])
            .await
            .unwrap());
    }
    assert!(!db
        .begin_workflow_step_attempt(community, run, claim, 1, "future", &[1; 32])
        .await
        .unwrap());
    let (a, b) = tokio::join!(
        db.begin_workflow_step_attempt(community, run, claim, 0, "a", &[1; 32]),
        db.begin_workflow_step_attempt(community, run, claim, 0, "a", &[1; 32])
    );
    assert_eq!(u8::from(a.unwrap()) + u8::from(b.unwrap()), 1);
    let result = serde_json::json!({"status":"completed","output":{"receipt":"synthetic"}});
    // Fail the second write, after the attempt result UPDATE. Neither result
    // nor current_step may survive that transaction's rollback.
    let fault = format!("journal_fault_{}", run.simple());
    // Audited identifiers contain only a fixed prefix and locally generated UUID
    // hex; the sole interpolated value is the typed UUID, never external input.
    sqlx::query(AssertSqlSafe(format!("CREATE FUNCTION {fault}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='{run}'::uuid AND NEW.current_step=1 THEN RAISE EXCEPTION 'synthetic progress failure'; END IF; RETURN NEW; END $$")))
        .execute(&pool).await.unwrap();
    sqlx::query(AssertSqlSafe(format!("CREATE TRIGGER {fault} BEFORE UPDATE ON workflow_runs FOR EACH ROW EXECUTE FUNCTION {fault}()")))
        .execute(&pool).await.unwrap();
    assert!(db
        .record_workflow_step_return(community, run, claim, 0, &result, true)
        .await
        .is_err());
    let rolled_back: bool = sqlx::query_scalar("SELECT a.result IS NULL AND r.current_step=0 FROM workflow_step_attempts a JOIN workflow_runs r ON r.community_id=a.community_id AND r.id=a.run_id WHERE a.community_id=$1 AND a.run_id=$2 AND a.step_index=0")
        .bind(community.as_uuid()).bind(run).fetch_one(&pool).await.unwrap();
    assert!(rolled_back);
    sqlx::query(AssertSqlSafe(format!(
        "DROP TRIGGER {fault} ON workflow_runs"
    )))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(AssertSqlSafe(format!("DROP FUNCTION {fault}()")))
        .execute(&pool)
        .await
        .unwrap();
    assert!(!db
        .record_workflow_step_return(community, run, wrong, 0, &result, true)
        .await
        .unwrap());
    assert!(db
        .record_workflow_step_return(community, run, claim, 0, &result, true)
        .await
        .unwrap());
    assert!(!db
        .record_workflow_step_return(
            community,
            run,
            claim,
            0,
            &serde_json::json!({"overwrite":true}),
            true
        )
        .await
        .unwrap());
    let stored:serde_json::Value=sqlx::query_scalar("SELECT result FROM workflow_step_attempts WHERE community_id=$1 AND run_id=$2 AND step_index=0")
        .bind(community.as_uuid()).bind(run).fetch_one(&pool).await.unwrap();
    assert_eq!(stored, result);
    assert!(db
        .begin_workflow_step_attempt(community, run, claim, 1, "b", &[2; 32])
        .await
        .unwrap());
    // Only this synthetic run is expired. A stale return cannot overwrite intent
    // or advance its progress; absence of a return is not evidence of no effects.
    sqlx::query("UPDATE workflow_runs SET execution_lease_until=clock_timestamp()-interval '1 second' WHERE community_id=$1 AND id=$2")
        .bind(community.as_uuid()).bind(run).execute(&pool).await.unwrap();
    assert!(!db
        .record_workflow_step_return(community, run, claim, 1, &result, true)
        .await
        .unwrap());
    assert!(!db
        .begin_workflow_step_attempt(community, run, claim, 1, "b", &[2; 32])
        .await
        .unwrap());
    let state:(i32,Option<serde_json::Value>)=sqlx::query_as("SELECT r.current_step,a.result FROM workflow_runs r JOIN workflow_step_attempts a ON a.community_id=r.community_id AND a.run_id=r.id WHERE r.community_id=$1 AND r.id=$2 AND a.step_index=1")
        .bind(community.as_uuid()).bind(run).fetch_one(&pool).await.unwrap();
    assert_eq!(state, (1, None));
    assert!(db
        .record_workflow_step_return(
            community,
            run,
            claim,
            1,
            &serde_json::json!({"huge":"x".repeat(32769)}),
            true
        )
        .await
        .is_err());
}

#[tokio::test]
#[ignore = "requires Postgres"]
async fn workflow_step_journal_fences_intent_and_return() {
    exercise_journal().await;
}

#[tokio::test]
#[ignore = "requires Postgres"]
async fn migration_schema_workflow_step_journal() {
    let pool = PgPool::connect(&crate::test_support::database_url())
        .await
        .unwrap();
    Db::from_pool(pool).migrate().await.unwrap();
    exercise_journal().await;
}
