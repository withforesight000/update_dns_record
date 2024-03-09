use std::collections::HashMap;

use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::errors::ClientError;

use crate::http_client::HttpClient;
pub struct CloudflareClient<T: HttpClient> {
    client: T,
    api_token: String,
    base_address: String,
    zone_id: String,
}

impl<T: HttpClient> CloudflareClient<T> {
    pub fn new(
        client: T,
        api_token: String,
        base_address: String,
        zone_id: String,
    ) -> CloudflareClient<T> {
        CloudflareClient {
            client,
            api_token,
            base_address,
            zone_id,
        }
    }

    fn handle_response(
        &self,
        response: Result<reqwest::blocking::Response, Box<dyn std::error::Error>>,
    ) -> Result<HashMap<String, Value>, ClientError> {
        match response {
            Ok(response) => match response.status() {
                StatusCode::OK => {
                    let body: Result<HashMap<String, Value>, reqwest::Error> =
                        response.json::<HashMap<String, Value>>();
                    match body {
                        Ok(body) => Ok(body),
                        Err(error) => Err(ClientError::BodyError(error)),
                    }
                }
                other => Err(ClientError::StatusCodeError(other, response.text().unwrap())),
            },
            Err(error) => Err(ClientError::RequestError(error)),
        }
    }

    pub fn get_dns_records(&self) -> Result<HashMap<String, Value>, ClientError> {
        let url = format!("{}/zones/{}/dns_records", self.base_address, self.zone_id);
        let response = self
            .client
            .get_with_bearer_token(&url, self.api_token.as_str());
        self.handle_response(response)
    }

    pub fn find_record_ids(
        &self,
        dns_records: HashMap<String, Value>,
        record_name: &str,
    ) -> Result<Vec<Value>, ClientError> {
        let record = dns_records["result"].as_array().map(|array| {
            array
                .iter()
                .filter(|record| record["name"].as_str().unwrap() == record_name)
                .cloned()
                .collect::<Vec<Value>>()
        });
        match record {
            Some(record) if !record.is_empty() => Ok(record),
            _ => Err(ClientError::RecordNotFound),
        }
    }

