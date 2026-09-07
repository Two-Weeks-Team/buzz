use super::*;
use crate::channel::{create_channel, ChannelType, ChannelVisibility};
use crate::user::ensure_user;
use nostr::Keys;

#[tokio::test]
#[ignore = "requires Postgres"]
async fn roleless_invitation_reads_role_after_writer_lock() {
    let url = crate::test_support::database_url();
    let pool = PgPool::connect(&url).await.expect("test DB");
    let community = CommunityId::from_uuid(Uuid::new_v4());
    sqlx::query("INSERT INTO communities (id, host) VALUES ($1, $2)")
        .bind(community.as_uuid())
        .bind(format!("roleless-{}.invalid", community.as_uuid()))
        .execute(&pool)
        .await
        .expect("community");
    let owner = Keys::generate().public_key().to_bytes();
    let target = Keys::generate().public_key().to_bytes();
    for key in [&owner, &target] {
        ensure_user(&pool, community, key).await.expect("user");
    }
    let channel = create_channel(
        &pool,
        community,
        "roleless",
        ChannelType::Stream,
        ChannelVisibility::Private,
        None,
        &owner,
        None,
    )
    .await
    .expect("channel");
    add_member_preserving_role(&pool, community, channel.id, &target, Some(&owner))
        .await
        .expect("new member");
    assert_eq!(
        get_member_role(&pool, community, channel.id, &target)
            .await
            .expect("role"),
        Some("member".into())
    );

    // A second writer is held behind a real Postgres advisory lock. Promote
    // while it waits: a role read before acquiring that lock would demote us.
    let waiter_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("waiter pool");
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&waiter_pool)
        .await
        .expect("pid");
    let mut holder = pool.begin().await.expect("holder");
    acquire_channel_membership_lock(&mut holder, community, channel.id)
        .await
        .expect("lock");
    let waiter = tokio::spawn(async move {
        add_member_preserving_role(&waiter_pool, community, channel.id, &target, Some(&owner)).await
    });
    let mut blocked = false;
    for _ in 0..100 {
        blocked = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND wait_event='advisory')",
        )
        .bind(pid)
        .fetch_one(&pool)
        .await
        .expect("wait state");
        if blocked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(blocked, "writer must actually wait on the membership lock");
    sqlx::query("UPDATE channel_members SET role='admin' WHERE community_id=$1 AND channel_id=$2 AND pubkey=$3")
        .bind(community.as_uuid()).bind(channel.id).bind(target.as_slice()).execute(&mut *holder).await.expect("promote under lock");
    holder.commit().await.expect("commit promotion");
    assert_eq!(
        waiter.await.expect("join").expect("roleless write").role,
        "admin"
    );
    assert_eq!(
        get_member_role(&pool, community, channel.id, &target)
            .await
            .expect("persisted role"),
        Some("admin".into())
    );

    // Explicit role changes remain supported; removed roles are not resurrected.
    add_member(
        &pool,
        community,
        channel.id,
        &target,
        MemberRole::Guest,
        Some(&owner),
    )
    .await
    .expect("explicit guest");
    assert_eq!(
        add_member_preserving_role(&pool, community, channel.id, &target, Some(&owner))
            .await
            .expect("preserve guest")
            .role,
        "guest"
    );
    remove_member(&pool, community, channel.id, &target, &owner)
        .await
        .expect("remove");
    assert_eq!(
        add_member_preserving_role(&pool, community, channel.id, &target, Some(&owner))
            .await
            .expect("reactivate")
            .role,
        "member"
    );
    assert!(
        add_member_preserving_role(&pool, community, channel.id, &owner, Some(&target))
            .await
            .is_err(),
        "role preservation does not bypass private invitation authority"
    );
}

#[tokio::test]
#[ignore = "requires Postgres"]
async fn private_invitation_requires_active_channel_admin() {
    let pool = PgPool::connect(&crate::test_support::database_url())
        .await
        .expect("test DB");
    let community = CommunityId::from_uuid(Uuid::new_v4());
    sqlx::query("INSERT INTO communities (id, host) VALUES ($1, $2)")
        .bind(community.as_uuid())
        .bind(format!("invitation-{}.invalid", community.as_uuid()))
        .execute(&pool)
        .await
        .expect("community");
    let owner = Keys::generate().public_key().to_bytes();
    let member = Keys::generate().public_key().to_bytes();
    let target = Keys::generate().public_key().to_bytes();
    for key in [&owner, &member, &target] {
        ensure_user(&pool, community, key).await.expect("user");
    }
    let channel = create_channel(
        &pool,
        community,
        "private-invitation",
        ChannelType::Stream,
        ChannelVisibility::Private,
        None,
        &owner,
        None,
    )
    .await
    .expect("channel");
    for role in [MemberRole::Member, MemberRole::Bot, MemberRole::Guest] {
        add_member(&pool, community, channel.id, &member, role, Some(&owner))
            .await
            .expect("owner assigns role");
        let denied = add_member(
            &pool,
            community,
            channel.id,
            &target,
            MemberRole::Member,
            Some(&member),
        )
        .await;
        assert!(matches!(denied, Err(DbError::AccessDenied(_))));
        assert!(get_member_role(&pool, community, channel.id, &target)
            .await
            .expect("target lookup")
            .is_none());
        add_member(&pool, community, channel.id, &member, role, Some(&member))
            .await
            .expect("idempotent self");
    }
    add_member(
        &pool,
        community,
        channel.id,
        &target,
        MemberRole::Member,
        Some(&owner),
    )
    .await
    .expect("owner invites");
    remove_member(&pool, community, channel.id, &target, &owner)
        .await
        .expect("remove target");
    assert!(matches!(
        add_member(
            &pool,
            community,
            channel.id,
            &target,
            MemberRole::Member,
            Some(&member)
        )
        .await,
        Err(DbError::AccessDenied(_))
    ));
    add_member(
        &pool,
        community,
        channel.id,
        &member,
        MemberRole::Admin,
        Some(&owner),
    )
    .await
    .expect("promote inviter");
    add_member(
        &pool,
        community,
        channel.id,
        &target,
        MemberRole::Member,
        Some(&member),
    )
    .await
    .expect("admin reactivates target");
    remove_member(&pool, community, channel.id, &target, &owner)
        .await
        .expect("remove target again");
    add_member(
        &pool,
        community,
        channel.id,
        &member,
        MemberRole::Member,
        Some(&owner),
    )
    .await
    .expect("demote inviter");
    assert!(matches!(
        add_member(
            &pool,
            community,
            channel.id,
            &target,
            MemberRole::Member,
            Some(&member)
        )
        .await,
        Err(DbError::AccessDenied(_))
    ));

    let open = create_channel(
        &pool,
        community,
        "open-invitation",
        ChannelType::Stream,
        ChannelVisibility::Open,
        None,
        &owner,
        None,
    )
    .await
    .expect("open channel");
    add_member(&pool, community, open.id, &member, MemberRole::Member, None)
        .await
        .expect("open self join");
    add_member(
        &pool,
        community,
        open.id,
        &target,
        MemberRole::Member,
        Some(&member),
    )
    .await
    .expect("open invitation unchanged");
}
