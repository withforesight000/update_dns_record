use std::collections::HashMap;

use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::errors::ClientError;

use crate::http_client::HttpClient;
pub trait DnsClient {
    fn get_dns_records(&self) -> Result<HashMap<String, Value>, ClientError>;
    fn update_record(
        &self,
        record_id: &Value,
        record: &Value,
        name: &str,
        record_type: &str,
    ) -> Result<HashMap<String, Value>, ClientError>;
}

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
        let mut body = handle_response(response)?;
        let total_pages = body
            .get("result_info")
            .and_then(|info| info.get("total_pages"))
            .and_then(Value::as_u64)
            .unwrap_or(1);
        if total_pages > 1 {
            let mut records = body
                .get("result")
                .and_then(Value::as_array)
                .cloned()
                .ok_or(ClientError::InvalidResponse("DNS result is not an array"))?;
            for page in 2..=total_pages {
                let response = self
                    .client
                    .get_with_bearer_token(&format!("{url}?page={page}"), self.api_token.as_str());
                let next = handle_response(response)?;
                let page_records = next
                    .get("result")
                    .and_then(Value::as_array)
                    .ok_or(ClientError::InvalidResponse("DNS result is not an array"))?;
                records.extend(page_records.iter().cloned());
            }
            body.insert("result".into(), Value::Array(records));
        }
        Ok(body)
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

impl<T: HttpClient> DnsClient for CloudflareClient<T> {
    fn get_dns_records(&self) -> Result<HashMap<String, Value>, ClientError> {
        CloudflareClient::get_dns_records(self)
    }

