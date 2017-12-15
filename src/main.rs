extern crate dialoguer;
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
#[macro_use]
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate tokio_core;
extern crate toml;
extern crate url;
extern crate xdg;

use dialoguer::Editor;
use futures::{Future, Stream};
use gitlab::Gitlab;
use gitlab::types::*;
use hyper::{Chunk, Client, Post, Request};
use hyper_tls::HttpsConnector;
use nom::IResult::Done;
use nom::be_u8;
use rprompt::prompt_reply_stdout;
use serde_json::Value;
use std::error::Error;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::str::FromStr;
use std::fmt;
use structopt::StructOpt;
use tokio_core::reactor::Core;
use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET};
use xdg::BaseDirectories;

#[derive(Debug)]
struct MyIssueState(IssueState);

impl FromStr for MyIssueState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "open" => Ok(MyIssueState(IssueState::Opened)),
            "closed" => Ok(MyIssueState(IssueState::Closed)),
            "reopened" => Ok(MyIssueState(IssueState::Reopened)),
            _ => Err(format!("Unknown state: {}", s)),
        }
    }
}

impl From<IssueState> for MyIssueState {
    fn from(issue: IssueState) -> Self {
        match issue {
            IssueState::Opened => MyIssueState(IssueState::Opened),
            IssueState::Closed => MyIssueState(IssueState::Closed),
            IssueState::Reopened => MyIssueState(IssueState::Reopened),
        }
    }
}

impl fmt::Display for MyIssueState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            IssueState::Opened => write!(f, "open"),
            IssueState::Closed => write!(f, "closed"),
            IssueState::Reopened => write!(f, "reopened"),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Config {
    gitlab_domain: String,
    gitlab_token: String,
}

fn extract_project(config: &Config) -> Result<String, Box<Error>> {
    let repo = git2::Repository::open(".")?;
    let remote = repo.find_remote("origin")?;
    let origin = remote.url().ok_or("origin is not valid UTF8")?;

    named!(
        raw_ssh,
        do_parse!(
            tag!("git@") >>
            domain: take_while!(|c: u8| c as char != ':') >>
            tag!(":") >> (domain)
        )
    );

    named!(
        ssh_url,
        do_parse!(
            tag!("git+ssh://") >>
            domain: take_while!(|c: u8| c as char != '/') >>
            tag!("/") >> (domain)
        )
    );

    named!(
        https_url,
        do_parse!(
            tag!("https://") >>
            domain: take_while!(|c: u8| c as char != '/') >>
            tag!("/") >> (domain)
        )
    );

    named!(
        repo_name<String>,
        map_res!(
            many_till!(call!(be_u8), alt_complete!(tag!(".git") | eof!())),
            |(bytes, _)| String::from_utf8(bytes)
        )
    );


    named!(
        address<(String, String)>,
        do_parse!(
        domain: map_res!(
            alt_complete!(
                raw_ssh |
                ssh_url |
                https_url
            ),
            |bytes| std::str::from_utf8(bytes).map(|s| s.to_owned())
        ) >>
        project: repo_name >>
        (domain, project)
    )
    );

    match address(origin.as_bytes()) {
        Done(_, (domain, project)) => {
            if domain == config.gitlab_domain {
                Ok(project)
            } else {
                Err(format!(
                    "Couldn't find credentials for {}, only {} is supported",
                    domain,
                    config.gitlab_domain
                ).into())
            }
        }
        e => Err(format!("Couldn't parse 'orgin' remote: {:?}", e).into()),
    }
}

fn create_issue_visual(
    config: &Config,
    project: &str,
    title: &str,
    labels: &Vec<String>,
    assignee: &Option<String>,
) -> Result<u32, Box<Error>> {
    let desc = Editor::new().edit("Issue body").unwrap();
    create_issue(config, project, title, &desc, labels, assignee)
}

fn create_issue(
    config: &Config,
    project: &str,
    title: &str,
    text: &Option<String>,
    labels: &Vec<String>,
    assignee: &Option<String>,
) -> Result<u64, Box<Error>> {
    let encoded_project = utf8_percent_encode(project, PATH_SEGMENT_ENCODE_SET);
    let desc = &text.clone().unwrap_or(String::new());
    let concat = labels.join(",");
    let assignee_id = if let &Some(ref a) = assignee {
        let r = get_user_id_by_name(a)?;
        format!("{}", r.value())
    } else {
        String::new()
    };

    let url = format!(
        "https://{}/api/v4/projects/{}/issues",
        &config.gitlab_domain,
        encoded_project
    );
    let mut core = Core::new()?;
    let connector = HttpsConnector::new(4, &core.handle())?;
    let client = Client::configure()
        .connector(connector)
        .build(&core.handle());

    let uri = url.parse()?;
    let mut request = Request::new(Post, uri);
    request
        .headers_mut()
        .set_raw("PRIVATE-TOKEN", config.gitlab_token.as_str());

    request.set_body(
        json!({
            "title": title,
            "description": desc,
            "labels": concat,
            "assignee_ids": assignee_id
        }).to_string());

    let work = client.request(request).and_then(|res| {
        res.body().concat2().and_then(move |body: Chunk| {
            let v: Value = serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let id: u64 = serde_json::from_value(v["iid"].clone()).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(id)
        })
    });
    Ok(core.run(work)?)
}

