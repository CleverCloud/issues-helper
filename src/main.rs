extern crate futures;
extern crate git2;
extern crate gitlab;
extern crate hyper;
extern crate hyper_tls;
extern crate itertools;
#[macro_use]
extern crate nom;
extern crate open;
extern crate rprompt;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate tokio_core;
extern crate toml;
extern crate url;
extern crate xdg;

mod config;
mod gitlab_api;

use config::*;
use gitlab_api::*;
use std::error::Error;
use structopt::StructOpt;

fn do_work(cmd: &Cmd) -> Result<String, Box<Error>> {
    match cmd {
        &Cmd::OpenIssue {
            open_browser,
            ref labels,
            ref assignee,
            ref title,
            ref text,
        } => {
            let config = read_config()?;
            let project = extract_project(&config)?;
            let res = create_issue(&config, &project, title, text, labels, assignee)?;
            let url = get_issue_url(&config.gitlab_domain, &project, &res);
            if open_browser {
                open_gitlab(&config.gitlab_domain, &project, Some(res))?
            }
            Ok(format!("Created issue #{} {}", res, url))
        }
        &Cmd::Browse {} => {
            let config = read_config()?;
            let project = extract_project(&config)?;
            let _ = open_gitlab(&config.gitlab_domain, &project, None);
            Ok(format!("Opening {}", &project))
        },
        &Cmd::ListIssues {
            ref filter_state,
        } => {
            let config = read_config()?;
            let project = extract_project(&config)?;
            list_issues(config, &project, filter_state)
        },
        &Cmd::Init {} => {
            init_config()?;
            Ok(format!(r#"
Wonderful! Config has been saved in `$XDG_CONFIG_HOME/issues-helper`.
By default, `$XDG_CONFIG_HOME` is `~/.config`.
You can now `cd` to a project directory and type:
`gli o "My issue"` to easily open issues.
It will pick up the project from the `origin` git remote.
Try `gli o --help` to see options.
Happy hacking :-)"#))
        }
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "gl-helper", about = "Gitlab helper.")]
enum Cmd {
    #[structopt(name = "b", about = "Open gitlab page in the browser")] Browse {},
    #[structopt(name = "o", about = "Open issue")]
    OpenIssue {
        #[structopt(name = "open", short = "o", long = "open", help = "Open browser after having created the issue")] open_browser: bool,
        #[structopt(name = "label", short = "l", long = "label", help = "Add labels to the issue")] labels: Vec<String>,
        #[structopt(name = "assignee", short = "a", long = "assignee", help = "Assigne the issue to a user")] assignee: Option<String>,
        title: String,
        text: Option<String>,
    },
    #[structopt(name = "init", about = "Generate configuration")] Init {},
    #[structopt(name = "l", about = "List all gitlab issues")]
    ListIssues {
        #[structopt(name = "filter", short = "f", long = "filter",
                    default_value = "open",
                    help = "Filter the issues by state. Possible values are: open, closed, reopened")]
        filter_state: MyIssueState,
    },
}

fn main() {
    let cmd = Cmd::from_args();
    match do_work(&cmd) {
        Ok(str) => println!("{}", str),
        Err(e) => println!("Something happened: {}", e),
    }
}
