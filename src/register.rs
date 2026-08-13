use std::io::{self, IsTerminal, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{env, process};

use anyhow::{anyhow, Context};
use clap::{Arg, ArgAction, ArgMatches, Command};
use serde::{Deserialize, Serialize};

use crate::edge::{new_client, EdgeClient};
use crate::group;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

pub(crate) fn subcommand() -> Command {
    Command::new("register")
        .about("Register an edge-os appliance with this Nimbra Edge installation")
        .arg(
            Arg::new("url")
                .required(true)
                .help("The URL of the appliance, e.g. https://10.0.0.1"),
        )
        .arg(
            Arg::new("name")
                .long("name")
                .help("The name to register the appliance as (defaults to its hostname)"),
        )
        .arg(
            Arg::new("region")
                .long("region")
                .help("The region to register the appliance in (defaults to the default region)"),
        )
        .arg(Arg::new("group").long("group").help(
            "The group to register the appliance in (defaults to the group of the current user)",
        ))
        .arg(
            Arg::new("username")
                .long("username")
                .short('u')
                .help("The appliance username [default: admin]"),
        )
        .arg(
            Arg::new("insecure")
                .long("insecure")
                .short('k')
                .action(ArgAction::SetTrue)
                .help("Do not verify the TLS certificate of the appliance"),
        )
        .arg(
            Arg::new("allow-self-signed")
                .long("allow-self-signed")
                .action(ArgAction::SetTrue)
                .help("Let the appliance accept a self-signed certificate from Nimbra Edge"),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .action(ArgAction::SetTrue)
                .help("Register even if the appliance is already registered"),
        )
}

pub(crate) fn run(args: &ArgMatches) {
    if let Err(e) = register(args) {
        eprintln!("{:#}", e);
        process::exit(1);
    }
}

fn register(args: &ArgMatches) -> anyhow::Result<()> {
    let insecure = args.get_flag("insecure");
    let url = normalize_url(args.get_one::<String>("url").expect("URL is mandatory"));

    let appliance = ApplianceClient::new(&url, insecure)?;
    let version = appliance.version()?;
    eprintln!("Found edge-os {} at {}", version.os_version, url);

    let client = new_client();
    let edge_url = client.url.trim_end_matches('/').to_owned();
    let group = group::resolve(&client, args.get_one::<String>("group").map(|s| s.as_str()))?;
    let region = args.get_one::<String>("region");
    if let Some(region) = region {
        ensure_region_exists(&client, region)?;
    }

    let (username, password) = credentials(args)?;
    appliance.login(&username, &password)?;

    let status = appliance.edge_connect_status()?;
    if status.status != ConnectStatus::NotConfigured && !args.get_flag("force") {
        return Err(anyhow!(
            "Appliance is already registered{}, pass --force to register it anyway",
            status
                .edge_url
                .map(|u| format!(" with {}", u))
                .unwrap_or_default()
        ));
    }

    let name = match args.get_one::<String>("name") {
        Some(name) => name.to_owned(),
        None => appliance.hostname()?,
    };

    let secret = client
        .create_appliance_token(&group, "edge")
        .context("Failed to create an appliance token")?;

    appliance.set_management_config(&ManagementConfig {
        edge_url: &edge_url,
        device_name: &name,
        secret: &secret,
        edge_role: "edge",
        region: region.map(String::as_str),
        allow_self_signed_certs: args.get_flag("allow-self-signed"),
    })?;

    wait_for_connection(&appliance, &name, &edge_url)
}

/// reqwest only renders the outermost error, which for connection failures says nothing about
/// what actually went wrong, so spell out the whole chain of causes.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();

    while let Some(error) = cause {
        message.push_str(&format!(": {}", error));
        cause = error.source();
    }

    message
}

fn normalize_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    if url.contains("://") {
        url.to_owned()
    } else {
        format!("https://{}", url)
    }
}

fn ensure_region_exists(client: &EdgeClient, name: &str) -> anyhow::Result<()> {
    let regions = client
        .find_region(name)
        .context("Failed to look up region")?;

    if regions.iter().any(|r| r.name == name) {
        Ok(())
    } else {
        Err(anyhow!("Region '{}' not found", name))
    }
}

fn credentials(args: &ArgMatches) -> anyhow::Result<(String, String)> {
    let username = args
        .get_one::<String>("username")
        .cloned()
        .or_else(|| env::var("EDGE_APPLIANCE_USER").ok());
    let password = env::var("EDGE_APPLIANCE_PASSWORD").ok();

    if !io::stdin().is_terminal() {
        return Ok((
            username.unwrap_or_else(|| "admin".to_owned()),
            password
                .context("EDGE_APPLIANCE_PASSWORD is required when not running interactively")?,
        ));
    }

    let username = match username {
        Some(username) => username,
        None => prompt("Username [admin]: ")?,
    };
    let password = match password {
        Some(password) => password,
        None => prompt_password()?,
    };

    Ok((
        if username.is_empty() {
            "admin".to_owned()
        } else {
            username
        },
        password,
    ))
}

fn prompt(prompt: &str) -> anyhow::Result<String> {
    eprint!("{}", prompt);
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_owned())
}

fn prompt_password() -> anyhow::Result<String> {
    eprint!("Password: ");
    io::stderr().flush()?;

    match rpassword::read_password() {
        Ok(password) => Ok(password),
        Err(_) => prompt(""),
    }
}

