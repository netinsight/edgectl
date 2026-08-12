use std::io::IsTerminal;
use std::process::{self, Command as ProcessCommand};

use clap::{Arg, ArgMatches, Command};
use serde::Deserialize;
use tabled::{builder::Builder, settings::Style};

#[derive(Debug, Deserialize)]
struct NimbraEdge {
    #[serde(default)]
    status: Option<NimbraEdgeStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NimbraEdgeStatus {
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    last_reconciled_at: Option<String>,
    #[serde(default)]
    version: Option<Version>,
    #[serde(default)]
    components: Vec<Component>,
    #[serde(default)]
    conditions: Vec<Condition>,
}

#[derive(Debug, Deserialize)]
struct Version {
    #[serde(default)]
    desired: Option<String>,
    #[serde(default)]
    installed: Option<String>,
    #[serde(default)]
    previous: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Component {
    name: String,
    status: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Condition {
    #[serde(rename = "type")]
    condition_type: String,
    status: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

pub(crate) fn subcommand() -> Command {
    Command::new("operator")
        .about("Inspect the Nimbra Edge Kubernetes operator")
        .subcommand_required(true)
        .subcommand(
            Command::new("status")
                .about("Show the status of the NimbraEdge resource")
                .arg(
                    Arg::new("color")
                        .long("color")
                        .value_parser(["auto", "always", "never"])
                        .num_args(0..=1)
                        .default_value("auto")
                        .default_missing_value("always")
                        .help("When to colorize the status output"),
                ),
        )
}

pub(crate) fn run(args: &ArgMatches) {
    match args.subcommand() {
        Some(("status", sub_args)) => status(sub_args),
        _ => unreachable!("subcommand_required prevents `None` or other options"),
    }
}

fn status(args: &ArgMatches) {
    let color = match args
        .get_one::<String>("color")
        .expect("color has a default")
        .as_str()
    {
        "always" => true,
        "never" => false,
        _ => std::io::stdout().is_terminal(),
    };

    let json = get_edge_resource();
    let edge: NimbraEdge = serde_json::from_str(&json).unwrap_or_else(|e| {
        eprintln!("Failed to parse the NimbraEdge resource: {}", e);
        process::exit(1);
    });

    let Some(status) = edge.status else {
        eprintln!("The NimbraEdge resource has no status yet");
        process::exit(1);
    };

    println!(
        "Phase:            {}",
        colorize(status.phase.as_deref().unwrap_or("Unknown"), color)
    );
    if let Some(version) = &status.version {
        println!(
            "Version:          {}",
            version.installed.as_deref().unwrap_or("unknown")
        );
        if version.desired != version.installed {
            println!(
                "Desired version:  {}",
                version.desired.as_deref().unwrap_or("unknown")
            );
        }
        if let Some(previous) = &version.previous {
            println!("Previous version: {}", previous);
        }
    }
    if let Some(last_reconciled) = &status.last_reconciled_at {
        println!("Last reconciled:  {}", last_reconciled);
    }

    for condition in &status.conditions {
        let label = format!("{}:", condition.condition_type);
        match &condition.reason {
            Some(reason) => println!(
                "{:<18}{} ({})",
                label,
                condition.status,
                colorize(reason, color)
            ),
            None => println!("{:<18}{}", label, condition.status),
        }
        if let Some(message) = &condition.message {
            println!("{:<18}{}", "", message);
        }
    }

    if !status.components.is_empty() {
        println!("\nComponents:");
        // The message shares a column with the status, padded by hand, so that the color escapes
        // always end up in the last column where they cannot upset the column widths
        let width = std::iter::once("Status".chars().count())
            .chain(status.components.iter().map(|c| c.status.chars().count()))
            .max()
            .unwrap_or_default();
        let cell = |status: &str, colored: String, message: Option<&String>| match message {
            Some(message) => format!(
                "{}{}  {}",
                colored,
                " ".repeat(width - status.chars().count()),
                message
            ),
            None => colored,
        };

        let mut builder = Builder::default();
        let any_message = status.components.iter().any(|c| c.message.is_some());
        let message_header = "Message".to_owned();
        builder.push_record([
            "Name".to_owned(),
            cell(
                "Status",
                "Status".to_owned(),
                any_message.then_some(&message_header),
            ),
        ]);
        for component in &status.components {
            builder.push_record([
                component.name.clone(),
                cell(
                    &component.status,
                    colorize(&component.status, color),
                    component.message.as_ref(),
                ),
            ]);
        }
        let mut table = builder.build();
        table.with(Style::empty());
        println!("{}", table);
    }
}

fn get_edge_resource() -> String {
    let output = ProcessCommand::new("kubectl")
        .args([
            "get",
            "-n",
            "edge",
            "nimbraedges.nimbra.io",
            "edge",
            "-o",
            "json",
        ])
        .output()
        .unwrap_or_else(|e| {
            eprintln!("Failed to run kubectl: {}", e);
            process::exit(1);
        });

    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        process::exit(output.status.code().unwrap_or(1));
    }

    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn colorize(status: &str, color: bool) -> String {
    if !color {
        return status.to_owned();
    }
    // Literals from the operator: phase and component status (edge-operator/src/crd.ts) and
    // the reasons of the single `Ready` condition (edge-operator/src/reconcile.ts, pipeline.ts)
    match status {
        "Ready" | "AllHealthy" => format!("\x1b[32m{}\x1b[0m", status),
        "Error" | "ValidationFailed" | "PermanentError" | "BillingFailed" => {
            format!("\x1b[31m{}\x1b[0m", status)
        }
        _ => format!("\x1b[33m{}\x1b[0m", status),
    }
}
