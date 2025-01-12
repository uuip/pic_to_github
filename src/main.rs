use anyhow::anyhow;
use base64::engine::general_purpose;
use base64::Engine;
use home::home_dir;
use reqwest::blocking::Client;
use reqwest::header;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::read_to_string;
use std::iter::Iterator;
use std::path::Path;
use std::time::Duration;
use std::{env, fs};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:112.0) Gecko/20100101 Firefox/132.0";
#[derive(Debug, Deserialize)]
struct Repo {
    owner: String,
    repo: String,
    path: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Committer {
    name: String,
    email: String,
}
#[derive(Debug, Deserialize)]
struct Config {
    token: String,
    github_proxy: Option<String>,
    repo: Repo,
    committer: Committer,
}

fn main() -> anyhow::Result<()> {
    let config_path = home_dir()
        .ok_or(anyhow!("Can't get home directory"))?
        .join(".config/pic_to_github.toml");
    let config: Config = toml::from_str(&read_to_string(config_path)?)?;

    let client = build_client(&config.token)?;
    let repo_url = build_repo_url(&config.repo);
    let github_proxy = config.github_proxy.unwrap_or_default();

    let command_line_args: Vec<String> = env::args().skip(1).collect();
    if command_line_args.is_empty() {
        return Err(anyhow!("No image paths provided"));
    }
    for img_path in command_line_args.into_iter() {
        let url = format!(
            "{}/{}{}",
            repo_url,
            short_uuid::ShortUuid::generate(),
            get_suffix(&img_path)
        );

        let content = encode_img(&img_path);
        let data = json!({
        "message"  : "add image",
        "committer": {
                "name" : config.committer.name,
                "email": config.committer.email
                },
        "content"  : content
        });

        let rsp = client.put(&url).json(&data).send()?.json::<Value>()?;
        let download_url = rsp["content"]["download_url"]
            .as_str()
            .ok_or(anyhow!("commit failed: {url} {rsp}"))?;
        println!("{github_proxy}{download_url}");
    }
    Ok(())
}

fn encode_img(img_path: &str) -> String {
    let buffer = fs::read(img_path).expect("Failed to read image file");
    general_purpose::STANDARD.encode(&buffer)
}

fn build_client(token: &str) -> anyhow::Result<Client> {
    let mut headers: HeaderMap = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static(UA));
    headers.insert(
        header::AUTHORIZATION,
        format!("token {}", token).try_into()?,
    );
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static("2022-11-28"),
    );

    Client::builder()
        .default_headers(headers)
        .gzip(true)
        .tcp_keepalive(Some(Duration::from_secs(10)))
        .build()
        .map_err(Into::into)
}

fn get_suffix(img_path: &str) -> String {
    Path::new(img_path)
        .extension()
        .map_or(String::new(), |ext| format!(".{}", ext.to_string_lossy()))
}

fn build_repo_url(repo: &Repo) -> String {
    format!(
        "https://api.github.com/repos/{}/{}/contents/{}",
        repo.owner,
        repo.repo,
        repo.path.as_deref().unwrap_or_default()
    )
    .trim_end_matches("/")
    .to_owned()
}
