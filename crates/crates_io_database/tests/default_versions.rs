//! Tests for the database triggers that maintain the `default_versions` summary
//! table. Each test drives the `versions` table with raw inserts/updates/deletes
//! and asserts the resulting default version, so the trigger behaviour is
//! exercised directly rather than through the HTTP layer.

use crates_io_database::schema::{crates, default_versions, versions};
use crates_io_test_db::TestDatabase;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

/// A crate's first version automatically becomes its default version, and the
/// version count is initialized to 1.
#[tokio::test]
async fn first_version_becomes_default() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    assert_eq!(stored_default(&mut conn, c1).await, None);

    create_version(&mut conn, c1, "1.0.0").await;
    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.0.0".into(), 1))
    );
}

/// Inserting a higher version moves the default to it without any explicit
/// recomputation, and bumps the version count.
#[tokio::test]
async fn higher_version_becomes_default_on_insert() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    create_version(&mut conn, c1, "1.0.0").await;
    create_version(&mut conn, c1, "1.1.0").await;

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.1.0".into(), 2))
    );
}

/// Inserting a lower version leaves the default untouched but still counts it.
#[tokio::test]
async fn lower_version_does_not_change_default() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    create_version(&mut conn, c1, "1.0.0").await;
    create_version(&mut conn, c1, "0.9.0").await;

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.0.0".into(), 2))
    );
}

/// A pre-release is not preferred over a release, even when it sorts higher.
#[tokio::test]
async fn prerelease_not_preferred_over_release() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    create_version(&mut conn, c1, "1.0.0").await;
    create_version(&mut conn, c1, "2.0.0-beta.1").await;

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.0.0".into(), 2))
    );
}

/// Yanking the default version moves the default to the next best version, and
/// unyanking restores it. The version count is unaffected by yanking.
#[tokio::test]
async fn yanking_default_moves_to_next_and_unyank_restores() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    create_version(&mut conn, c1, "1.0.0").await;
    let v2 = create_version(&mut conn, c1, "1.1.0").await;

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.1.0".into(), 2))
    );

    set_yanked(&mut conn, v2, true).await;
    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.0.0".into(), 2))
    );

    set_yanked(&mut conn, v2, false).await;
    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.1.0".into(), 2))
    );
}

/// When all versions are yanked, the highest yanked version is the default.
#[tokio::test]
async fn highest_yanked_version_is_default_when_all_yanked() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    let v1 = create_version(&mut conn, c1, "1.0.0").await;
    let v2 = create_version(&mut conn, c1, "1.1.0").await;

    set_yanked(&mut conn, v1, true).await;
    set_yanked(&mut conn, v2, true).await;

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.1.0".into(), 2))
    );
}

/// Deleting the default version moves the default to the next best version. The
/// delete and the `default_versions` reconciliation happen in one statement, so
/// the deferred foreign key check passes at commit.
#[tokio::test]
async fn deleting_default_moves_to_next() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    create_version(&mut conn, c1, "1.0.0").await;
    let v2 = create_version(&mut conn, c1, "1.1.0").await;

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.1.0".into(), 2))
    );

    conn.transaction(async |conn| {
        diesel::delete(versions::table.filter(versions::id.eq(v2)))
            .execute(conn)
            .await
    })
    .await
    .unwrap();

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.0.0".into(), 1))
    );
}

/// Deleting the last remaining version removes the (now dangling)
/// `default_versions` row.
#[tokio::test]
async fn deleting_last_version_removes_default() {
    let test_db = TestDatabase::new();
    let mut conn = test_db.async_connect().await;

    let c1 = create_crate(&mut conn, "c1").await;
    let v1 = create_version(&mut conn, c1, "1.0.0").await;

    assert_eq!(
        stored_default(&mut conn, c1).await,
        Some(("1.0.0".into(), 1))
    );

    conn.transaction(async |conn| {
        diesel::delete(versions::table.filter(versions::id.eq(v1)))
            .execute(conn)
            .await
    })
    .await
    .unwrap();

    assert_eq!(stored_default(&mut conn, c1).await, None);
}

/// Creates a crate and returns its id.
async fn create_crate(conn: &mut AsyncPgConnection, name: &str) -> i32 {
    diesel::insert_into(crates::table)
        .values(crates::name.eq(name))
        .returning(crates::id)
        .get_result(conn)
        .await
        .unwrap()
}

/// Creates a version of `crate_id` and returns its id.
async fn create_version(conn: &mut AsyncPgConnection, crate_id: i32, num: &str) -> i32 {
    diesel::insert_into(versions::table)
        .values((
            versions::crate_id.eq(crate_id),
            versions::num.eq(num),
            versions::num_no_build.eq(num),
            versions::checksum.eq(""),
            versions::crate_size.eq(0),
        ))
        .returning(versions::id)
        .get_result(conn)
        .await
        .unwrap()
}

/// Sets a version's yank status.
async fn set_yanked(conn: &mut AsyncPgConnection, version_id: i32, yanked: bool) {
    diesel::update(versions::table.filter(versions::id.eq(version_id)))
        .set(versions::yanked.eq(yanked))
        .execute(conn)
        .await
        .unwrap();
}

/// Loads the stored default version of `crate_id` as `(version number, version
/// count)`, or `None` if the crate has no `default_versions` row.
async fn stored_default(conn: &mut AsyncPgConnection, crate_id: i32) -> Option<(String, i32)> {
    default_versions::table
        .inner_join(versions::table)
        .filter(default_versions::crate_id.eq(crate_id))
        .select((versions::num, default_versions::num_versions))
        .first::<(String, Option<i32>)>(conn)
        .await
        .optional()
        .unwrap()
        .map(|(num, count)| (num, count.unwrap_or_default()))
}
