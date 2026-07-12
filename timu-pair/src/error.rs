use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadError {
    Invalid,
    Expired,
    UnsupportedVersion,
}

impl fmt::Display for PayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => formatter.write_str("invalid pairing payload"),
            Self::Expired => formatter.write_str("pairing payload has expired"),
            Self::UnsupportedVersion => formatter.write_str("unsupported pairing payload version"),
        }
    }
}

impl std::error::Error for PayloadError {}
