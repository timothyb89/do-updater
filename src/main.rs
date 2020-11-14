#[macro_use] extern crate err_derive;
extern crate env_logger;
extern crate digitalocean;
#[macro_use] extern crate log;

use std::collections::HashSet;
use std::error::Error;
use std::thread;
use std::time::Duration;

use clokwerk::{Scheduler, TimeUnits};
use digitalocean::prelude::*;
use digitalocean::DigitalOcean;
use digitalocean::api::{Domain, DomainRecord, Droplet};
use env_logger::Env;
use serde::Deserialize;

const CARGO_PKG_NAME: &'static str = env!("CARGO_PKG_NAME");

#[derive(Debug, Error)]
enum DOError {
    #[error(display = "invalid configuration: {:#?}", _0)]
    InvalidConfig(String),

    #[error(display = "failed to create a DigitalOcean client: {:#?}", _0)]
    LoginFail(String),

    #[error(display = "failed to list droplets: {:#?}", _0)]
    ListDropletsFail(String),

    #[error(display = "failed to fetch domain records: {:#?}", _0)]
    ListRecordsFail(String),

    #[error(display = "failed to create record: {:?}", _0)]
    CreateRecordFail(String),

    #[error(display = "failed to delete record: {:?}", _0)]
    DeleteRecordFail(String)
}

type Result<T> = std::result::Result<T, DOError>;

#[derive(Deserialize, Debug, Clone)]
struct Config {
    api_key: String,
    domain_name: String,
    droplet_tag: String,
    record_name: String,
    record_kind: String,
    record_ttl: u32
}

fn list_droplet_ips(client: &DigitalOcean, config: &Config) -> Result<HashSet<String>> {
    let droplets = match Droplet::list_by_tag(&config.droplet_tag).execute(&client) {
        Ok(droplets) => droplets,
        Err(e) => return Err(DOError::ListDropletsFail(e.to_string()))
    };

    let mut ips: HashSet<String> = HashSet::new();

    for droplet in droplets {
        let networks = droplet.networks();
        for network in networks.clone().v4 {
            if network.kind == "public" {
                ips.insert(network.ip_address.to_string());
            }
        }
    }

    Ok(ips)
}

fn list_records(
    client: &DigitalOcean,
    config: &Config
) -> Result<Vec<DomainRecord>> {
    let req = Domain::get(&config.domain_name).records();
    let all_records = match client.execute(req) {
        Ok(records) => records,
        Err(e) => return Err(DOError::ListRecordsFail(e.to_string()))
    };

    Ok(all_records.into_iter()
        .filter(|record| record.name() == &config.record_name)
        .filter(|record| record.kind() == &config.record_kind)
        .collect::<Vec<_>>())
}

fn create_record(client: &DigitalOcean, config: &Config, address: &str) -> Result<()> {
    let req = Domain::get(&config.domain_name).records().create(
        &config.record_kind,
        &config.record_name,
        &String::from(address)
    );

    info!("creating record for address: {}", &address);
    match client.execute(req) {
        Ok(_) => Ok(()),
        Err(e) => Err(DOError::CreateRecordFail(e.to_string()))
    }
}

fn delete_record(client: &DigitalOcean, config: &Config, id: &usize) -> Result<()> {
    let req = Domain::get(&config.domain_name).records().delete(*id);

    info!("deleting record id {:?}", id);
    match client.execute(req) {
        Ok(_) => Ok(()),
        Err(e) => Err(DOError::DeleteRecordFail(e.to_string()))
    }
}

fn sync(config: &Config) -> Result<()> {
    warn!(
        "syncing domain '{}' to record '{}' (tag: '{}', kind: '{}', ttl: {})",
        config.domain_name, config.record_name, config.droplet_tag,
        config.record_kind, config.record_ttl
    );

    let client = match DigitalOcean::new(config.api_key.clone()) {
        Ok(client) => client,
        Err(e) => return Err(DOError::LoginFail(e.to_string()))
    };

    let desired_ips = list_droplet_ips(&client, &config)?;
    debug!("desired ips: {:?}", &desired_ips);

    let records = list_records(&client, &config)?;

    let current_ips: HashSet<String> = records.iter()
        .map(|record| record.data().clone())
        .collect();

    let to_create = desired_ips.difference(&current_ips);
    let to_delete = current_ips.difference(&desired_ips);
    let up_to_date = desired_ips.intersection(&current_ips);

    info!("create:     {:?}", to_create);
    info!("delete:     {:?}", to_delete);
    info!("up-to-date: {:?}", up_to_date);

    // TODO: would be nice to update TTL for existing entries if changed

    for address in to_create {
        create_record(&client, &config, &address)?;
    }

    for address in to_delete {
        match records.iter().find(|r| r.data() == address) {
            Some(record) => delete_record(&client, &config, record.id())?,
            None => warn!("record not found for address: {}", &address)
        };
    }

    info!("sync complete");

    Ok(())
}

fn run() -> Result<()> {
    let config = match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(e) => return Err(DOError::InvalidConfig(e.to_string()))
    };

    info!("running initial sync...");
    sync(&config)?;

    info!("starting scheduler");
    let mut scheduler = Scheduler::new();
    scheduler.every(1.hours()).run(move || {
        if let Err(e) = sync(&config) {
            error!("sync failed due to error: {:?}", e);
        }
    });

    loop {
        scheduler.run_pending();
        thread::sleep(Duration::from_millis(1000));
    }
}

fn print_error(e: &dyn Error) {
    error!("error: {}", e);
    let mut cause = e.source();
    while let Some(e) = cause {
        error!("caused by: {}", e);
        cause = e.source();
    }
}

fn main() {
    env_logger::init_from_env(Env::default().filter_or(
        "RUST_LOG",
        format!("{}=info", CARGO_PKG_NAME.replace("-", "_"))
    ));

    match run() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };
}
