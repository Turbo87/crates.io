use crates_io::db;
use crates_io::schema::{crates, versions};
use diesel::prelude::*;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::warn;

/// The root directory of all crates. Hardcoded for now since this is a one-off script.
const CRATES_ROOT_DIR: &str = "/Users/tbieniek/Code/all-crates";

/// Looks for versions that are missing links and backfills them.
///
/// This is useful for versions that were published before the `links` field was
/// added to the database and index.
fn main() -> anyhow::Result<()> {
    crates_io::util::tracing::init();

    // The date of the first version published with a filled-in `links` field.
    let threshold_date = chrono::DateTime::parse_from_rfc3339("2018-03-21T21:00:00Z")?.naive_utc();

    let mut conn = db::oneoff_connection()?;
    let versions = versions::table
        .inner_join(crates::table)
        .select((crates::name, versions::num, versions::id))
        .filter(versions::created_at.lt(threshold_date))
        .filter(versions::links.is_null())
        .load::<(String, String, i32)>(&mut conn)?;

    let num_versions = versions.len();

    let template = "{bar:60} ({pos}/{len}, ETA {eta}) {wide_msg}";
    let pb = ProgressBar::new(num_versions as u64)
        .with_style(ProgressStyle::with_template(template).unwrap());

    let mut versions_with_links = versions
        .par_iter()
        .progress_with(pb.clone())
        .filter_map(|(name, version, id)| {
            let path = Path::new(CRATES_ROOT_DIR)
                .join(crates_io_index::Repository::relative_index_file(name))
                .join(format!("{name}-{version}.crate"));

            let pkgname = format!("{name}-{version}");
            let file = File::open(&path)
                .inspect_err(|err| warn!(?path, "Failed to open file: {err}"))
                .ok()?;

            let tarball = crates_io_tarball::process_tarball(&pkgname, file, u64::MAX)
                .inspect_err(|err| warn!(?path, "Failed to process tarball: {err}"))
                .ok()?;

            let package = tarball.manifest.package.unwrap();

            package.links.map(|links| (name, version, id, links))
        })
        .collect::<Vec<_>>();

    versions_with_links.par_sort();

    let mut file = File::create("links-backfill.sql")?;
    for (name, version, id, links) in versions_with_links {
        writeln!(
            file,
            "UPDATE versions SET links = '{links}' WHERE id = {id}; -- {name} {version}",
        )?;
    }

    Ok(())
}
