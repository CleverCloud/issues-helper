#[macro_use]
extern crate nom;
extern crate url;
extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio_core;
extern crate serde_json;
extern crate xdg;

use std::process::Command;
use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET};
use nom::IResult::Done;
use std::io::{self, Write};
use futures::{Future, Stream};
use hyper::{Client,Request,Post,Chunk};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::Core;
use serde_json::{Value};
use std::fs::File;
use std::io::prelude::*;
use xdg::BaseDirectories;
use std::env;

fn read_key() -> Result<String,()> {
    let path = try!(BaseDirectories::new().map_err(|_| ()).and_then(|d| d.place_config_file("gitlab-clever").map_err(|_| ())));
    let mut f = try!(File::open(path).map_err(|_| ()));
    let mut contents = String::new();
    let _ = try!(f.read_to_string(&mut contents).map_err(|_| ()));

    Ok(contents.trim().to_string())
}

fn extract_project() -> Result<String,()> {
    named!(address, do_parse!(
        tag!("git@CHANGME:") >>
        a: take_until!(".git") >>
        (a)
    ));

    let command = Command::new("git")
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .expect("failed to execute process");
    
    match address(&command.stdout[..]) {
        Done(_, a) => Ok(String::from_utf8(a[..].to_vec()).unwrap()),
        _ => Err(())
    }
}

fn create_issue(api_token: &str, project: &str, title: &str) -> Result<u32,()> {
    let encoded_project = utf8_percent_encode(project, PATH_SEGMENT_ENCODE_SET);
    let encoded_title = utf8_percent_encode(title, PATH_SEGMENT_ENCODE_SET);
    let url = format!("https://CHANGEME/api/v4/projects/{}/issues?title={}", encoded_project, encoded_title);
    let mut core = try!(Core::new().map_err(|_| ()));
    let client = Client::configure()
        .connector(HttpsConnector::new(4, &core.handle()).unwrap())
        .build(&core.handle());


    let uri = try!(url.parse().map_err(|_| ()));
    let mut request = Request::new(Post, uri);
    request.headers_mut().set_raw("PRIVATE-TOKEN", api_token);

    let work = client.request(request)
      .and_then(|res| {
        res.body().concat2().and_then(move |body: Chunk| {
            let v: Value = try!(serde_json::from_slice(&body).map_err(|e| {
                io::Error::new(
                io::ErrorKind::Other,
                e)
            }));
            let id: u32 = try!(serde_json::from_value(v["iid"].clone()).map_err(|e| {
                io::Error::new(
                io::ErrorKind::Other,
                e)
            }));
            Ok(id)
        })
    });
    let res = core.run(work);
    let id = try!(res.map_err(|_| ()));
    Ok(id)
}

fn do_work() -> Result<u32,()> {
    let key = read_key()?;
    let project = extract_project()?;
    let title = env::args().nth(1).ok_or(())?;
    let res = create_issue(&key, &project, &title)?;
    Ok(res)
}

fn main() {
    match do_work() {
        Ok(id) => println!("Created issue #{}", id),
        Err(_) => println!("Something happened")
    }
}