use std::collections::HashMap;

use reqwest::{self, StatusCode};
use serde_json::{json, Value};

use crate::errors::ClientError;
pub struct CloudflareClient {
    api_token: String,
    base_address: String,
    zone_id: String
}

impl CloudflareClient {
    pub fn new(api_token: String, base_address: String, zone_id: String) -> CloudflareClient {
        CloudflareClient {
            api_token,
            base_address,
            zone_id
        }
    }

    fn handle_response(&self, response: Result<reqwest::blocking::Response, reqwest::Error>) -> Result<HashMap<String, Value>, ClientError> {
        match response {
            Ok(response) => match response.status() {
                StatusCode::OK => {
                    let body: Result<HashMap<String, Value>, reqwest::Error> = response.json::<HashMap<String, Value>>();
                    match body {
                        Ok(body) => {
                            Ok(body)
                        }
                        Err(error) => {
                            Err(ClientError::BodyError(error))
                        }
                    }
                }
                other => {
                    Err(ClientError::StatusCodeError(other))
                }
            }
            Err(error) => {
                Err(ClientError::RequestError(error))
            }
        }
    }

    pub fn get_dns_records(&self) -> Result<HashMap<String, Value>, ClientError> {
        let url = format!("{}/zones/{}/dns_records", self.base_address, self.zone_id);
        let client = reqwest::blocking::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send();
        self.handle_response(response)
    }

    pub fn find_record_ids(&self, dns_records: HashMap<String, Value>, record_name: &str) -> Result<Vec<Value>, ClientError> {
        let record = dns_records["result"]
            .as_array()
            .map(|array| {
                array
                    .iter()
                    .filter(|record| record["name"].as_str().unwrap() == record_name)
                    .cloned()
                    .collect::<Vec<Value>>()
            });
        match record {
            Some(record) if !record.is_empty() => {
                Ok(record)
            }
            _ => Err(ClientError::RecordNotFound),
        }
    }

    pub fn update_record(&self, record_id: &Value, record: &Value) -> Result<HashMap<String, Value>, ClientError> {
        let url = format!("{}/zones/{}/dns_records/{}", self.base_address, self.zone_id, record_id.as_str().unwrap());
        let req_body = json!({"content": record});
        let client = reqwest::blocking::Client::new();
        let response = client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send();
        self.handle_response(response)
    }
}
