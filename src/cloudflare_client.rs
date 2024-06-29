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

    pub fn get_dns_records(&self) -> Result<HashMap<String, Value>, ClientError> {
        let url = format!("{}/zones/{}/dns_records", self.base_address, self.zone_id);
        let response = self
            .client
            .get_with_bearer_token(&url, self.api_token.as_str());
        handle_response(response)
    }

    pub fn update_record(
        &self,
        record_id: &Value,
        record: &Value,
        name: &str,
        r#type: &str,
    ) -> Result<HashMap<String, Value>, ClientError> {
        let url = format!(
            "{}/zones/{}/dns_records/{}",
            self.base_address,
            self.zone_id,
            record_id.as_str().unwrap()
        );
        let req_body = json!({"content": record, "name": name, "type": r#type});
        let response =
            self.client
                .patch_with_bearer_token(&url, self.api_token.as_str(), &req_body);
        handle_response(response)
    }
}

pub fn filter_by_record_name(
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

pub fn filter_by_record_type(
    dns_records: &[Value],
    record_type: &str,
) -> Result<Value, ClientError> {
    let record = dns_records
        .iter()
        .filter(|record| record["type"].as_str().unwrap() == record_type)
        .cloned()
        .collect::<Vec<Value>>();

    if record.is_empty() {
        Err(ClientError::RecordNotFound)
    } else if record.len() > 1 {
        Err(ClientError::MultipleRecordsFound(record))
    } else {
        Ok(record[0].clone())
    }
}

fn handle_response(
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
            other => Err(ClientError::StatusCodeError(
                other,
                response.text().unwrap(),
            )),
        },
        Err(error) => Err(ClientError::NetworkError(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::MockHttpClient;
    use test_context::{test_context, TestContext};

    struct MyContext {
        api_token: String,
        server: String,
        zone_id: String,
    }

    impl TestContext for MyContext {
        fn setup() -> MyContext {
            MyContext {
                api_token: "API_TOKEN".to_string(),
                server: "https://CLOUDFLARE".to_string(),
                zone_id: "ZONE_ID".to_string(),
            }
        }
    }

    mod get_dns_records {
        use super::*;

        #[test_context(MyContext)]
        #[test]
        fn test_return_200(ctx: &MyContext) {
            let mut mock = MockHttpClient::new();

            let expected_body = json!({
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
            })
            .to_string();

            let cloned_body = expected_body.clone();
            mock.expect_get_with_bearer_token()
                .times(1)
                .return_once(|_, _| {
                    Ok(reqwest::blocking::Response::from(
                        http::Response::builder()
                            .status(200)
                            .body(cloned_body)
                            .unwrap(),
                    ))
                });

            let client = CloudflareClient::new(
                mock,
                ctx.api_token.clone(),
                ctx.server.clone(),
                ctx.zone_id.clone(),
            );
            let actual_body = client.get_dns_records().unwrap();
            assert_eq!(actual_body, serde_json::from_str(&expected_body).unwrap());
        }

        #[test_context(MyContext)]
        #[test]
        fn test_return_other_than_200(ctx: &MyContext) {
            let mut mock = MockHttpClient::new();

            mock.expect_get_with_bearer_token()
                .times(1)
                .return_once(|_, _| {
                    Ok(reqwest::blocking::Response::from(
                        http::Response::builder()
                            .status(500)
                            .body("INTERNAL SERVER ERROR")
                            .unwrap(),
                    ))
                });

            let client = CloudflareClient::new(
                mock,
                ctx.api_token.clone(),
                ctx.server.clone(),
                ctx.zone_id.clone(),
            );
            let err = client.get_dns_records().unwrap_err();

            match err {
                ClientError::StatusCodeError(status_code, _) => {
                    assert_eq!(status_code, StatusCode::INTERNAL_SERVER_ERROR);
                }
                _ => panic!("Expected StatusCodeError"),
            }
        }

        #[test_context(MyContext)]
        #[test]
        fn test_return_network_error(ctx: &MyContext) {
            let mut mock = MockHttpClient::new();

            mock.expect_get_with_bearer_token()
                .times(1)
                .returning(|_, _| {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "OTHER",
                    )))
                });

            let client = CloudflareClient::new(
                mock,
                ctx.api_token.clone(),
                ctx.server.clone(),
                ctx.zone_id.clone(),
            );
            let err = client.get_dns_records().unwrap_err();

            match err {
                ClientError::NetworkError(_) => {}
                _ => panic!("Expected NetworkError"),
            }
        }
    }

    mod update_record {
        use super::*;

        #[test_context(MyContext)]
        #[test]
        fn test_return_200(ctx: &MyContext) {
            let mut mock = MockHttpClient::new();

            let expected_body = json!({
                "result": {
                    "id": "record_id_1",
                    "name": "example.com",
                    "content": "127.0.0.1"
                },
            })
            .to_string();

            let cloned_body = expected_body.clone();
            mock.expect_patch_with_bearer_token()
                .times(1)
                .return_once(|_, _, _| {
                    Ok(reqwest::blocking::Response::from(
                        http::Response::builder()
                            .status(200)
                            .body(cloned_body)
                            .unwrap(),
                    ))
                });

            let client = CloudflareClient::new(
                mock,
                ctx.api_token.clone(),
                ctx.server.clone(),
                ctx.zone_id.clone(),
            );
            let actual_body = client
                .update_record(
                    &Value::String("RECORD_ID".to_string()),
                    &Value::String("RECORD".to_string()),
                    "NAME",
                    "RTYPE",
                )
                .unwrap();
            assert_eq!(actual_body, serde_json::from_str(&expected_body).unwrap());
        }

        #[test_context(MyContext)]
        #[test]
        fn test_return_other_than_200(ctx: &MyContext) {
            let mut mock = MockHttpClient::new();

            mock.expect_patch_with_bearer_token()
                .times(1)
                .return_once(|_, _, _| {
                    Ok(reqwest::blocking::Response::from(
                        http::Response::builder()
                            .status(500)
                            .body("INTERNAL SERVER ERROR")
                            .unwrap(),
                    ))
                });

            let client = CloudflareClient::new(
                mock,
                ctx.api_token.clone(),
                ctx.server.clone(),
                ctx.zone_id.clone(),
            );
            let err = client
                .update_record(
                    &Value::String("RECORD_ID".to_string()),
                    &Value::String("RECORD".to_string()),
                    "NAME",
                    "RTYPE",
                )
                .unwrap_err();

            match err {
                ClientError::StatusCodeError(status_code, _) => {
                    assert_eq!(status_code, StatusCode::INTERNAL_SERVER_ERROR);
                }
                _ => panic!("Expected StatusCodeError"),
            }
        }

        #[test_context(MyContext)]
        #[test]
        fn test_return_network_error(ctx: &MyContext) {
            let mut mock = MockHttpClient::new();

            mock.expect_patch_with_bearer_token()
                .times(1)
                .returning(|_, _, _| {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "OTHER",
                    )))
                });

            let client = CloudflareClient::new(
                mock,
                ctx.api_token.clone(),
                ctx.server.clone(),
                ctx.zone_id.clone(),
            );
            let err = client
                .update_record(
                    &Value::String("RECORD_ID".to_string()),
                    &Value::String("RECORD".to_string()),
                    "NAME",
                    "RTYPE",
                )
                .unwrap_err();

            match err {
                ClientError::NetworkError(_) => {}
                _ => panic!("Expected NetworkError"),
            }
        }
    }

    #[test]
    fn test_filter_by_record_name_should_return_record_ids() {
        let mut dns_records = HashMap::new();
        dns_records.insert(
            "result".to_string(),
            json!(
                [
                    {
                        "id": "record_id_1",
                        "name": "example.com",
                        "content": "127.0.0.1"
                    },
                    {
                        "id": "record_id_2",
                        "name": "subdomain.example.com",
                        "content": "192.168.0.1"
                    },
                    {
                        "id": "record_id_3",
                        "name": "example.com",
                        "content": "192.168.0.2"
                    }
                ]
            ),
        );

        let record_ids = filter_by_record_name(dns_records, "example.com");
        match record_ids {
            Err(_) => panic!("unexpected err returned"),
            Ok(record_ids) => {
                assert_eq!(record_ids.len(), 2);
                assert_eq!(record_ids[0]["id"], "record_id_1");
                assert_eq!(record_ids[0]["name"], "example.com");
                assert_eq!(record_ids[1]["id"], "record_id_3");
                assert_eq!(record_ids[1]["name"], "example.com");
            }
        }
    }

    #[test]
    fn test_filter_by_record_name_should_return_err() {
        let mut dns_records = HashMap::new();
        dns_records.insert(
            "result".to_string(),
            json!(
                [
                    {
                        "id": "record_id_1",
                        "name": "example.com",
                        "content": "127.0.0.1"
                    },
                    {
                        "id": "record_id_2",
                        "name": "subdomain.example.com",
                        "content": "192.168.0.1"
                    },
                    {
                        "id": "record_id_3",
                        "name": "example.com",
                        "content": "192.168.0.2"
                    }
                ]
            ),
        );

        let record_ids = filter_by_record_name(dns_records, "foo.com");
        match record_ids {
            Err(ClientError::RecordNotFound) => (),
            Err(_) => panic!("unexpected err returned"),
            Ok(record_ids) => {
                panic!("unexpected record_ids returned: {:?}", record_ids);
            }
        }
    }

    #[test]
    fn test_filter_by_record_type_should_return_record_id() {
        let dns_records = vec![
            json!({
                "id": "record_id_1",
                "name": "example.com",
                "content": "127.0.0.1",
                "type": "A"
            }),
            json!({
                "id": "record_id_3",
                "name": "example.com",
                "content": "2001:db8::1",
                "type": "AAAA"
            }),
        ];

        let record_ids = filter_by_record_type(&dns_records, "A");
        match record_ids {
            Err(_) => panic!("unexpected err returned"),
            Ok(record_ids) => {
                assert_eq!(record_ids["id"], "record_id_1");
                assert_eq!(record_ids["type"], "A");
            }
        }
    }

    #[test]
    fn test_filter_by_record_type_should_return_multiple_records_found_error() {
        let dns_records = vec![
            json!({
                "id": "record_id_1",
                "name": "example.com",
                "content": "127.0.0.1",
                "type": "A"
            }),
            json!({
                "id": "record_id_3",
                "name": "example.com",
                "content": "2001:db8::1",
                "type": "AAAA"
            }),
            json!({
                "id": "record_id_4",
                "name": "example.com",
                "content": "192.168.0.1",
                "type": "A"
            }),
        ];

        let record = filter_by_record_type(&dns_records, "A");
        match record {
            Err(ClientError::MultipleRecordsFound(_)) => (),
            Err(_) => panic!("unexpected err returned"),
            Ok(record_ids) => {
                panic!("unexpected record_ids returned: {:?}", record_ids);
            }
        }
    }

    #[test]
    fn test_filter_by_record_type_should_return_record_not_found_error() {
        let dns_records = vec![
            json!({
                "id": "record_id_1",
                "name": "example.com",
                "content": "127.0.0.1",
                "type": "A"
            }),
            json!({
                "id": "record_id_3",
                "name": "example.com",
                "content": "2001:db8::1",
                "type": "AAAA"
            }),
            json!({
                "id": "record_id_4",
                "name": "example.com",
                "content": "192.168.0.1",
                "type": "A"
            }),
        ];

        let record = filter_by_record_type(&dns_records, "CNAME");
        match record {
            Err(ClientError::RecordNotFound) => (),
            Err(_) => panic!("unexpected err returned"),
            Ok(record_ids) => {
                panic!("unexpected record_ids returned: {:?}", record_ids);
            }
        }
    }
}
