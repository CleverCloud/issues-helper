use config::*;
use config::Project;
use gitlab::*;
use futures::{Future, Stream};
use gitlab;
use gitlab::Gitlab;
use gitlab::types::*;
use hyper::{Chunk, Client, Post, Request};
use hyper_tls::HttpsConnector;
use open;
use serde_json;
use serde_json::Value;
use std::error::Error;
use std::io;
use std::str::FromStr;
use std::fmt;
use std::result::Result;
use tokio_core::reactor::Core;
use url::percent_encoding::{utf8_percent_encode, PATH_SEGMENT_ENCODE_SET, QUERY_ENCODE_SET};

#[derive(Debug)]
pub struct MyIssueState(IssueState);

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

pub fn create_issue(
    config: &Config,
    project: &Project,
    title: &str,
    text: &Option<String>,
    labels: &Vec<String>,
    assignee: &Option<String>,
) -> Result<u64, Box<Error>> {
    let project_name = project.name();
    let encoded_project = utf8_percent_encode(&project_name, PATH_SEGMENT_ENCODE_SET);
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
    let client = Client::configure()
        .connector(connector)
        .build(&core.handle());


    let uri = url.parse()?;
    let mut request = Request::new(Post, uri);
    request
        .headers_mut()
        .set_raw("PRIVATE-TOKEN", config.gitlab_token.as_str());

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

pub fn list_issues(config: Config, project: &Project, filter_state: &IssueFilter) -> Result<String, Box<Error>> {
    let gitlab_client = Gitlab::new(&config.gitlab_domain, &config.gitlab_token)?;
    let project = gitlab_client.project_by_name(project.name())?;

    gitlab_client
        .issues(project.id)
        .and_then(|issues| {
            issues
                .into_iter()
                .filter(|i| { match filter_state {
                    &IssueFilter::Open => (i.state == IssueState::Opened) || i.state == IssueState::Reopened,
                    &IssueFilter::Closed => (i.state == IssueState::Closed),
                }})
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

pub fn get_issue_url(domain: &str, project_name: &str, number: &u64) -> String {
    format!("https://{}/{}/issues/{}", domain, project_name, number)
}

pub fn get_project_url(domain: &str, project: &Project) -> String {
    format!("https://{}/{}", domain, project.name())
}

pub fn open_gitlab(domain: &str, p: &Project, issue: Option<u64>) -> Result<(), Box<Error>> {
    if let Some(i) = issue {
        open::that(get_issue_url(domain, &p.name(), &i))?;
    } else {
        open::that(get_project_url(domain, p))?;
    }
    Ok(())
}