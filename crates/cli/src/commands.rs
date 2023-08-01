#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum Commands {
    MatrixBot(MatrixBot),
}

#[derive(clap::Args, Debug)]
pub(crate) struct MatrixBot {
    #[clap(subcommand)]
    pub command: MatrixBotCommands,
}

#[derive(clap::Subcommand, Debug)]
pub(crate) enum MatrixBotCommands {
    Run(MatrixBotRun),
}

#[derive(clap::Args, Debug)]
pub(crate) struct MatrixBotRun {}
