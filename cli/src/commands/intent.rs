use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum IntentCommands {
    Get,
    Approve,
    Disapprove,
    Execute,
    Delete,
}