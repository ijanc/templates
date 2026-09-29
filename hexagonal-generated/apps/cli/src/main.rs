// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: 2026 your name <author@example.com>

//! Driving adapter on the command line: the same use cases as the HTTP
//! API, straight over the database named by `DATABASE_URL`.

use std::sync::Arc;

use anyhow::Context;
use application::{
    CreateItem, ItemService, ItemUseCases, LIMIT_DEFAULT, ListQuery, UpdateItem,
};
use clap::{Parser, Subcommand};
use store_sqlx::SqlxStore;
use uuid::Uuid;

/// Program name, used in diagnostics.
pub const PROG: &str = "hexagonal-generated-cli";

#[derive(Debug, Parser)]
#[command(name = PROG, version, about = "An example generated using the hexagonal template")]
struct Opts {
    /// Database connection string; `.env` is read first
    #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
    database_url: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Apply pending migrations
    Migrate,
    /// Manage items
    #[command(subcommand)]
    Items(Items),
}

#[derive(Debug, Subcommand)]
enum Items {
    /// List items, oldest first, one per line: id and name
    List {
        #[arg(long, default_value_t = LIMIT_DEFAULT)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show one item, one field per line
    Get { id: Uuid },
    /// Create an item and print its id
    Create {
        name: String,
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Replace the name and description of an item
    Update {
        id: Uuid,
        name: String,
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Delete an item
    Delete { id: Uuid },
}

fn main() {
    dotenvy::dotenv().ok();
    let opts = Opts::parse();
    if let Err(e) = run(opts) {
        eprintln!("{PROG}: {e:#}");
        std::process::exit(1);
    }
}

#[tokio::main]
async fn run(opts: Opts) -> anyhow::Result<()> {
    let store = SqlxStore::connect(&opts.database_url)
        .await
        .context("database: connect")?;
    match opts.command {
        Command::Migrate => {
            store.migrate().await?;
            println!("migrations applied");
        }
        Command::Items(cmd) => {
            let items = ItemService::new(Arc::new(store));
            items_command(&items, cmd).await?;
        }
    }
    Ok(())
}

/// The item commands, against the port only.
async fn items_command(
    items: &dyn ItemUseCases,
    cmd: Items,
) -> anyhow::Result<()> {
    match cmd {
        Items::List { limit, offset } => {
            let page = items.list(ListQuery { limit, offset }).await?;
            for item in &page.items {
                println!("{}\t{}", item.id, item.name);
            }
        }
        Items::Get { id } => {
            let item = items.get(id).await?;
            println!("id\t{}", item.id);
            println!("name\t{}", item.name);
            println!("description\t{}", item.description.unwrap_or_default());
            println!("created_at\t{}", item.created_at.to_rfc3339());
            println!("updated_at\t{}", item.updated_at.to_rfc3339());
        }
        Items::Create { name, description } => {
            let input = CreateItem { name, description };
            let item = items.create(input).await?;
            println!("{}", item.id);
        }
        Items::Update {
            id,
            name,
            description,
        } => {
            let input = UpdateItem { name, description };
            items.update(id, input).await?;
        }
        Items::Delete { id } => {
            items.delete(id).await?;
        }
    }
    Ok(())
}
