use crate::dialoguer;
use crates_io::db;
use crates_io::models::{Crate, Version};
use crates_io::schema::versions;
use crates_io::worker::jobs::{GenerateOgImage, SyncToGitIndex, SyncToSparseIndex};
use crates_io_worker::BackgroundJob;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};

#[derive(clap::Parser, Debug)]
#[command(
    name = "yank-version",
    about = "Yank a crate from the database and index."
)]
pub struct Opts {
    /// Name of the crate
    crate_name: String,
    /// Version number that should be deleted
    version: String,
    /// Don't ask for confirmation: yes, we are sure. Best for scripting.
    #[arg(short, long)]
    yes: bool,
}

pub async fn run(opts: Opts) -> anyhow::Result<()> {
    let mut conn = db::oneoff_connection().await?;

    conn.transaction(async |conn| yank(opts, conn).await)
        .await?;

    Ok(())
}

async fn yank(opts: Opts, conn: &mut AsyncPgConnection) -> anyhow::Result<()> {
    let Opts {
        crate_name,
        version,
        yes,
    } = opts;
    let krate: Crate = Crate::by_name(&crate_name).first(conn).await?;

    let v: Version = Version::belonging_to(&krate)
        .filter(versions::num.eq(&version))
        .select(Version::as_select())
        .first(conn)
        .await?;

    if v.yanked {
        println!("Version {version} of crate {crate_name} is already yanked");
        return Ok(());
    }

    if !yes {
        let prompt = format!(
            "Are you sure you want to yank {crate_name}#{version} ({})?",
            v.id
        );
        if !dialoguer::confirm(&prompt).await? {
            return Ok(());
        }
    }

    println!("yanking version {} ({})", v.num, v.id);
    diesel::update(&v)
        .set(versions::yanked.eq(true))
        .execute(conn)
        .await?;

    // The default version is recomputed automatically by the database trigger on
    // the `versions.yanked` update above; we only need to regenerate the
    // OpenGraph image and sync the index.
    let git_index_job = SyncToGitIndex::new(&krate.name);
    let sparse_index_job = SyncToSparseIndex::new(&krate.name);
    let og_image_job = GenerateOgImage::new(krate.name.clone());

    tokio::try_join!(
        git_index_job.enqueue(&*conn),
        sparse_index_job.enqueue(&*conn),
        og_image_job.enqueue(&*conn),
    )?;

    Ok(())
}
