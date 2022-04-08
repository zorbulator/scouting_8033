use actix_web::{http::header::{TryIntoHeaderValue, InvalidHeaderValue, HeaderValue}, web, dev::ServiceRequest, Result as HttpResult, body::MessageBody};
use actix_web_httpauth::{headers::www_authenticate::Challenge, extractors::{basic::BasicAuth, AuthenticationError}};

// have to do all of this to make a response for if basic authentication fails
#[derive(Clone, Debug)]
struct BasicChallenge();

impl std::fmt::Display for BasicChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Basic")
    }
}

impl TryIntoHeaderValue for BasicChallenge {
    type Error = InvalidHeaderValue;
    fn try_into_value(self) -> Result<actix_web::http::header::HeaderValue, Self::Error> {
        HeaderValue::from_bytes(b"Basic")
    }
}

impl Challenge for BasicChallenge {
    fn to_bytes(&self) -> web::Bytes {
        "Basic".try_into_bytes().unwrap()
    }
}

// simple HTTP basic auth password check to protect the website
pub async fn check_password(req: ServiceRequest, credentials: BasicAuth) -> HttpResult<ServiceRequest> {
    match credentials.password() {
        Some(pass) if pass == "goselkie" => Ok(req),
        _ => Err(AuthenticationError::new(BasicChallenge()).into())
    }
}
