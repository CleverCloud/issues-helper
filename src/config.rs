use nom::IResult::Done;
use nom::be_u8;
use rprompt::prompt_reply_stdout;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use xdg::BaseDirectories;
use git2;
use std;
use toml;

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub gitlab_domain: String,
    pub gitlab_token: String,
    pub github_token: String,
}

#[derive(Debug)]
pub enum Place {
    Github,
    Gitlab(String)
}

#[derive(Debug)]
pub struct Project {
    pub place: Place,
    pub owner: String,
    pub repo: String
}

impl Project {
    pub fn name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

fn parse_origin(origin: &str) -> Result<(String,String,String), Box<Error>> {
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
            tag!("git+ssh://git@") >>
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
        owner<String>,
        map_res!(
            many_till!(call!(be_u8), tag!("/")),
            |(bytes, _)| String::from_utf8(bytes)
        )
    );

    named!(
        repo<String>,
        map_res!(
            many_till!(call!(be_u8), alt_complete!(tag!(".git") | eof!())),
            |(bytes, _)| String::from_utf8(bytes)
        )
    );

    named!(
        address<(String, String,String)>,
        do_parse!(
        domain: map_res!(
            alt_complete!(
                raw_ssh |
                ssh_url |
                https_url
            ),
            |bytes| std::str::from_utf8(bytes).map(|s| s.to_owned())
        ) >>
        owner: owner >>
        repo: repo >>
        (domain, owner, repo)
    )
    );

    match address(origin.as_bytes()) {
        Done(_, (domain, owner, repo)) => {
                Ok((domain, owner, repo))
        }
        e => Err(format!("Couldn't parse 'orgin' remote: {:?}", e).into()),
    }
}

#[cfg(test)]
mod parsing_tests {
    use super::*;

    #[test]
    fn parsing_raw_ssh_with_ext() {
        let raw_ssh_with_ext = "git@github.com:CleverCloud/issues-helper.git";
        assert_eq!(
            parse_origin(raw_ssh_with_ext).unwrap_or((String::new(), String::new(), String::new())),
            ("github.com".into(), "CleverCloud".into(), "issues-helper".into())
        );
    }
    #[test]
    fn parsing_ssh_url_with_ext() {
        let ssh_url_with_ext = "git+ssh://git@github.com/CleverCloud/issues-helper.git";
        assert_eq!(
            parse_origin(ssh_url_with_ext).unwrap_or((String::new(), String::new(), String::new())),
            ("github.com".into(), "CleverCloud".into(), "issues-helper".into())
        );
    }
    #[test]
    fn parsing_https_url_with_ext() {
        let https_url_with_ext = "https://github.com/CleverCloud/issues-helper.git";
        assert_eq!(
            parse_origin(https_url_with_ext).unwrap_or((String::new(), String::new(), String::new())),
            ("github.com".into(), "CleverCloud".into(), "issues-helper".into())
        );
    }
    #[test]
    fn parsing_raw_ssh() {
        let raw_ssh = "git@github.com:CleverCloud/issues-helper";
        assert_eq!(
            parse_origin(raw_ssh).unwrap_or((String::new(), String::new(), String::new())),
            ("github.com".into(), "CleverCloud".into(), "issues-helper".into())
        );
    }
    #[test]
    fn parsing_ssh_url() {
        let ssh_url = "git+ssh://git@github.com/CleverCloud/issues-helper";
        assert_eq!(
            parse_origin(ssh_url).unwrap_or((String::new(), String::new(), String::new())),
            ("github.com".into(), "CleverCloud".into(), "issues-helper".into())
        );
    }
    #[test]
    fn parsing_https_url() {
        let https_url = "https://github.com/CleverCloud/issues-helper";
        assert_eq!(
            parse_origin(https_url).unwrap_or((String::new(), String::new(), String::new())),
            ("github.com".into(), "CleverCloud".into(), "issues-helper".into())
        );
    }
}

pub fn extract_project(config: &Config) -> Result<Project, Box<Error>> {
    let repo = git2::Repository::open(".")?;
    let remote = repo.find_remote("origin")?;
    let origin = remote.url().ok_or("origin is not valid UTF8")?;
    let (domain, owner, repo) = parse_origin(&origin)?;

    if domain == config.gitlab_domain {
        Ok(Project {
            place: Place::Gitlab(domain),
            owner,
            repo
        })
    } else if domain == "github.com" {
        Ok(Project {
            place: Place::Github,
            owner,
            repo
        })
    } else {
        Err(format!(
           "Couldn't find credentials for {}, only {} is supported",
           domain,
           config.gitlab_domain
        ).into())
    }
}

pub fn init_config() -> Result<(), Box<Error>> {
    let config = ask_config()?;
    save_config(&config)?;
    Ok(())
}

pub fn ask_config() -> Result<Config, Box<Error>> {
    println!("Hi! First I need to know the domain name of your gitlab instance (eg gitlab.example.org)");
    let gitlab_domain = prompt_reply_stdout("Gitlab domain name: ")?;
    println!("Thanks, now I need a personal access token to authenticate calls.");
    println!("You can generate one here: https://{}/profile/personal_access_tokens", &gitlab_domain);
    let gitlab_token = prompt_reply_stdout("Gitlab personal access token: ")?;
    println!("Wonderful! Now I'll need a *github* personal access token.");
    println!("You can generate one here: https://github.com/settings/tokens/new");
    println!("You only need to check the `Repo` scope");
    let github_token = prompt_reply_stdout("Github personal access token: ")?;

    Ok(Config {
        gitlab_domain: gitlab_domain.to_owned(),
        gitlab_token: gitlab_token.to_owned(),
        github_token: github_token.to_owned(),
    })
}

pub fn save_config(config: &Config) -> Result<(), Box<Error>> {
    let toml = toml::to_string(&config)?;
    let path = BaseDirectories::new()?.place_config_file("issues-helper")?;
    let mut f = File::create(path)?;
    f.write(toml.as_bytes())?;

    Ok(())
}

pub fn read_config() -> Result<Config, Box<Error>> {
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