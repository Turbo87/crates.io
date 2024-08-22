use crates_io::db;
use crates_io::schema::{crates, versions};
use crates_io::sql::array_length;
use diesel::prelude::*;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use itertools::Itertools;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use tracing::{info, warn};

/// The root directory of all crates. Hardcoded for now since this is a one-off script.
const CRATES_ROOT_DIR: &str = "/Users/tbieniek/Code/all-crates";

/// The path to the CSV file containing the processed versions.
const CSV_PATH: &str = "lib-bin.csv";

/// The path to the SQL file to generate.
const SQL_PATH: &str = "lib-bin.sql";

/// The number of records to write in a single SQL query.
const CHUNK_SIZE: usize = 1000;

/// Processes all versions without lib/bin information in the database and
/// appends the results to a CSV file. The CSV file is then read and used to
/// generate an SQL file that updates the versions table with the lib/bin
/// information.
fn main() -> anyhow::Result<()> {
    crates_io::util::tracing::init();

    let mut conn = db::oneoff_connection()?;

    info!("Fetching versions without lib/bin information from the database…");
    let versions: Vec<(i32, String, String, i32)> = versions::table
        .inner_join(crates::table)
        .select((
            versions::id,
            crates::name,
            versions::num,
            array_length(versions::bin_names.assume_not_null(), 1),
        ))
        .filter(array_length(versions::bin_names.assume_not_null(), 1).gt(1))
        .get_results(&mut conn)?;

    info!("Reading processed versions from CSV file…");
    let processed_versions = read_csv()?;

    info!("Filtering out already processed versions…");
    let versions: Vec<_> = versions
        .into_iter()
        .filter(|(version_id, _, _, _)| !processed_versions.contains(version_id))
        .collect();

    info!("Processing {} versions…", versions.len());

    let (tx, rx) = channel::<(i32, bool, Vec<String>)>();

    info!("Starting CSV writer thread…");
    let handle = thread::spawn(move || {
        let file = File::options()
            .create(true)
            .append(true)
            .open(CSV_PATH)
            .unwrap();

        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(file);

        for (version_id, has_lib, bin_names) in rx {
            let has_lib = if has_lib { "t" } else { "f" };
            let bin_names = format!("{{{}}}", bin_names.join(","));

            writer
                .write_record(&[&version_id.to_string(), has_lib, &bin_names])
                .unwrap();
        }
    });

    let pb = ProgressBar::new(versions.len() as u64);
    let template = "{bar:60} ({pos}/{len}, ETA {eta}) {wide_msg}";
    pb.set_style(ProgressStyle::with_template(template).unwrap());

    info!("Processing versions…");
    versions.par_iter().progress_with(pb.clone()).for_each(
        |(version_id, name, version, num_bins)| {
            let path = Path::new(CRATES_ROOT_DIR)
                .join(crates_io_index::Repository::relative_index_file(name))
                .join(format!("{name}-{version}.crate"));

            let pkgname = format!("{name}-{version}");
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(err) => {
                    pb.suspend(|| warn!(?path, "Failed to open file: {err}"));
                    return;
                }
            };

            let tarball = match crates_io_tarball::process_tarball(&pkgname, file, u64::MAX) {
                Ok(tarball) => tarball,
                Err(err) => {
                    pb.suspend(|| warn!(?path, "Failed to process tarball: {err}"));
                    return;
                }
            };

            let has_lib = tarball.manifest.lib.is_some();

            let bin_names = tarball
                .manifest
                .bin
                .into_iter()
                .filter_map(|bin| bin.name.clone())
                .collect::<Vec<_>>();

            if bin_names.len() != *num_bins as usize {
                tx.send((*version_id, has_lib, bin_names)).unwrap();
            }
        },
    );

    drop(tx);
    handle.join().unwrap();

    info!("Generating SQL file…");
    let csv_file = File::open(CSV_PATH)?;
    let mut rdr = csv::Reader::from_reader(csv_file);
    let iter = rdr
        .records()
        .into_iter()
        .map(|record| record.unwrap())
        .chunks(CHUNK_SIZE);

    let mut sql_file = File::create(SQL_PATH)?;

    for chunk in &iter {
        writeln!(sql_file, "update versions")?;
        writeln!(
            sql_file,
            "set has_lib = tmp.has_lib, bin_names = tmp.bin_names"
        )?;
        writeln!(sql_file, "from (values")?;

        for (i, record) in chunk.enumerate() {
            let comma = if i < CHUNK_SIZE - 1 { "," } else { "" };
            writeln!(
                sql_file,
                "    ({}, '{}'::bool, '{}'::text[]){}",
                &record[0], &record[1], &record[2], comma
            )?;
        }

        writeln!(sql_file, ") as tmp (version_id, has_lib, bin_names)")?;
        writeln!(
            sql_file,
            "where id = tmp.version_id and versions.has_lib is null;"
        )?;
        // writeln!(sql_file)?;
        // writeln!(sql_file, "select pg_sleep(.25);")?;
        writeln!(sql_file)?;
    }

    Ok(())
}

fn read_csv() -> anyhow::Result<HashSet<i32>> {
    let file = match File::open(CSV_PATH) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HashSet::new());
        }
        Err(err) => return Err(err.into()),
    };

    let mut rdr = csv::Reader::from_reader(file);

    let mut set = HashSet::new();
    for result in rdr.records() {
        let record = result?;
        let version_id: i32 = record[0].parse()?;
        set.insert(version_id);
    }

    Ok(set)
}
