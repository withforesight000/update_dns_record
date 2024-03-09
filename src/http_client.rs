use reqwest::blocking::Response;

#[cfg(test)]
use mockall::{automock, predicate::*};
use serde_json::Value;
use std::fmt;

pub struct ReqwestClient {
    client: reqwest::blocking::Client,
}

impl ReqwestClient {
    pub fn new() -> ReqwestClient {
        ReqwestClient {
            client: reqwest::blocking::Client::new(),
        }
    }
}

#[cfg_attr(test, automock)]
pub trait HttpClient {
    fn get_with_bearer_token(
        &self,
        url: &str,
        bearer_token: &str,
    ) -> Result<Response, Box<dyn std::error::Error>>;
    fn patch_with_bearer_token(
        &self,
        url: &str,
        bearer_token: &str,
        req_body: &Value,
    ) -> Result<Response, Box<dyn std::error::Error>>;
}

impl HttpClient for ReqwestClient {
    fn get_with_bearer_token(
        &self,
        url: &str,
        bearer_token: &str,
    ) -> Result<Response, Box<dyn std::error::Error>> {
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", bearer_token))
            .send();
        match response {
            Ok(response) => Ok(response),
            Err(e) => Err(Box::new(e)),
        }
    }

    fn patch_with_bearer_token(
        &self,
        url: &str,
        bearer_token: &str,
        req_body: &Value,
    ) -> Result<Response, Box<dyn std::error::Error>> {
        let response = self
            .client
            .patch(url)
            .header("Authorization", format!("Bearer {}", bearer_token))
            .header("Content-Type", "application/json")
            .json(req_body)
            .send();
        match response {
            Ok(response) => Ok(response),
            Err(e) => Err(Box::new(e)),
        }
    }
}

#[derive(Debug)]
pub struct MockNetworkError;

impl fmt::Display for MockNetworkError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Mock network error")
    }
}

impl std::error::Error for MockNetworkError {}
