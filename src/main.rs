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
#[macro_use]
extern crate itertools;
extern crate gitlab;

use gitlab::Gitlab;
use gitlab::types::*;
use itertools::Itertools;
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

fn read_key() -> Result<String, Box<Error>> {
    let path = BaseDirectories::new()?.place_config_file("gitlab-clever")?;
    let mut f = File::open(path)?;
    let mut contents = String::new();
    let _ = f.read_to_string(&mut contents)?;

    Ok(contents.trim().to_string())
}

fn extract_project() -> Result<String, Box<Error>> {
    let repo = git2::Repository::open(".")?;
    let remote = repo.find_remote("origin")?;
    let origin = remote.url().ok_or("origin is not valid UTF8")?;

    named!(address<String>, do_parse!(
        alt_complete!(tag!("git@CHANGEME:") | tag!("git+ssh://git@CHANGEME/")) >>
        a: map_res!(many_till!(call!(be_u8), alt_complete!(tag!(".git") | eof!())), |(bytes, _)| String::from_utf8(bytes)) >>
        (a)
    ));

    match address(origin.as_bytes()) {
        Done(_, s) => Ok(s),
        e => Err(format!("Couldn't parse 'orgin' remote: {:?}", e).into()),
    }
}

fn create_issue(
    api_token: &str,
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
        "https://CHANGEME/api/v4/projects/{}/issues?title={}&description={}{}{}",
        encoded_project,
        encoded_title,
        encoded_desc,
        &labels_param,
        &assignee_param);
    let mut core = Core::new()?;
    let connector = HttpsConnector::new(4, &core.handle())?;
    let client = Client::configure().connector(connector).build(
        &core.handle(),
    );


    let uri = url.parse()?;
    let mut request = Request::new(Post, uri);
    request.headers_mut().set_raw("PRIVATE-TOKEN", api_token);

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
    let key = read_key()?;
    let gl = Gitlab::new("gitlab.clever-cloud.com", key)?;
    let user: gitlab::User = gl.user_by_name(name)?;
    Ok(user.id)
}

fn do_work(cmd: &Cmd) -> Result<String, Box<Error>> {
    let key = read_key()?;
    let project = extract_project()?;

    match cmd {
        &Cmd::OpenIssue {
            open_browser,
            ref labels,
            ref assignee,
            ref title,
            ref text,
        } => {
            let res = create_issue(&key, &project, title, text, labels, assignee)?;
            let url = format!("https://gitlab.clever-cloud.com/{}/issues/{}", &project, &res);
            if open_browser {
                open_gitlab(&project, Some(res))?
            }
            Ok(format!("Created issue #{} {}", res, url))
        }
        &Cmd::Browse {} => {
            let _ = open_gitlab(&project, None);
            Ok(format!("Opening {}", &project))
        }
    }
}

fn open_gitlab(p: &str, issue: Option<u32>) -> Result<(), Box<Error>> {
    if let Some(i) = issue {
        open::that(
            format!("https://gitlab.clever-cloud.com/{}/issues/{}", p, i),
        )?;
    } else {
        open::that(format!("https://gitlab.clever-cloud.com/{}", p))?;
    }
    Ok(())
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
}

fn main() {
    let cmd = Cmd::from_args();
    match do_work(&cmd) {
        Ok(str) => println!("{}", str),
        Err(e) => println!("Something happened: {}", e),
    }
}
