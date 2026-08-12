use crate::alarm;
use crate::appliance;
use crate::context;
use crate::group;
use crate::group_list;
use crate::health;
use crate::input;
use crate::login;
use crate::node;
use crate::operator;
use crate::output;
use crate::output_list;
use crate::region;
use crate::settings;
use crate::token;
use crate::tunnels;

use clap::Command;

pub(crate) fn build() -> Command {
    Command::new("edgectl")
        .about("Nimbra Edge CLI")
        .version(option_env!("VERSION").unwrap_or("unknown"))
        .subcommand_required(true)
        .subcommand(alarm::subcommand())
        .subcommand(input::subcommand())
        .subcommand(output::subcommand())
        .subcommand(output_list::subcommand())
        .subcommand(appliance::subcommand())
        .subcommand(group::subcommand())
        .subcommand(group_list::subcommand())
        .subcommand(region::subcommand())
        .subcommand(node::subcommand())
        .subcommand(operator::subcommand())
        .subcommand(tunnels::subcommand())
        .subcommand(settings::subcommand())
        .subcommand(health::subcommand())
        .subcommand(login::subcommand())
        .subcommand(token::subcommand())
        .subcommand(context::subcommand())
        .subcommand(Command::new("build-info").about("Show build information for installation"))
        .subcommand(Command::new("tui").about("Open interactive TUI"))
}
