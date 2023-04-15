use axum::headers;

use axum::headers::{Header, HeaderName, HeaderValue};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref X_FILE_NAME: HeaderName = HeaderName::from_lowercase(b"x-file-name").unwrap();
}

pub struct XFileName(pub String);

impl Header for XFileName {
    fn name() -> &'static HeaderName {
        &X_FILE_NAME
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        I: Iterator<Item = &'i HeaderValue>,
    {
        let value = values
            .next()
            .and_then(|value| value.to_str().ok())
            .ok_or_else(headers::Error::invalid)?;

        Ok(XFileName(value.to_string()))
    }

    fn encode<E>(&self, values: &mut E)
    where
        E: Extend<HeaderValue>,
    {
        let value = HeaderValue::from_str(&self.0).unwrap();

        values.extend(std::iter::once(value));
    }
}