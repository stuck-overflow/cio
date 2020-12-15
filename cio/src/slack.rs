use std::collections::HashMap;
use std::env;

use chrono::offset::Utc;
use chrono::serde::ts_seconds;
use chrono::DateTime;
use reqwest::{Body, Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::instrument;

/// The Slack app webhook URL for our app to post to the #hiring channel.
#[instrument]
#[inline]
pub fn get_hiring_channel_post_url() -> String {
    env::var("SLACK_HIRING_CHANNEL_POST_URL").unwrap()
}

/// The Slack app webhook URL for our app to post to the #public-relations channel.
#[instrument]
#[inline]
pub fn get_public_relations_channel_post_url() -> String {
    env::var("SLACK_PUBLIC_RELATIONS_CHANNEL_POST_URL").unwrap()
}

/// Post text to a channel.
#[instrument]
#[inline]
pub async fn post_to_channel(url: String, v: Value) {
    let client = Client::new();
    let resp = client.post(&url).body(Body::from(v.to_string())).send().await.unwrap();

    match resp.status() {
        StatusCode::OK => (),
        s => {
            println!("posting to slack webhook ({}) failed, status: {} | resp: {}", url, s, resp.text().await.unwrap());
        }
    };
}
