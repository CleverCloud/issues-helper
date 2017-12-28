use config::*;
use open;
use serde_json;
use serde_json::Value;
use std::error::Error;
use std::io;
use std::str::FromStr;
use std::fmt;
use std::result::Result;
use gh::client::{Executor, Github};

#[derive(Debug,Serialize)]
struct IssueData {
    title: String,
    body: Option<String>,
    labels: Vec<String>,
    assignees: Vec<String>,
}

#[derive(Debug,Deserialize)]
struct IssueCreationData {
    number: u64,
    html_url: String,
}

pub fn create_issue(
    config: &Config,
    project: &Project,
    title: &str,
    text: &Option<String>,
    labels: &Vec<String>,
    assignee: &Option<String>,
) -> Result<(u64,String), Box<Error>> {
    let mut assignees: Vec<String> = vec![];
    if let &Some(ref a) = assignee {
        assignees.push(a.clone());
    }
    let data = IssueData {
        title: title.to_owned(),
        body: text.clone(),
        labels: labels.clone(),
        assignees: assignees,
    };
    let client = Github::new(&config.github_token)?;
    let (_,_,result) = client.post(data)
        .repos()
        .owner(&project.owner)
        .repo(&project.repo)
        .issues()
        .execute::<IssueCreationData>()?;

    if let Some(d) = result {
        Ok((
            d.number,
            d.html_url,
        ))
    } else {
        Err(format!(
            "Error parsing github API response"
        ).into())
    }

}

#[derive(Debug,Deserialize)]
struct GhIssue {
    number: u64,
    html_url: String,
    title: String,
    created_at: String,
    state: String,
}

pub fn list_issues(config: Config, project: &Project, filter_state: &IssueFilter) -> Result<String, Box<Error>> {
    if filter_state != &IssueFilter::Open {
        println!("WARNING: Only open issues are currently returned by the API");
    }
    let client = Github::new(&config.github_token)?;
    let (_,_,issues) = client.get()
                   .repos()
                   .owner(&project.owner)
                   .repo(&project.repo)
                   .issues()
                   .execute::<Vec<GhIssue>>()?;
    issues.unwrap_or(vec![])
      .into_iter()
      .filter(|i| i.state == format!("{}",filter_state))
      .for_each(|i| {
        println!(
            "#{} {} {} {} {}",
            i.number,
            i.state,
            i.title,
            i.created_at,
            i.html_url
        )
      });
    
    Ok(String::new())
}

pub fn open_project(project: &Project) -> Result<(), Box<Error>> {
    open::that(format!(
        "https://github.com/{}/{}",
        &project.owner,
        &project.repo,
    ))?;
    Ok(())
}