use crate::fns::{compute_default_version, rebuild_default_version};
use crate::schema::default_versions;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use tracing::{debug, instrument, warn};

/// Updates the `default_versions` table entry for the specified crate.
///
/// The actual calculation and update happen inside the `rebuild_default_version`
/// database function, which is normally invoked automatically by triggers on the
/// `versions` table. This wrapper exists for explicit recomputation, e.g. from
/// the `crates-admin` tooling.
///
/// The default version is determined by the following criteria:
///
/// 1. The highest non-prerelease version that is not yanked.
/// 2. The highest non-yanked version.
/// 3. The highest version.
#[instrument(skip(conn))]
pub async fn update_default_version(
    crate_id: i32,
    mut conn: &AsyncPgConnection,
) -> QueryResult<()> {
    debug!("Rebuilding default version for crate {crate_id}…");

    diesel::select(rebuild_default_version(crate_id))
        .execute(&mut conn)
        .await?;

    Ok(())
}

/// Verifies that the default version for the specified crate is up-to-date.
#[instrument(skip(conn))]
pub async fn verify_default_version(
    crate_id: i32,
    mut conn: &AsyncPgConnection,
) -> QueryResult<()> {
    let calculated: Option<i32> = diesel::select(compute_default_version(crate_id))
        .get_result(&mut conn)
        .await?;

    let saved = default_versions::table
        .select(default_versions::version_id)
        .filter(default_versions::crate_id.eq(crate_id))
        .first::<i32>(&mut conn)
        .await
        .optional()?;

    match (saved, calculated) {
        (Some(saved), Some(calculated)) if saved == calculated => {
            debug!("Default version for crate {crate_id} is up to date");
        }
        (Some(saved), Some(calculated)) => {
            warn!(
                "Default version for crate {crate_id} is outdated (expected: {calculated}, actual: {saved})"
            );
        }
        (Some(saved), None) => {
            warn!("Default version for crate {crate_id} should not exist (actual: {saved})");
        }
        (None, Some(calculated)) => {
            warn!("Default version for crate {crate_id} is missing (expected: {calculated})");
        }
        (None, None) => {
            debug!("Crate {crate_id} has no versions and no default version, as expected");
        }
    }

    Ok(())
}
