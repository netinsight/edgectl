use std::{env, process};

use anyhow::{anyhow, Context};
use clap::{Arg, ArgMatches, Command};
use tabled::{builder::Builder, settings::Style};

use crate::config::Config;
use crate::edge::{new_client, EdgeClient, Group, NewGroup};

pub(crate) fn subcommand() -> clap::Command {
    Command::new("group")
        .about("Manage groups")
        .subcommand_required(true)
        .subcommand(Command::new("list"))
        .subcommand(
            Command::new("show").arg(
                Arg::new("name")
                    .required(true)
                    .help("The group name to show details for"),
            ),
        )
        .subcommand(
            Command::new("create").arg(Arg::new("name").required(true).help("The group name")),
        )
        .subcommand(
            Command::new("delete").arg(Arg::new("name").required(true).help("The group name")),
        )
        .subcommand(
            Command::new("core-secret").arg(Arg::new("name").required(true).help("The group name")),
        )
}

pub(crate) fn run(subcmd: &ArgMatches) {
    match subcmd.subcommand() {
        Some(("list", _)) | None => {
            let client = new_client();
            list(client)
        }
        Some(("show", args)) => {
            let client = new_client();
            let name = args
                .get_one::<String>("name")
                .map(|s| s.as_str())
                .expect("Group name is mandatory");
            show(client, name)
        }
        Some(("core-secret", args)) => {
            let client = new_client();
            let name = args
                .get_one::<String>("name")
                .map(|s| s.as_str())
                .expect("Group name is mandatory");
            core_secret(client, name)
        }
        Some(("create", args)) => {
            let client = new_client();
            let name = args
                .get_one::<String>("name")
                .map(|s| s.as_str())
                .expect("Group name is mandatory");
            create(client, name)
        }
        Some(("delete", args)) => {
            let client = new_client();
            let name = args
                .get_one::<String>("name")
                .map(|s| s.as_str())
                .expect("Group name is mandatory");
            delete(client, name)
        }
        _ => unreachable!("subcommand_required prevents `None` or other options"),
    }
}

/// Resolves the id of the group to act in: either the named group, or the group of the user in the
/// current context.
pub(crate) fn resolve(client: &EdgeClient, name: Option<&String>) -> anyhow::Result<String> {
    if let Some(name) = name {
        let groups = client
            .find_groups(name)
            .context("Failed to look up group")?;
        return groups
            .into_iter()
            .find(|g| &g.name == name)
            .map(|g| g.id)
            .ok_or_else(|| anyhow!("Group '{}' not found", name));
    }

    let username = env::var("EDGE_USER")
        .ok()
        .or_else(|| {
            Config::load()
                .get_current_context()
                .map(|c| c.username.clone())
        })
        .unwrap_or_else(|| "admin".to_owned());

    client
        .get_user_group(&username)
        .context("Failed to look up the group of the current user")?
        .ok_or_else(|| {
            anyhow!(
                "Failed to determine the group of user '{}', pass --group",
                username
            )
        })
}

fn list(client: EdgeClient) {
    let groups = client.list_groups().expect("Failed to fetch group list");

    let mut builder = Builder::default();

    builder.push_record(["ID", "Name"]);
    for group in groups {
        builder.push_record([group.id, group.name])
    }

    let mut table = builder.build();
    table.with(Style::empty());
    println!("{}", table)
}

fn show(client: EdgeClient, name: &str) {
    let groups = client.find_groups(name).expect("Failed to find groups");
    if groups.is_empty() {
        println!("No such group: {}", name);
        process::exit(1);
    }
    for group in groups {
        println!("Name:                 {}", group.name);
        println!("ID:                   {}", group.id);
        println!(
            "Appliance secret:     {}",
            group.appliance_secret.unwrap_or("".to_owned())
        );
    }
}

pub(crate) fn core_secret(client: EdgeClient, name: &str) {
    let groups = match client.find_groups(name) {
        Ok(groups) => groups,
        Err(e) => {
            println!("Failed to list groups: {}", e);
            process::exit(1);
        }
    };
    let groups: Vec<&Group> = groups.iter().filter(|g| g.name == name).collect();
    if groups.is_empty() {
        println!("Group not found: {}", name);
        process::exit(1);
    }
    for group in groups {
        let secret = client
            .get_group_core_secret(&group.id)
            .expect("Failed to get group secret");
        println!("{}", secret)
    }
}

fn create(client: EdgeClient, name: &str) {
    match client.create_group(NewGroup {
        name: name.to_owned(),
        appliance_secret: uuid::Uuid::new_v4().to_string(),
    }) {
        Err(e) => {
            println!("Failed to create group: {}", e);
            process::exit(1);
        }
        Ok(g) => {
            println!(
                "Created group {} with appliance secret {}",
                g.name,
                g.appliance_secret.unwrap_or("".to_owned())
            )
        }
    }
}

fn delete(client: EdgeClient, name: &str) {
    let groups = match client.find_groups(name) {
        Ok(groups) => groups,
        Err(e) => {
            println!("Failed to list groups for deletion: {}", e);
            process::exit(1);
        }
    };
    let groups: Vec<&Group> = groups.iter().filter(|g| g.name == name).collect();
    if groups.is_empty() {
        println!("Group not found: {}", name);
        process::exit(1);
    }
    for group in groups {
        if let Err(e) = client.delete_group(&group.id) {
            println!("Failed to delete group {}: {}", group.name, e);
            process::exit(1);
        }
        println!("Deleted groupd {}", group.name)
    }
}
