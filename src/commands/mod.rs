pub mod anytime;
pub mod area;
pub mod areas;
pub mod delete;
pub mod edit;
pub mod find;
pub mod inbox;
pub mod logbook;
pub mod mark;
pub mod new;
pub mod project;
pub mod projects;
pub mod reorder;
pub mod schedule;
pub mod set_auth;
pub mod someday;
pub mod tags;
pub mod today;
pub mod upcoming;

use crate::app::Cli;
use anyhow::Result;
use clap::{Args, Subcommand};

pub trait Command {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()>;
}

#[derive(Debug, Default, Args)]
pub struct DetailedArgs {
    #[arg(long)]
    pub detailed: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Inbox(inbox::InboxArgs),
    Today(today::TodayArgs),
    Upcoming(upcoming::UpcomingArgs),
    Anytime(anytime::AnytimeArgs),
    Someday(someday::SomedayArgs),
    Logbook(logbook::LogbookArgs),
    Projects(projects::ProjectsArgs),
    Project(project::ProjectArgs),
    Areas(areas::AreasArgs),
    Area(area::AreaArgs),
    Tags(tags::TagsArgs),
    New(new::NewArgs),
    Edit(edit::EditArgs),
    Mark(mark::MarkArgs),
    Schedule(schedule::ScheduleArgs),
    Reorder(reorder::ReorderArgs),
    Delete(delete::DeleteArgs),
    SetAuth(set_auth::SetAuthArgs),
    Find(find::FindArgs),
}

impl Command for Commands {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        match self {
            Commands::Inbox(args) => args.run(cli, out),
            Commands::Today(args) => args.run(cli, out),
            Commands::Upcoming(args) => args.run(cli, out),
            Commands::Anytime(args) => args.run(cli, out),
            Commands::Someday(args) => args.run(cli, out),
            Commands::Logbook(args) => args.run(cli, out),
            Commands::Projects(args) => args.run(cli, out),
            Commands::Project(args) => args.run(cli, out),
            Commands::Areas(args) => args.run(cli, out),
            Commands::Area(args) => args.run(cli, out),
            Commands::Tags(args) => args.run(cli, out),
            Commands::New(args) => args.run(cli, out),
            Commands::Edit(args) => args.run(cli, out),
            Commands::Mark(args) => args.run(cli, out),
            Commands::Schedule(args) => args.run(cli, out),
            Commands::Reorder(args) => args.run(cli, out),
            Commands::Delete(args) => args.run(cli, out),
            Commands::SetAuth(args) => args.run(cli, out),
            Commands::Find(args) => args.run(cli, out),
        }
    }
}
