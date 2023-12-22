#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

mod btc;
mod config;
mod db;
mod util;
mod wallet;

use anyhow::Result;
use chrono::Local;
use env_logger::fmt::Color as LColor;
use log::debug;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<()> {
    init_logger();
    debug!("start...");

    config::init();
    db::init(&config::db_path()).await;

    debug!("exit...");
    Ok(())
}

fn init_logger() {
    env_logger::builder()
        .format(|buf, record| {
            let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
            let mut level_style = buf.style();
            match record.level() {
                log::Level::Warn | log::Level::Error => {
                    level_style.set_color(LColor::Red).set_bold(true)
                }
                _ => level_style.set_color(LColor::Blue).set_bold(true),
            };

            writeln!(
                buf,
                "[{} {} {} {}] {}",
                ts,
                level_style.value(record.level()),
                record
                    .file()
                    .unwrap_or("None")
                    .split('/')
                    .last()
                    .unwrap_or("None"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();
}