fn get_user_id_by_name(name: &str) -> Result<UserId, Box<Error>> {
    let config = read_config()?;
    let gl = Gitlab::new(&config.gitlab_domain, &config.gitlab_token)?;
    let user: gitlab::User = gl.user_by_name(name)?;
    Ok(user.id)
}

fn list_issues(config: Config, project: &str, filter_state: &MyIssueState) -> Result<String, Box<Error>> {
    let gitlab_client = Gitlab::new(&config.gitlab_domain, &config.gitlab_token)?;
    let project = gitlab_client.project_by_name(project)?;

    gitlab_client
        .issues(project.id)
        .and_then(|issues| {
            issues
                .into_iter()
                .filter(|i| i.state == filter_state.0)
                .for_each(|i| {
                    println!(
                        "#{} {} {} {} {}",
                        i.iid,
                        MyIssueState::from(i.state),
                        i.title,
                        i.created_at.format("%F %H:%M"),
                        get_issue_url(
                            &config.gitlab_domain,
                            &project.path_with_namespace,
                            &i.iid.value()
                        )
                    )
                });
            Ok("".to_string())
        })
        .map_err(From::from)
}

fn get_issue_url(domain: &str, project: &str, number: &u64) -> String {
    format!("https://{}/{}/issues/{}", domain, project, number)
}

fn get_project_url(domain: &str, project: &str) -> String {
    format!("https://{}/{}", domain, project)
}

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
        &Cmd::OpenIssueVisual {
            open_browser,
            ref labels,
            ref assignee,
            ref title,
        } => {
            let config = read_config()?;
            let project = extract_project(&config)?;
            let res = create_issue_visual(&config, &project, title, labels, assignee)?;
            let url = format!(
                "https://{}/{}/issues/{}",
                &config.gitlab_domain,
                &project,
                &res
            );
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

fn open_gitlab(domain: &str, p: &str, issue: Option<u64>) -> Result<(), Box<Error>> {
    if let Some(i) = issue {
        open::that(get_issue_url(domain, p, &i))?;
    } else {
        open::that(get_project_url(domain, p))?;
    }
    Ok(())
}

fn init_config() -> Result<(), Box<Error>> {
    let config = ask_config()?;
    save_config(&config)?;
    Ok(())
}

fn ask_config() -> Result<Config, Box<Error>> {
    println!("Hi! First I need to know the domain name of your gitlab instance (eg gitlab.example.org)");
    let gitlab_domain = prompt_reply_stdout("Gitlab domain name: ")?;
    println!("Thanks, now I need a personal access token to authenticate calls.");
    println!("You can generate one here: https://{}/profile/personal_access_tokens", &gitlab_domain);
    let gitlab_token = prompt_reply_stdout("Gitlab personal access token: ")?;

    Ok(Config {
        gitlab_domain: gitlab_domain.to_owned(),
        gitlab_token: gitlab_token.to_owned(),
    })
}

fn save_config(config: &Config) -> Result<(), Box<Error>> {
    let toml = toml::to_string(&config)?;
    let path = BaseDirectories::new()?.place_config_file("issues-helper")?;
    let mut f = File::create(path)?;
    f.write(toml.as_bytes())?;

    Ok(())
}

fn read_config() -> Result<Config, Box<Error>> {
    let path = BaseDirectories::new()?.place_config_file("issues-helper")?;
    let missing_config: Box<Error> = format!(
r#"It looks like you've not configured me yet.
Please run `gli init` so we can get going!"#).into();
    let mut f = File::open(path).map_err(|_| missing_config)?;

    let mut contents = String::new();
    f.read_to_string(&mut contents)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
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
    #[structopt(name = "v", about = "Open issue and edit body in $EDITOR")]
    OpenIssueVisual {
        #[structopt(name = "open", short = "o", long = "open", help = "Open browser after having created the issue")] open_browser: bool,
        #[structopt(name = "label", short = "l", long = "label", help = "Add labels to the issue")] labels: Vec<String>,
        #[structopt(name = "assignee", short = "a", long = "assignee", help = "Assigne the issue to a user")] assignee: Option<String>,
        title: String,
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
