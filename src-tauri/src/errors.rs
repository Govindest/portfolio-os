use serde::Serialize;

/// Typed error categories surfaced to the UI. Each variant carries a
/// user-friendly message; technical detail travels separately so the UI can
/// show it in an expandable section without confusing ordinary users.
#[derive(Debug, thiserror::Error, Serialize, Clone)]
#[serde(tag = "category", content = "detail")]
pub enum BrokerError {
    #[error("Authentication failed")]
    Authentication(ErrorDetail),
    #[error("Not authorized")]
    Authorization(ErrorDetail),
    #[error("Network problem")]
    Network(ErrorDetail),
    #[error("Rate limited")]
    RateLimit(ErrorDetail),
    #[error("Invalid request")]
    InvalidRequest(ErrorDetail),
    #[error("Brokerage unavailable")]
    BrokerageUnavailable(ErrorDetail),
    #[error("Market closed")]
    MarketClosed(ErrorDetail),
    #[error("Insufficient buying power")]
    InsufficientBuyingPower(ErrorDetail),
    #[error("Unsupported asset")]
    UnsupportedAsset(ErrorDetail),
    #[error("Validation failed")]
    Validation(ErrorDetail),
    #[error("Storage failure")]
    Storage(ErrorDetail),
    #[error("Credential store failure")]
    CredentialStore(ErrorDetail),
    #[error("Unknown broker response")]
    UnknownResponse(ErrorDetail),
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct ErrorDetail {
    /// Message safe to show any user.
    pub user_message: String,
    /// Optional technical detail for the expandable section. Never contains secrets.
    pub technical: Option<String>,
}

impl BrokerError {
    pub fn detail(user_message: &str, technical: impl Into<Option<String>>) -> ErrorDetail {
        ErrorDetail {
            user_message: user_message.to_string(),
            technical: technical.into(),
        }
    }

    pub fn auth(user: &str, tech: Option<String>) -> Self {
        Self::Authentication(Self::detail(user, tech))
    }
    pub fn network(user: &str, tech: Option<String>) -> Self {
        Self::Network(Self::detail(user, tech))
    }
    pub fn validation(user: &str) -> Self {
        Self::Validation(Self::detail(user, None))
    }
    pub fn storage(tech: String) -> Self {
        Self::Storage(ErrorDetail {
            user_message: "PortfolioOS could not read or write local data.".into(),
            technical: Some(tech),
        })
    }
    pub fn credential_store(tech: String) -> Self {
        Self::CredentialStore(ErrorDetail {
            user_message:
                "PortfolioOS could not access the system credential store. Your credentials were not changed."
                    .into(),
            technical: Some(tech),
        })
    }

    pub fn user_message(&self) -> String {
        match self {
            BrokerError::Authentication(d)
            | BrokerError::Authorization(d)
            | BrokerError::Network(d)
            | BrokerError::RateLimit(d)
            | BrokerError::InvalidRequest(d)
            | BrokerError::BrokerageUnavailable(d)
            | BrokerError::MarketClosed(d)
            | BrokerError::InsufficientBuyingPower(d)
            | BrokerError::UnsupportedAsset(d)
            | BrokerError::Validation(d)
            | BrokerError::Storage(d)
            | BrokerError::CredentialStore(d)
            | BrokerError::UnknownResponse(d) => d.user_message.clone(),
        }
    }
}

impl From<rusqlite::Error> for BrokerError {
    fn from(e: rusqlite::Error) -> Self {
        BrokerError::storage(e.to_string())
    }
}

impl From<reqwest::Error> for BrokerError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() {
            BrokerError::Network(ErrorDetail {
                user_message: "PortfolioOS could not reach the brokerage service. Check your internet connection.".into(),
                technical: Some(redact(&e.to_string())),
            })
        } else {
            BrokerError::UnknownResponse(ErrorDetail {
                user_message: "The brokerage returned an unexpected response.".into(),
                technical: Some(redact(&e.to_string())),
            })
        }
    }
}

/// Strip query strings from URLs embedded in error text so tokens never leak into logs.
fn redact(s: &str) -> String {
    s.split('?').next().unwrap_or(s).to_string()
}