fn wait_for_connection(
    appliance: &ApplianceClient,
    name: &str,
    edge_url: &str,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let mut last = None;

    while Instant::now() < deadline {
        // Sleep before the first poll: restarting edge-connect wipes its runtime directory, and
        // until that happens a re-registered appliance still reports the connection it had before.
        sleep(POLL_INTERVAL);

        // Restarting edge-connect also makes requests fail for a moment, which is expected and not
        // a reason to stop waiting.
        if let Ok(status) = appliance.edge_connect_status() {
            if status.status == ConnectStatus::Connected {
                println!("Registered appliance '{}' with {}", name, edge_url);
                return Ok(());
            }
            last = Some(status);
        }
    }

    let (status, error) = match last {
        Some(status) => (status.status.to_string(), status.error),
        None => ("unknown".to_owned(), None),
    };

    Err(anyhow!(
        "The appliance did not connect to {} within {}s (status: {}{})",
        edge_url,
        CONNECT_TIMEOUT.as_secs(),
        status,
        error.map(|e| format!(", {}", e)).unwrap_or_default()
    ))
}

struct ApplianceClient {
    client: reqwest::blocking::Client,
    url: String,
    insecure: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplianceVersion {
    os_version: String,
}

#[derive(Debug, PartialEq)]
enum ConnectStatus {
    NotConfigured,
    Connected,
    Connecting,
    Disconnected,
    Error,
    Unknown(String),
}

impl std::fmt::Display for ConnectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "not configured"),
            Self::Connected => write!(f, "connected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::Error => write!(f, "error"),
            Self::Unknown(status) => write!(f, "{}", status),
        }
    }
}

impl<'de> Deserialize<'de> for ConnectStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "notConfigured" => Self::NotConfigured,
            "connected" => Self::Connected,
            "connecting" => Self::Connecting,
            "disconnected" => Self::Disconnected,
            "error" => Self::Error,
            _ => Self::Unknown(value),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EdgeConnectStatus {
    status: ConnectStatus,
    edge_url: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagementConfig<'a> {
    edge_url: &'a str,
    device_name: &'a str,
    secret: &'a str,
    edge_role: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<&'a str>,
    allow_self_signed_certs: bool,
}

impl ApplianceClient {
    fn new(url: &str, insecure: bool) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .cookie_store(true)
            .danger_accept_invalid_certs(insecure)
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            url: url.to_owned(),
            insecure,
        })
    }

    fn version(&self) -> anyhow::Result<ApplianceVersion> {
        let res = self
            .client
            .get(format!("{}/api/version", self.url))
            .send()
            .map_err(|e| {
                let cause = error_chain(&e);
                if !self.insecure && cause.contains("certificate") {
                    anyhow!(
                        "Failed to connect to {}: {}\n\
                         Pass --insecure if the appliance uses a self-signed certificate",
                        self.url,
                        cause
                    )
                } else {
                    anyhow!("Failed to connect to {}: {}", self.url, cause)
                }
            })?;

        if !res.status().is_success() {
            return Err(anyhow!(
                "{} does not look like an edge-os appliance",
                self.url
            ));
        }

        res.json()
            .with_context(|| format!("{} does not look like an edge-os appliance", self.url))
    }

    fn login(&self, username: &str, password: &str) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct LoginRequest<'a> {
            username: &'a str,
            password: &'a str,
        }

        let res = self
            .client
            .post(format!("{}/api/auth/login", self.url))
            .json(&LoginRequest { username, password })
            .send()
            .context("Failed to log in to the appliance")?;

        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow!("Invalid username or password for {}", self.url));
        }

        self.error_if_not_success(res, "Failed to log in to the appliance")
            .map(|_| ())
    }

    fn hostname(&self) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        struct SystemInfo {
            hostname: String,
        }

        let res = self
            .client
            .get(format!("{}/api/system-info", self.url))
            .send()
            .context("Failed to fetch appliance system info")?;

        Ok(self
            .error_if_not_success(res, "Failed to fetch appliance system info")?
            .json::<SystemInfo>()
            .context("Failed to parse appliance system info")?
            .hostname)
    }

    fn edge_connect_status(&self) -> anyhow::Result<EdgeConnectStatus> {
        let res = self
            .client
            .get(format!("{}/api/edge-connect/status", self.url))
            .send()
            .context("Failed to fetch appliance registration status")?;

        self.error_if_not_success(res, "Failed to fetch appliance registration status")?
            .json()
            .context("Failed to parse appliance registration status")
    }

    fn set_management_config(&self, config: &ManagementConfig) -> anyhow::Result<()> {
        let res = self
            .client
            .put(format!("{}/api/management", self.url))
            .json(config)
            .send()
            .context("Failed to configure the appliance")?;

        self.error_if_not_success(res, "Failed to configure the appliance")
            .map(|_| ())
    }

    fn error_if_not_success(
        &self,
        res: reqwest::blocking::Response,
        context: &str,
    ) -> anyhow::Result<reqwest::blocking::Response> {
        #[derive(Deserialize)]
        struct ErrorResponse {
            message: String,
        }

        if res.status().is_success() {
            return Ok(res);
        }

        let status = res.status();
        let message = res
            .json::<ErrorResponse>()
            .map(|e| e.message)
            .unwrap_or_else(|_| status.to_string());

        Err(anyhow!("{}: {}", context, message))
    }
}