    fn update_record(
        &self,
        record_id: &Value,
        record: &Value,
        name: &str,
        record_type: &str,
    ) -> Result<HashMap<String, Value>, ClientError> {
        CloudflareClient::update_record(self, record_id, record, name, record_type)
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
mod contract_tests {
    use super::*;
    use crate::http_client::MockHttpClient;
    use mockall::predicate::eq;

    fn response(status: u16, body: &str) -> reqwest::blocking::Response {
        http::Response::builder()
            .status(status)
            .body(body.to_owned())
            .unwrap()
            .into()
    }

    #[test]
    fn requests_preserve_url_token_and_patch_body() {
        let mut http = MockHttpClient::new();
        http.expect_get_with_bearer_token()
            .with(
                eq("https://api.example/zones/zone/dns_records"),
                eq("token"),
            )
            .times(1)
            .return_once(|_, _| Ok(response(200, r#"{"result":[]}"#)));
        http.expect_patch_with_bearer_token()
            .with(
                eq("https://api.example/zones/zone/dns_records/id"),
                eq("token"),
                eq(json!({"content":"8.8.8.8","name":"example.com","type":"A"})),
            )
            .times(1)
            .return_once(|_, _, _| Ok(response(200, r#"{"result":{}}"#)));
        let client = CloudflareClient::new(
            http,
            "token".into(),
            "https://api.example".into(),
            "zone".into(),
        );
        client.get_dns_records().unwrap();
        client
            .update_record(&json!("id"), &json!("8.8.8.8"), "example.com", "A")
            .unwrap();
    }

    #[test]
    fn pagination_finds_later_records_and_detects_duplicates_across_pages() {
        let mut http = MockHttpClient::new();
        let mut sequence = mockall::Sequence::new();
        for (url, body) in [
            (
                "https://api.example/zones/zone/dns_records",
                json!({
                    "result":[{"id":"a1","name":"example.com","type":"A"}],
                    "result_info":{"total_pages":3}
                }),
            ),
            (
                "https://api.example/zones/zone/dns_records?page=2",
                json!({
                    "result":[{"id":"v6","name":"example.com","type":"AAAA"}]
                }),
            ),
            (
                "https://api.example/zones/zone/dns_records?page=3",
                json!({
                    "result":[{"id":"a2","name":"example.com","type":"A"}]
                }),
            ),
        ] {
            http.expect_get_with_bearer_token()
                .with(eq(url), eq("token"))
                .times(1)
                .in_sequence(&mut sequence)
                .return_once(move |_, _| Ok(response(200, &body.to_string())));
        }
        let client = CloudflareClient::new(
            http,
            "token".into(),
            "https://api.example".into(),
            "zone".into(),
        );
        let records =
            filter_by_record_name(client.get_dns_records().unwrap(), "example.com").unwrap();
        assert_eq!(filter_by_record_type(&records, "AAAA").unwrap()["id"], "v6");
        assert!(
            matches!(filter_by_record_type(&records, "A"), Err(ClientError::MultipleRecordsFound(records)) if records.len() == 2)
        );
    }

    #[test]
    fn pagination_failure_never_returns_partial_records() {
        for (status, body) in [
            (500, "failed"),
            (200, "invalid json"),
            (200, r#"{"result":null}"#),
        ] {
            let mut http = MockHttpClient::new();
            let mut sequence = mockall::Sequence::new();
            http.expect_get_with_bearer_token()
                .times(1)
                .in_sequence(&mut sequence)
                .return_once(|_, _| {
                    Ok(response(
                        200,
                        r#"{"result":[],"result_info":{"total_pages":2}}"#,
                    ))
                });
            http.expect_get_with_bearer_token()
                .with(eq("base/zones/zone/dns_records?page=2"), eq("token"))
                .times(1)
                .in_sequence(&mut sequence)
                .return_once(move |_, _| Ok(response(status, body)));
            let client = CloudflareClient::new(http, "token".into(), "base".into(), "zone".into());
            assert!(client.get_dns_records().is_err());
        }
    }

    #[test]
    fn invalid_json_is_a_body_error_for_get_and_patch() {
        for patch in [false, true] {
            let mut http = MockHttpClient::new();
            if patch {
                http.expect_patch_with_bearer_token()
                    .times(1)
                    .return_once(|_, _, _| Ok(response(200, "invalid")));
            } else {
                http.expect_get_with_bearer_token()
                    .times(1)
                    .return_once(|_, _| Ok(response(200, "invalid")));
            }
            let client = CloudflareClient::new(
                http,
                "token".into(),
                "https://api.example".into(),
                "zone".into(),
            );
            let result = if patch {
                client.update_record(&json!("id"), &json!("ip"), "name", "A")
            } else {
                client.get_dns_records()
            };
            assert!(matches!(result, Err(ClientError::BodyError(_))));
        }
    }

    #[test]
    fn only_200_is_success_and_error_body_is_preserved() {
        for status in [201, 204, 401, 403, 429, 500] {
            let error = handle_response(Ok(response(status, "failure body"))).unwrap_err();
            match error {
                ClientError::StatusCodeError(code, body) => {
                    assert_eq!(code.as_u16(), status);
                    assert_eq!(body, "failure body");
                }
                _ => panic!("expected status error"),
            }
        }
    }

    #[test]
    fn name_filter_is_exact_and_keeps_order_and_both_types() {
        let records = serde_json::from_value(json!({"result":[
            {"id":"first","name":"example.com","type":"AAAA"},
            {"id":"other","name":"Example.com","type":"A"},
            {"id":"last","name":"example.com","type":"A"}
        ]}))
        .unwrap();
        let matches = filter_by_record_name(records, "example.com").unwrap();
        assert_eq!(
            matches
                .iter()
                .map(|r| r["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["first", "last"]
        );
        assert_eq!(
            filter_by_record_type(&matches, "AAAA").unwrap()["id"],
            "first"
        );
    }

    #[test]
    fn empty_or_non_array_result_is_not_found() {
        for result in [json!([]), json!(null), json!({})] {
            assert!(matches!(
                filter_by_record_name(
                    serde_json::from_value(json!({"result":result})).unwrap(),
                    "name"
                ),
                Err(ClientError::RecordNotFound)
            ));
        }
        assert!(matches!(
            filter_by_record_type(&[], "A"),
            Err(ClientError::RecordNotFound)
        ));
    }

    #[test]
    fn duplicate_type_error_preserves_matching_records() {
        let records = vec![
            json!({"id":"one","type":"A"}),
            json!({"id":"skip","type":"AAAA"}),
            json!({"id":"two","type":"A"}),
        ];
        match filter_by_record_type(&records, "A").unwrap_err() {
            ClientError::MultipleRecordsFound(found) => {
                assert_eq!(found, vec![records[0].clone(), records[2].clone()])
            }
            _ => panic!("expected duplicate error"),
        }
    }

    #[test]
    #[should_panic]
    fn missing_result_keeps_existing_panic() {
        let _ = filter_by_record_name(HashMap::new(), "name");
    }

    #[test]
    #[should_panic]
    fn non_string_record_id_keeps_existing_panic() {
        let client = CloudflareClient::new(
            MockHttpClient::new(),
            "token".into(),
            "base".into(),
            "zone".into(),
        );
        let _ = client.update_record(&json!(42), &json!("ip"), "name", "A");
    }

    #[test]
    fn error_messages_are_preserved() {
        assert_eq!(
            ClientError::RecordNotFound.to_string(),
            "Failed to find record from response"
        );
        assert_eq!(
            ClientError::MultipleRecordsFound(vec![]).to_string(),
            "Failed to filter to one record: []"
        );
        assert_eq!(
            ClientError::NetworkError(Box::new(std::io::Error::other("offline"))).to_string(),
            "Failed to make request to Cloudflare: offline"
        );
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
                .returning(|_, _| Err(Box::new(std::io::Error::other("OTHER"))));

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
                .returning(|_, _, _| Err(Box::new(std::io::Error::other("OTHER"))));

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
