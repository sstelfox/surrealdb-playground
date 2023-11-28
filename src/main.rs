use std::error::Error;
use std::time::Duration;
use std::io::{BufRead, Write};

use pico_args::Arguments;
use serde::Serialize;
use surrealdb::Surreal;
use surrealdb::engine::local::RocksDb;
use surrealdb::opt::Config;
use surrealdb::opt::auth::Root;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let default_user = Root {
        username: "default",
        password: "SuperSekret!!NoSh4ring."
    };

    let db_config = Config::new()
        .strict()
        .query_timeout(Some(Duration::from_secs(5)))
        .transaction_timeout(Some(Duration::from_secs(10)))
        .user(default_user.clone());

    let mut db_path = std::env::current_dir().expect("valid current dir");
    db_path.push("data");
    std::fs::create_dir_all(&db_path)?;

    let mut db = Surreal::new::<RocksDb>((db_path, db_config)).await?;
    db.signin(default_user).await?;

    let mut cli_args = Arguments::from_env();
    if cli_args.contains("--init") {
        let mut init_path = std::env::current_dir().expect("valid current dir");
        init_path.push("initialization.surql");

        let init_surql = std::fs::read_to_string(&init_path)?;
        let response = db.query(&init_surql).await?;

        if let Err(err) = response.check() {
            println!("failed running initialization: {err:?}");
            std::process::exit(1);
        }
    }

    let mut prompt_log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("data/prompt_history.jsonl")
        .expect("prompt log to be accessible");

    record_history(&mut prompt_log, &HistoryEntry::SessionStart)?;

    #[derive(Serialize)]
    struct Role<'a> {
        name: &'a str,
    }

    let sr = db.signin(surrealdb::opt::auth::Scope {
        namespace: "generative_ontology",
        database: "industry",
        scope: "role",
        params: Role { name: "ontologist" },
    }).await;

    if let Err(err) = sr {
        let mut errors = vec![format!("{err:?}")];
        let mut source = err.source();

        while let Some(err) = source {
            errors.push(format!("{err:?}"));
            source = err.source();
        }

        println!("errors:\n{}", errors.join("\n"));
    }

    let stdin = std::io::stdin();
    let mut input = stdin.lock().lines();

    let mut prompt = Prompt::default();
    let mut prompt_mode = PromptMode::Single;

    prompt.print();

    while let Some(line_attempt) = input.next() {
        let line = line_attempt?;

        match prompt_mode {
            PromptMode::Single => {
                if line == "exit" {
                    break;
                }

                if line.starts_with('{') {
                    prompt_mode = PromptMode::Multiline(line[1..].to_string());
                    prompt.blank();
                    continue;
                }

                let successful = process_prompt(&mut db, &line).await;
                record_history(&mut prompt_log, &HistoryEntry::Prompt { idx: prompt.index(), msg: line, successful })?;

                prompt.print();
            }
            PromptMode::Multiline(mut collected) => {
                collected.push('\n');

                if !line.ends_with('}') {
                    collected.push_str(&line);
                    prompt_mode = PromptMode::Multiline(collected);
                    prompt.blank();
                    continue;
                }

                collected.push_str(&line[..line.len()-1]);
                let successful = process_prompt(&mut db, &collected).await;
                record_history(&mut prompt_log, &HistoryEntry::Prompt { idx: prompt.index(), msg: collected, successful })?;

                prompt_mode = PromptMode::Single;
                prompt.print();
            }
        }
    }

    println!("");

    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum HistoryEntry {
    SessionStart,
    Prompt { idx: usize, msg: String, successful: bool },
}

async fn process_prompt(db: &mut Surreal<surrealdb::engine::local::Db>, prompt: &str) -> bool {
    let response = match db.query(prompt).await {
        Ok(resp) => resp,
        Err(err) => {
            println!("Query Error:\n{err}");
            return false;
        }
    };

    match response.check() {
        Ok(resp) => {
            println!("Ok:\n{resp:?}");
            true
        }
        Err(err) => {
            println!("Statement Error:\n{err}");
            false
        }
    }
}

fn record_history(log_fd: &mut std::fs::File, entry: &HistoryEntry) -> std::io::Result<()> {
    writeln!(log_fd, "{}", serde_json::to_string(entry).expect("serializable"))
}

struct Prompt {
    index: usize,
}

impl Prompt {
    fn blank(&self) {
        print!("    |  ");
        std::io::stdout().lock().flush().expect("flush");
    }

    fn index(&self) -> usize {
        self.index
    }

    fn new() -> Self {
        Prompt { index: 0 }
    }

    fn print(&mut self) {
        self.index += 1;
        print!("{:4}> ", self.index);
        std::io::stdout().lock().flush().expect("flush");
    }
}

impl Default for Prompt {
    fn default() -> Self {
        Self::new()
    }
}

enum PromptMode {
    Single,
    Multiline(String),
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("a console I/O error occurred: {0}")]
    IoError(#[from] std::io::Error),

    #[error("a surreal database error occurred: {0}")]
    SurrealDb(#[from] surrealdb::Error),
}
