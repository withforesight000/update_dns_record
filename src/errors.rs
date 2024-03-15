use std::fmt;

pub enum ClientError {
    RequestError(Box<dyn std::error::Error>),
    // ResponseError(reqwest::Error),
    StatusCodeError(reqwest::StatusCode, String),
    BodyError(reqwest::Error),
    RecordNotFound,
    MultipleRecordsFound(Vec<serde_json::Value>)
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClientError::RequestError(error) => {
                write!(f, "Failed to make request to Cloudflare: {}", *error)
            }
            // ClientError::ResponseError(error) => {
            //     write!(f, "Failed to get response from Cloudflare: {}", error)
            // }
            ClientError::StatusCodeError(status_code, resp_body) => {
                write!(f, "Failed to fetch 200 OK from Cloudflare: {} {}", status_code, resp_body)
            }
            ClientError::BodyError(error) => {
                write!(f, "Failed to fetch body from Cloudflare: {}", error)
            }
            ClientError::RecordNotFound => {
                write!(f, "Failed to find record from response")
            }
            ClientError::MultipleRecordsFound(record) => {
                write!(f, "Failed to filter to one record: {:?}", record)
            }
        }
    }
}