    pub fn update_record(
        &self,
        record_id: &Value,
        record: &Value,
        name: &str,
        r#type: &str
    ) -> Result<HashMap<String, Value>, ClientError> {
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            self.base_address,
            self.zone_id,
            record_id.as_str().unwrap()
        );
        let req_body = json!({"content": record, "name": name, "type": r#type});
        let response = self.client.patch_with_bearer_token(
            &url,
            self.api_token.as_str(),
            &req_body,
        );
        self.handle_response(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::{MockHttpClient, MockNetworkError, ReqwestClient};

    use mockito::{Mock, ServerGuard};

    struct TestContext {
        server: ServerGuard,
        api_token: String,
        zone_id: String,
    }

    impl TestContext {
        fn new() -> TestContext {
            let server = mockito::Server::new();

            TestContext {
                server,
                api_token: "your_api_token".to_string(),
                zone_id: "your_zone_id".to_string(),
            }
        }
    }

    fn mock_get_dns_records(ctx: &mut TestContext) -> Mock {
        ctx.server
            .mock(
                "GET",
                format!("/zones/{}/dns_records", ctx.zone_id).as_str(),
            )
            .with_header(
                "Authorization",
                format!("Bearer {}", ctx.api_token).as_str(),
            )
    }

    #[test]

    // get_dns_records()
    fn test_get_dns_records_should_return_200() {
        let mut ctx = TestContext::new();
        let client = CloudflareClient::new(
            ReqwestClient::new(),
            ctx.api_token.clone(),
            ctx.server.url(),
            ctx.zone_id.clone(),
        );

        let response_body = json!({
            "result": [
                {
                    "id": "record_id_1",
                    "name": "example.com",
                    "content": "127.0.0.1"
                },
                {
                    "id": "record_id_2",
                    "name": "subdomain.example.com",
                    "content": "192.168.0.1"
                }
            ]
        });

        let api_token = ctx.api_token.clone();
        let _m = mock_get_dns_records(&mut ctx)
            .with_header("Authorization", format!("Bearer {}", api_token).as_str())
            .with_body(response_body.to_string())
            .with_status(200)
            .expect(1)
            .create();

        let dns_records = client.get_dns_records().unwrap_or_else(|_| panic!());

        // assert_eq!(dns_records["result"].len(), 2);
        _m.assert();
        assert_eq!(dns_records["result"][0]["id"], "record_id_1");
        assert_eq!(dns_records["result"][1]["id"], "record_id_2");
    }

    #[test]
    fn test_get_dns_records_should_return_other_than_200() {
        let mut ctx = TestContext::new();
        let client = CloudflareClient::new(
            ReqwestClient::new(),
            ctx.api_token.clone(),
            ctx.server.url(),
            ctx.zone_id.clone(),
        );

        let api_token = ctx.api_token.clone();
        let _m = mock_get_dns_records(&mut ctx)
            .with_header("Authorization", format!("Bearer {}", api_token).as_str())
            .with_status(500)
            .expect(1)
            .create();

        let err = client.get_dns_records().unwrap_err();

        match err {
            ClientError::StatusCodeError(status_code, _) => {
                assert_eq!(status_code, StatusCode::INTERNAL_SERVER_ERROR);
            }
            _ => panic!("Expected StatusCodeError"),
        }
        _m.assert();
    }

    #[test]
    fn test_get_dns_records_should_return_network_error() {
        let ctx = TestContext::new();
        let mut mock = MockHttpClient::new();
        mock.expect_get_with_bearer_token()
            .times(1)
            .returning(|_, _| Err(Box::new(MockNetworkError {})));

        let client = CloudflareClient::new(
            mock,
            ctx.api_token.clone(),
            ctx.server.url(),
            ctx.zone_id.clone(),
        );

        let err = client.get_dns_records().unwrap_err();

        match err {
            ClientError::RequestError(_) => {}
            _ => panic!("Expected RequestError"),
        }
    }

    // #[test]
    // fn test_find_record_ids() {
    //     let api_token = "your_api_token".to_string();
    //     let base_address = "http://localhost:8080".to_string();
    //     let zone_id = "your_zone_id".to_string();
    //     let client = CloudflareClient::new(api_token.clone(), base_address.clone(), zone_id.clone());

    //     let dns_records = json!({
    //         "result": [
    //             {
    //                 "id": "record_id_1",
    //                 "name": "example.com",
    //                 "content": "127.0.0.1"
    //             },
    //             {
    //                 "id": "record_id_2",
    //                 "name": "subdomain.example.com",
    //                 "content": "192.168.0.1"
    //             }
    //         ]
    //     });

    //     let record_name = "example.com";
    //     let record_ids = client.find_record_ids(dns_records.clone(), record_name).unwrap();

    //     assert_eq!(record_ids.len(), 1);
    //     assert_eq!(record_ids[0]["id"], "record_id_1");

    //     let record_name = "nonexistent.example.com";
    //     let record_ids = client.find_record_ids(dns_records.clone(), record_name).unwrap();

    //     assert_eq!(record_ids.len(), 0);
    // }

    // #[test]
    // fn test_update_record() {
    //     let api_token = "your_api_token".to_string();
    //     let base_address = mockito::server_url();
    //     let zone_id = "your_zone_id".to_string();
    //     let client = CloudflareClient::new(api_token.clone(), base_address.clone(), zone_id.clone());

    //     let record_id = "record_id_1";
    //     let record = json!({
    //         "name": "example.com",
    //         "content": "127.0.0.1"
    //     });

    //     let response_body = json!({
    //         "result": {
    //             "id": record_id,
    //             "name": "example.com",
    //             "content": "127.0.0.1"
    //         }
    //     });

    //     let _m = mockito::mock("PATCH", format!("/zones/{}/dns_records/{}", zone_id, record_id).as_str())
    //         .match_header("Authorization", Matcher::Exact(format!("Bearer {}", api_token).as_str()))
    //         .match_header("Content-Type", "application/json")
    //         .match_body(Matcher::Json(record.clone()))
    //         .with_status(200)
    //         .with_header("content-type", "application/json")
    //         .with_body(response_body.to_string())
    //         .create();

    //     let updated_record = client.update_record(&Value::String(record_id.to_string()), &record).unwrap();

    //     assert_eq!(updated_record["result"]["id"], record_id);
    //     assert_eq!(updated_record["result"]["name"], "example.com");
    //     assert_eq!(updated_record["result"]["content"], "127.0.0.1");
    // }
}
