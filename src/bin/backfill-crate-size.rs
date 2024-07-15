use bigdecimal::ToPrimitive;
use crates_io::db;
use crates_io::schema::{crates, versions};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
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
const CSV_PATH: &str = "crate-size.csv";

/// The path to the SQL file to generate.
const SQL_PATH: &str = "crate-size.sql";

/// The number of records to write in a single SQL query.
const CHUNK_SIZE: usize = 10000;

/// Processes all versions without `crate_size` information in the database and
/// appends the results to a CSV file. The CSV file is then read and used to
/// generate an SQL file that updates the versions table with the `crate_size`
/// information.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    crates_io::util::tracing::init();

    let mut conn = db::oneoff_connection().await?;

    info!("Fetching versions without lib/bin information from the database…");
    let versions: Vec<(i32, String, String)> = versions::table
        .filter(versions::crate_size.is_null())
        .inner_join(crates::table)
        .select((versions::id, crates::name, versions::num))
        .get_results(&mut conn)
        .await?;

    drop(conn);

    info!("Reading processed versions from CSV file…");
    let processed_versions = read_csv()?;

    info!("Filtering out already processed versions…");
    let versions: Vec<_> = versions
        .into_iter()
        .filter(|(version_id, _, _)| !processed_versions.contains(version_id))
        .collect();

    info!("Processing {} versions…", versions.len());

    let (tx, rx) = channel::<(i32, i32)>();

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

        for (version_id, size) in rx {
            writer
                .write_record(&[&version_id.to_string(), &size.to_string()])
                .unwrap();
        }
    });

    let pb = ProgressBar::new(versions.len() as u64);
    let template = "{bar:60} ({pos}/{len}, ETA {eta}) {wide_msg}";
    pb.set_style(ProgressStyle::with_template(template)?);

    info!("Processing versions…");
    versions
        .par_iter()
        .progress_with(pb.clone())
        .for_each(|(version_id, name, version)| {
            let path = Path::new(CRATES_ROOT_DIR)
                .join(crates_io_index::Repository::relative_index_file(name))
                .join(format!("{name}-{version}.crate"));

            let size = match std::fs::metadata(&path) {
                Ok(metadata) => metadata.len(),
                Err(err) => {
                    pb.suspend(|| warn!(?path, "Failed to fetch metadata for file: {err}"));
                    return;
                }
            };

            let size = match size.to_i32() {
                Some(size) => size,
                None => {
                    pb.suspend(|| warn!(?path, "File is to large to fit into i32: {size}"));
                    return;
                }
            };

            tx.send((*version_id, size)).unwrap();
        });

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
        writeln!(sql_file, "set crate_size = tmp.crate_size")?;
        writeln!(sql_file, "from (values")?;

        for (i, record) in chunk.enumerate() {
            if i > 0 {
                writeln!(sql_file, ",")?;
            }
            write!(sql_file, "    ({}, {})", &record[0], &record[1])?;
        }

        writeln!(sql_file)?;
        writeln!(sql_file, ") as tmp (version_id, crate_size)")?;
        writeln!(sql_file, "where id = tmp.version_id;")?;
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
