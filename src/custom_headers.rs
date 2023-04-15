use axum::headers::{self, Header, HeaderName, HeaderValue};
use lazy_static::lazy_static;

pub const X_FILE_NAME: XFileNameHeaderName = XFileNameHeaderName {};

lazy_static! {
    static ref INTERNAL_TEXT: &'static [u8] = "x-file-name".as_bytes();
    static ref INTERNAL_NAME: HeaderName = HeaderName::from_lowercase(&INTERNAL_TEXT).unwrap();
}

pub struct XFileNameHeaderName;

impl Into<HeaderName> for XFileNameHeaderName {
    fn into(self) -> HeaderName {
        INTERNAL_NAME.clone()
    }
}

pub struct XFileName(pub String);

impl Header for XFileName {
    fn name() -> &'static HeaderName {
        &INTERNAL_NAME
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