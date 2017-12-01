#[macro_use]
extern crate nom;
extern crate url;
extern crate futures;
extern crate git2;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio_core;
extern crate serde_json;
extern crate xdg;
extern crate open;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate itertools;
extern crate gitlab;
extern crate rprompt;
extern crate toml;
#[macro_use]
extern crate serde_derive;

use gitlab::Gitlab;
use gitlab::types::*;
use std::error::Error;
use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET, QUERY_ENCODE_SET};
use nom::IResult::Done;
use nom::be_u8;
use std::io;
use futures::{Future, Stream};
use hyper::{Client, Request, Post, Chunk};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;
use serde_json::Value;
use std::fs::File;
use std::io::prelude::*;
use xdg::BaseDirectories;
use structopt::StructOpt;
use rprompt::prompt_reply_stdout;

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
        repo_name<String>,
        map_res!(many_till!(
            call!(be_u8),
            alt_complete!(
                tag!(".git") |
                eof!())
        ), |(bytes, _)| String::from_utf8(bytes))
    );


    named!(address<(String,String)>, do_parse!(
        domain: map_res!(
            alt_complete!(
                raw_ssh |
                ssh_url
            ),
            |bytes| std::str::from_utf8(bytes).map(|s| s.to_owned())
        ) >>
        project: repo_name >>
        (domain, project)
    ));

    match address(origin.as_bytes()) {
        Done(_, (domain, project)) => Ok(project),
        e => Err(format!("Couldn't parse 'orgin' remote: {:?}", e).into()),
    }
}

fn create_issue(
    config: &Config,
    project: &str,
    title: &str,
    text: &Option<String>,
    labels: &Vec<String>,
    assignee: &Option<String>,
) -> Result<u32, Box<Error>> {
    let encoded_project = utf8_percent_encode(project, PATH_SEGMENT_ENCODE_SET);
    let encoded_title = utf8_percent_encode(title, QUERY_ENCODE_SET);
    let desc = &text.clone().unwrap_or(String::new());
    let encoded_desc = utf8_percent_encode(desc, QUERY_ENCODE_SET);
    let concat = labels.join(",");
    let encoded_labels = utf8_percent_encode(&concat, QUERY_ENCODE_SET);
    let labels_param = if labels.len() > 0 {
        format!("&labels={}", encoded_labels)
    } else {
        "".to_owned()
    };
    let assignee_param = if let &Some(ref a) = assignee {
        let r = get_user_id_by_name(a)?;
        format!("&assignee_ids={}", r.value())
    } else {
        String::new()
    };

    let url = format!(
        "https://{}/api/v4/projects/{}/issues?title={}&description={}{}{}",
        &config.gitlab_domain,
        encoded_project,
        encoded_title,
        encoded_desc,
        &labels_param,
        &assignee_param
    );
    let mut core = Core::new()?;
    let connector = HttpsConnector::new(4, &core.handle())?;
    let client = Client::configure().connector(connector).build(
        &core.handle(),
    );


    let uri = url.parse()?;
    let mut request = Request::new(Post, uri);
    request.headers_mut().set_raw(
        "PRIVATE-TOKEN",
        config.gitlab_token.as_str(),
    );

    let work = client.request(request).and_then(|res| {
        res.body().concat2().and_then(move |body: Chunk| {
            let v: Value = serde_json::from_slice(&body).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e)
            })?;
            let id: u32 = serde_json::from_value(v["iid"].clone()).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e)
            })?;
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
        }
        &Cmd::Init {} => {
            init_config()?;
            // ToDo give an example of how to use the tool
            Ok(format!("Config generated"))
        }
    }
}

fn open_gitlab(domain: &str, p: &str, issue: Option<u32>) -> Result<(), Box<Error>> {
    if let Some(i) = issue {
        open::that(format!(
            "https://{}/{}/issues/{}",
            domain,
            p,
            i
        ))?;
    } else {
        open::that(format!("https://{}/{}", domain, p))?;
    }
    Ok(())
}

fn init_config() -> Result<(), Box<Error>> {
    let config = ask_config()?;
    save_config(&config)?;
    Ok(())
}

fn ask_config() -> Result<Config, Box<Error>> {
    // ToDo give more indications about how to find the values
    let gitlab_domain = prompt_reply_stdout("Gitlab domain name: ")?;
    // maybe automatically open a browser window to generate the access token?
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
    let mut f = File::open(path)?;
    let mut contents = String::new();
    f.read_to_string(&mut contents)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}

#[derive(StructOpt, Debug)]
#[structopt(name = "gl-helper", about = "Gitlab helper.")]
enum Cmd {
    #[structopt(name = "b", about = "Open gitlab page in the browser")]
    Browse {},
    #[structopt(name = "o", about = "Open issue")]
    OpenIssue {
        #[structopt(name = "open", short = "o", long = "open",
                    help = "Open browser after having created the issue")]
        open_browser: bool,
        #[structopt(name = "label", short = "l", long = "label", help = "Add labels to the issue")]
        labels: Vec<String>,
        #[structopt(name = "assignee", short = "a", long = "assignee",
                    help = "Assigne the issue to a user")]
        assignee: Option<String>,
        title: String,
        text: Option<String>,
    },
    #[structopt(name = "init", about = "Generate configuration")]
    Init {},
}

fn main() {
    let cmd = Cmd::from_args();
    match do_work(&cmd) {
        Ok(str) => println!("{}", str),
        Err(e) => println!("Something happened: {}", e),
    }
}
