use axum::headers::{self, Header, HeaderName, HeaderValue};
use lazy_static::lazy_static;

pub const X_FILE_SIZE: XFileNameHeaderName = XFileNameHeaderName {};

lazy_static! {
    static ref INTERNAL_TEXT: &'static [u8] = "x-file-size".as_bytes();
    static ref INTERNAL_NAME: HeaderName = HeaderName::from_lowercase(&INTERNAL_TEXT).unwrap();
}

pub struct XFileNameHeaderName;

impl From<XFileNameHeaderName> for HeaderName {
    fn from(_: XFileNameHeaderName) -> Self {
        INTERNAL_NAME.clone()
    }
}

pub struct XFileSize(pub u64);

impl Header for XFileSize {
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
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(headers::Error::invalid)?;
        Ok(Self(value))
    }

    fn encode<E>(&self, values: &mut E)
    where
        E: Extend<HeaderValue>,
    {
        let value = HeaderValue::from_str(&self.0.to_string()).unwrap();
        values.extend(std::iter::once(value));
    }
}