use super::*;
use crate::channel::{create_channel, ChannelType, ChannelVisibility};
use crate::user::ensure_user;
use nostr::Keys;

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
