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
}

pub fn extract_project(config: &Config) -> Result<String, Box<Error>> {
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

    Ok(Config {
        gitlab_domain: gitlab_domain.to_owned(),
        gitlab_token: gitlab_token.to_owned(),
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