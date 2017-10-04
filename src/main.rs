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

use std::error::Error;
use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET, QUERY_ENCODE_SET};
use nom::IResult::Done;
use std::io;
use futures::{Future, Stream};
use hyper::{Client,Request,Post,Chunk};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;
use serde_json::{Value};
use std::fs::File;
use std::io::prelude::*;
use xdg::BaseDirectories;
use std::env;

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
        tag!("git@CHANGEME:") >>
        a: map!(map_res!(take_until!(".git"), std::str::from_utf8), ToString::to_string) >>
        (a)
    ));

    match address(origin.as_bytes()) {
        Done(_, s) => Ok(s),
        e => Err(format!("Couldn't parse 'orgin' remote: {:?}", e).into())
    }
}

fn create_issue(api_token: &str, project: &str, title: &str) -> Result<u32, Box<Error>> {
    let encoded_project = utf8_percent_encode(project, PATH_SEGMENT_ENCODE_SET);
    let encoded_title = utf8_percent_encode(title, QUERY_ENCODE_SET);
    let url = format!("https://CHANGEME/api/v4/projects/{}/issues?title={}", encoded_project, encoded_title);
    let mut core = Core::new()?;
    let client = Client::configure()
        .connector(HttpsConnector::new(4, &core.handle()).unwrap())
        .build(&core.handle());


    let uri = url.parse()?;
    let mut request = Request::new(Post, uri);
    request.headers_mut().set_raw("PRIVATE-TOKEN", api_token);

    let work = client.request(request)
      .and_then(|res| {
        res.body().concat2().and_then(move |body: Chunk| {
            let v: Value = serde_json::from_slice(&body).map_err(|e| {
                io::Error::new(
                io::ErrorKind::Other,
                e)
            })?;
            let id: u32 = serde_json::from_value(v["iid"].clone()).map_err(|e| {
                io::Error::new(
                io::ErrorKind::Other,
                e)
            })?;
            Ok(id)
        })
    });
    Ok(core.run(work)?)
}

fn do_work() -> Result<u32, Box<Error>> {
    let key = read_key()?;
    let project = extract_project()?;
    let title = env::args().nth(1).ok_or("Argument required: issue title")?;
    let res = create_issue(&key, &project, &title)?;
    Ok(res)
}

fn main() {
    match do_work() {
        Ok(id) => println!("Created issue #{}", id),
        Err(e) => println!("Something happened: {}", e)
    }
}
