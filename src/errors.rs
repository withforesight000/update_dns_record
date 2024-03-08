use std::fmt;

pub enum ClientError {
    RequestError(Box<dyn std::error::Error>),
    // ResponseError(reqwest::Error),
    StatusCodeError(reqwest::StatusCode),
    BodyError(reqwest::Error),
    RecordNotFound
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
            ClientError::StatusCodeError(status_code) => {
                write!(f, "Failed to get 200 OK from Cloudflare: {}", status_code)
            }
            ClientError::BodyError(error) => {
                write!(f, "Failed to get body from Cloudflare: {}", error)
            }
            ClientError::RecordNotFound => {
                write!(f, "Failed to find record from response")
            }
        }
    }
}

