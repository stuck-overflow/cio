use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::from_utf8;

use clap::ArgMatches;
use csv::ReaderBuilder;
use futures::Future;
use hubcaps::http_cache::FileBasedCache;
use hubcaps::{Credentials, Github};
use log::info;
use reqwest::blocking::Client;
use tokio::runtime::Runtime;
use yup_oauth2::{
    service_account_key_from_file, GetToken, ServiceAccountAccess, Token,
};

use crate::core::{Config, RFD};

/// Write a file.
pub fn write_file(file: PathBuf, contents: String) {
    // create each directory.
    fs::create_dir_all(file.parent().unwrap()).unwrap();

    // Write to the file.
    let mut f = fs::File::create(file.clone()).unwrap();
    f.write_all(contents.as_bytes()).unwrap();

    info!("wrote file: {}", file.to_str().unwrap());
}

/// Read and decode the config from the files that are passed on the command line.
pub fn read_config_from_files(cli_matches: &ArgMatches) -> Config {
    let files: Vec<String>;
    match cli_matches.values_of("file") {
        None => panic!("no configuration files specified"),
        Some(val) => {
            files = val.map(|s| s.to_string()).collect();
        }
    };

    let mut contents = String::from("");
    for file in files.iter() {
        info!("decoding {}", file);

        // Read the file.
        let body = fs::read_to_string(file).expect("reading the file failed");

        // Append the body of the file to the rest of the contents.
        contents.push_str(&body);
    }

    // Decode the contents.
    let config: Config = toml::from_str(&contents).unwrap();

    return config;
}

/// Get a GSuite token.
pub fn get_gsuite_token() -> Token {
    // Get the GSuite credentials file.
    let gsuite_credential_file = env::var("GADMIN_CREDENTIAL_FILE").unwrap();
    let gsuite_subject = env::var("GADMIN_SUBJECT").unwrap();
    let gsuite_secret =
        service_account_key_from_file(&gsuite_credential_file.to_string())
            .unwrap();
    let mut auth = ServiceAccountAccess::new(gsuite_secret)
        .sub(gsuite_subject.to_string())
        .build();

    // Add the scopes to the secret and get the token.
    let mut runtime = Runtime::new().unwrap();
    let t = auth
        .token(vec![
            "https://www.googleapis.com/auth/admin.directory.group",
            "https://www.googleapis.com/auth/admin.directory.resource.calendar",
            "https://www.googleapis.com/auth/admin.directory.user",
            "https://www.googleapis.com/auth/apps.groups.settings",
            "https://www.googleapis.com/auth/spreadsheets",
            "https://www.googleapis.com/auth/drive",
        ])
        .then(|tok| Ok(return tok));

    let token = runtime.block_on(t).unwrap();

    if token.access_token.len() < 1 {
        panic!("empty token is not valid");
    }

    return token;
}

/// Authenticate with GitHub.
pub fn authenticate_github() -> Github {
    // Initialize the github client.
    let github_token = env::var("GITHUB_TOKEN").unwrap();
    // Get the current working directory.
    let curdir = env::current_dir().unwrap();
    // Create the HTTP cache.
    let http_cache =
        Box::new(FileBasedCache::new(curdir.join(".cache/github")));
    return Github::custom(
        "https://api.github.com",
        concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")),
        Credentials::Token(github_token),
        Client::builder().build().unwrap(),
        http_cache,
    );
}

/// Get the RFDs from the rfd GitHub repo.
pub fn get_rfds_from_repo(github: Github) -> BTreeMap<i32, RFD> {
    let github_org = env::var("GITHUB_ORG").unwrap();
    let mut runtime = Runtime::new().unwrap();

    // Get the contents of the .helpers/rfd.csv file.
    let rfd_csv_content = runtime
        .block_on(
            github
                .repo(github_org.to_string(), "rfd")
                .content()
                .file(".helpers/rfd.csv"),
        )
        .unwrap()
        .content;
    let rfd_csv_string = from_utf8(&rfd_csv_content).unwrap();

    // Create the csv reader.
    let mut csv_reader = ReaderBuilder::new()
        .delimiter(b',')
        .has_headers(true)
        .from_reader(rfd_csv_string.as_bytes());

    // Create the BTreeMap of RFDs.
    let mut rfds: BTreeMap<i32, RFD> = Default::default();
    for r in csv_reader.records() {
        let record = r.unwrap();
        // Add this to our BTreeMap.
        rfds.insert(
            record[0].to_string().parse::<i32>().unwrap(),
            RFD {
                number: record[0].to_string(),
                title: record[1].to_string(),
                link: record[2].to_string(),
                state: record[3].to_string(),
                discussion: record[4].to_string(),
            },
        );
    }

    return rfds;
}

/// The warning for files that we automatically generate so folks don't edit them
/// all willy nilly.
pub static TEMPLATE_WARNING: &'static str =
    "# THIS FILE HAS BEEN GENERATED BY THE CONFIGS REPO
# AND SHOULD NEVER BE EDITED BY HAND!!
# Instead change the link in configs/links.toml

";
