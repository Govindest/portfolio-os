//! Strategy definitions as declarative YAML (schema_version 1). YAML is data,
//! never code — nothing here is executed. A matching JSON Schema ships in
//! docs/strategy.schema.json for external tooling.

use crate::errors::BrokerError;
use serde::{Deserialize, Serialize};

pub const WEIGHT_TOLERANCE: f64 = 0.005; // total weight must be within 0.5% of 100%

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StrategyDoc {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub assets: Vec<StrategyAsset>,
    #[serde(default)]
    pub rebalance: RebalanceConfig,
    #[serde(default)]
    pub risk_controls: RiskControls,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StrategyAsset {
    pub symbol: String,
    pub target_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RebalanceConfig {
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_drift")]
    pub drift_threshold_percent: f64,
    #[serde(default = "default_min_order")]
    pub minimum_order_value: f64,
    #[serde(default = "default_true")]
    pub allow_fractional_shares: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskControls {
    #[serde(default = "default_max_pos")]
    pub maximum_position_weight: f64,
    #[serde(default = "default_max_order")]
    pub maximum_order_value: f64,
    #[serde(default = "default_max_batch")]
    pub maximum_batch_value: f64,
}

fn default_method() -> String {
    "target_weight".into()
}
fn default_drift() -> f64 {
    2.0
}
fn default_min_order() -> f64 {
    5.0
}
fn default_true() -> bool {
    true
}
fn default_max_pos() -> f64 {
    0.35
}
fn default_max_order() -> f64 {
    1000.0
}
fn default_max_batch() -> f64 {
    5000.0
}

impl Default for RebalanceConfig {
    fn default() -> Self {
        Self {
            method: default_method(),
            drift_threshold_percent: default_drift(),
            minimum_order_value: default_min_order(),
            allow_fractional_shares: true,
        }
    }
}

impl Default for RiskControls {
    fn default() -> Self {
        Self {
            maximum_position_weight: default_max_pos(),
            maximum_order_value: default_max_order(),
            maximum_batch_value: default_max_batch(),
        }
    }
}

/// Parse and validate a YAML strategy document. Returns readable error text.
pub fn parse_yaml(yaml: &str) -> Result<StrategyDoc, BrokerError> {
    let mut doc: StrategyDoc = serde_yaml::from_str(yaml)
        .map_err(|e| BrokerError::validation(&format!("The YAML file could not be read: {e}")))?;
    validate(&mut doc)?;
    Ok(doc)
}

/// Normalize and validate a strategy document in place.
pub fn validate(doc: &mut StrategyDoc) -> Result<(), BrokerError> {
    if doc.schema_version != 1 {
        return Err(BrokerError::validation(&format!(
            "Unsupported schema_version {}. This version of PortfolioOS supports schema_version 1.",
            doc.schema_version
        )));
    }
    if doc.name.trim().is_empty() {
        return Err(BrokerError::validation("The strategy needs a name."));
    }
    if doc.assets.is_empty() {
        return Err(BrokerError::validation(
            "The strategy must contain at least one asset.",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for a in doc.assets.iter_mut() {
        a.symbol = a.symbol.trim().to_uppercase();
        if a.symbol.is_empty()
            || !a
                .symbol
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.')
        {
            return Err(BrokerError::validation(&format!(
                "\"{}\" is not a valid ticker symbol.",
                a.symbol
            )));
        }
        if !seen.insert(a.symbol.clone()) {
            return Err(BrokerError::validation(&format!(
                "{} appears more than once. Each symbol may only be listed once.",
                a.symbol
            )));
        }
        if a.target_weight <= 0.0 {
            return Err(BrokerError::validation(&format!(
                "The target weight for {} must be positive.",
                a.symbol
            )));
        }
        if a.target_weight > doc.risk_controls.maximum_position_weight + 1e-9 {
            return Err(BrokerError::validation(&format!(
                "{} has a target weight of {:.1}%, above the maximum position weight of {:.1}%.",
                a.symbol,
                a.target_weight * 100.0,
                doc.risk_controls.maximum_position_weight * 100.0
            )));
        }
    }
    let total: f64 = doc.assets.iter().map(|a| a.target_weight).sum();
    // Total may be below 100% — the remainder is a deliberate cash allocation —
    // but must never exceed 100%.
    if total > 1.0 + WEIGHT_TOLERANCE {
        return Err(BrokerError::validation(&format!(
            "Target weights add up to {:.1}%, which is more than 100%.",
            total * 100.0
        )));
    }
    if total < 0.5 {
        return Err(BrokerError::validation(&format!(
            "Target weights add up to only {:.1}%. Allocate at least 50% or add a larger cash note in the description.",
            total * 100.0
        )));
    }
    if doc.rebalance.method != "target_weight" {
        return Err(BrokerError::validation(&format!(
            "Unknown rebalance method \"{}\". Supported: target_weight.",
            doc.rebalance.method
        )));
    }
    if doc.rebalance.drift_threshold_percent < 0.0 || doc.rebalance.drift_threshold_percent > 50.0 {
        return Err(BrokerError::validation(
            "The drift threshold must be between 0% and 50%.",
        ));
    }
    if doc.rebalance.minimum_order_value < 0.0 {
        return Err(BrokerError::validation(
            "The minimum order value cannot be negative.",
        ));
    }
    Ok(())
}

pub fn to_yaml(doc: &StrategyDoc) -> Result<String, BrokerError> {
    serde_yaml::to_string(doc)
        .map_err(|e| BrokerError::validation(&format!("Could not serialize strategy: {e}")))
}

/// Build an equal-weight strategy from a symbol list.
pub fn equal_weight(
    name: &str,
    description: Option<String>,
    symbols: &[String],
) -> Result<StrategyDoc, BrokerError> {
    if symbols.is_empty() {
        return Err(BrokerError::validation("Pick at least one symbol."));
    }
    let w = 1.0 / symbols.len() as f64;
    let mut doc = StrategyDoc {
        schema_version: 1,
        name: name.to_string(),
        description,
        assets: symbols
            .iter()
            .map(|s| StrategyAsset {
                symbol: s.clone(),
                target_weight: w,
            })
            .collect(),
        rebalance: RebalanceConfig::default(),
        risk_controls: RiskControls {
            // Equal weight above the default cap is fine — cap adapts upward.
            maximum_position_weight: (w + 0.05).max(default_max_pos()),
            ..RiskControls::default()
        },
    };
    validate(&mut doc)?;
    Ok(doc)
}

pub const EXAMPLE_YAML: &str = r#"schema_version: 1
name: Equal Weight Technology
description: Equal allocation across selected technology companies

assets:
  - symbol: AAPL
    target_weight: 0.25
  - symbol: MSFT
    target_weight: 0.25
  - symbol: NVDA
    target_weight: 0.25
  - symbol: GOOGL
    target_weight: 0.25

rebalance:
  method: target_weight
  drift_threshold_percent: 2
  minimum_order_value: 5
  allow_fractional_shares: true

risk_controls:
  maximum_position_weight: 0.30
  maximum_order_value: 1000
  maximum_batch_value: 5000
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_yaml_parses() {
        let doc = parse_yaml(EXAMPLE_YAML).unwrap();
        assert_eq!(doc.assets.len(), 4);
        assert_eq!(doc.rebalance.drift_threshold_percent, 2.0);
    }

    #[test]
    fn rejects_duplicate_symbols() {
        let yaml = r#"
schema_version: 1
name: Dup
assets:
  - symbol: aapl
    target_weight: 0.3
  - symbol: AAPL
    target_weight: 0.3
"#;
        let err = parse_yaml(yaml).unwrap_err();
        assert!(err.user_message().contains("more than once"));
    }

    #[test]
    fn normalizes_symbols_to_uppercase() {
        let yaml = r#"
schema_version: 1
name: Case
assets:
  - symbol: " msft "
    target_weight: 0.6
  - symbol: brk.b
    target_weight: 0.35
risk_controls:
  maximum_position_weight: 0.60
"#;
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(doc.assets[0].symbol, "MSFT");
        assert_eq!(doc.assets[1].symbol, "BRK.B");
    }

    #[test]
    fn rejects_overweight_total() {
        let yaml = r#"
schema_version: 1
name: Over
assets:
  - symbol: A
    target_weight: 0.30
  - symbol: B
    target_weight: 0.30
  - symbol: C
    target_weight: 0.30
  - symbol: D
    target_weight: 0.30
"#;
        assert!(parse_yaml(yaml).is_err());
    }

    #[test]
    fn allows_cash_allocation_under_100() {
        let yaml = r#"
schema_version: 1
name: Cash Buffer
assets:
  - symbol: VOO
    target_weight: 0.30
  - symbol: BND
    target_weight: 0.30
  - symbol: VXUS
    target_weight: 0.30
"#;
        assert!(parse_yaml(yaml).is_ok());
    }

    #[test]
    fn rejects_negative_and_zero_weights() {
        for w in ["0", "-0.2"] {
            let yaml = format!(
                "schema_version: 1\nname: Bad\nassets:\n  - symbol: AAPL\n    target_weight: {w}\n"
            );
            assert!(parse_yaml(&yaml).is_err());
        }
    }

    #[test]
    fn rejects_position_above_max_weight() {
        let yaml = r#"
schema_version: 1
name: TooBig
assets:
  - symbol: AAPL
    target_weight: 0.50
risk_controls:
  maximum_position_weight: 0.30
"#;
        let err = parse_yaml(yaml).unwrap_err();
        assert!(err.user_message().contains("maximum position weight"));
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let yaml = "schema_version: 99\nname: X\nassets:\n  - symbol: AAPL\n    target_weight: 1\nrisk_controls:\n  maximum_position_weight: 1.0\n";
        assert!(parse_yaml(yaml).is_err());
    }

    #[test]
    fn equal_weight_builder() {
        let doc = equal_weight("EW", None, &["AAPL".into(), "MSFT".into(), "NVDA".into()]).unwrap();
        let total: f64 = doc.assets.iter().map(|a| a.target_weight).sum();
        assert!((total - 1.0).abs() < 1e-9);
    }

    #[test]
    fn roundtrip_yaml() {
        let doc = parse_yaml(EXAMPLE_YAML).unwrap();
        let yaml = to_yaml(&doc).unwrap();
        let doc2 = parse_yaml(&yaml).unwrap();
        assert_eq!(doc, doc2);
    }
}
