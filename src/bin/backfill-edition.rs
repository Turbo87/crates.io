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
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread;
use tracing::{info, warn};

#[derive(Debug, clap::Parser)]
struct Args {
    /// The root directory of an `get-all-crates` run.
    crates_path: PathBuf,

    /// The path to the CSV file containing the processed versions.
    #[clap(long, default_value = "edition.csv")]
    csv_path: PathBuf,

    /// The path to the SQL file to generate.
    #[clap(long, default_value = "edition.sql")]
    sql_path: PathBuf,

    /// The number of records to write in a single SQL query.
    #[clap(long, default_value = "1000")]
    chunk_size: usize,
}

/// Processes all versions without edition information in the database and
/// appends the results to a CSV file. The CSV file is then read and used to
/// generate an SQL file that updates the versions table with the edition
/// information.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    crates_io::util::tracing::init();

    let args: Args = clap::Parser::parse();

    let mut conn = db::oneoff_connection().await?;

    info!("Fetching versions without edition information from the database…");
    let versions: Vec<(i32, String, String)> = versions::table
        .inner_join(crates::table)
        .select((versions::id, crates::name, versions::num))
        .filter(versions::edition.is_null())
        .get_results(&mut conn)
        .await?;

    info!("Reading processed versions from CSV file…");
    let processed_versions = read_csv(&args.csv_path)?;

    info!("Filtering out already processed versions…");
    let versions: Vec<_> = versions
        .into_iter()
        .filter(|(version_id, _, _)| !processed_versions.contains(version_id))
        .collect();

    let (tx, rx) = channel::<(i32, Option<&'static str>)>();

    info!("Starting CSV writer thread…");
    let csv_path = args.csv_path.clone();
    let handle = thread::spawn(move || {
        let file = File::options()
            .create(true)
            .append(true)
            .open(csv_path)
            .unwrap();

        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(file);

        for (version_id, edition) in rx {
            let version_id = version_id.to_string();
            if let Some(edition) = edition {
                let edition = format!("'{}'", edition);
                writer.write_record([&version_id, &edition]).unwrap();
            } else {
                writer.write_record([&version_id, "NULL"]).unwrap();
            }
        }
    });

    let pb = ProgressBar::new(versions.len() as u64);
    let template = "{bar:60} ({pos}/{len}, ETA {eta}) {wide_msg}";
    pb.set_style(ProgressStyle::with_template(template).unwrap());

    info!("Processing versions…");
    versions
        .par_iter()
        .progress_with(pb.clone())
        .for_each(|(version_id, name, version)| {
            let path = args
                .crates_path
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

            let edition = tarball
                .manifest
                .package
                .and_then(|pkg| pkg.edition)
                .and_then(|ed| ed.as_local())
                .map(|ed| ed.as_str());

            tx.send((*version_id, edition)).unwrap();
        });

    drop(tx);
    handle.join().unwrap();

    info!("Generating SQL file…");
    let csv_file = File::open(&args.csv_path)?;
    let mut rdr = csv::Reader::from_reader(csv_file);
    let iter = rdr
        .records()
        .map(|record| record.unwrap())
        .filter(|record| &record[1] != "NULL")
        .chunks(args.chunk_size);

    let mut sql_file = File::create(&args.sql_path)?;

    for chunk in &iter {
        writeln!(sql_file, "update versions")?;
        writeln!(sql_file, "set edition = tmp.edition")?;
        writeln!(sql_file, "from (values")?;

        for (i, record) in chunk.enumerate() {
            if i > 0 {
                writeln!(sql_file, ",")?;
            }
            write!(sql_file, "    ({}, {})", &record[0], &record[1])?;
        }

        writeln!(sql_file)?;
        writeln!(sql_file, ") as tmp (version_id, edition)")?;
        writeln!(sql_file, "where id = tmp.version_id;")?;
        writeln!(sql_file)?;
    }

    Ok(())
}

fn read_csv(path: &Path) -> anyhow::Result<HashSet<i32>> {
    let file = match File::open(path) {
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
