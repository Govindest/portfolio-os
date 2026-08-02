//! Rebalancing engine. Pure functions over positions + quotes + strategy so
//! every rule is unit-testable without a broker.

use crate::broker::{OrderSide, Position, ProposedOrder, Quote};
use crate::strategy::StrategyDoc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySettings {
    pub max_order_value: f64,
    pub max_batch_value: f64,
    pub max_position_weight: f64,
    pub min_order_value: f64,
    pub allow_fractional: bool,
    pub allow_sells: bool,
}

impl Default for SafetySettings {
    fn default() -> Self {
        Self {
            max_order_value: 1_000.0,
            max_batch_value: 5_000.0,
            max_position_weight: 0.35,
            min_order_value: 5.0,
            allow_fractional: true,
            allow_sells: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceInput {
    pub positions: Vec<Position>,
    pub quotes: Vec<Quote>,
    pub cash: f64,
    pub buying_power: f64,
    /// Symbols with open orders — skipped to avoid double-trading.
    pub open_order_symbols: Vec<String>,
    pub market_open: bool,
    pub broker_supports_fractional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalancePlan {
    pub orders: Vec<ProposedOrder>,
    pub skipped: Vec<SkippedAsset>,
    pub warnings: Vec<String>,
    pub total_buy_value: f64,
    pub total_sell_value: f64,
    pub estimated_cash_after: f64,
    pub portfolio_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedAsset {
    pub symbol: String,
    pub reason: String,
}

fn round_qty(qty: f64, fractional: bool) -> f64 {
    if fractional {
        // Brokers accept up to 4 decimal places for fractional quantities.
        (qty * 10_000.0).floor() / 10_000.0
    } else {
        qty.floor()
    }
}

/// Compute a rebalance plan. Deterministic: same inputs, same plan.
pub fn plan_rebalance(
    strategy: &StrategyDoc,
    input: &RebalanceInput,
    safety: &SafetySettings,
) -> RebalancePlan {
    let mut warnings = vec![];
    let mut skipped = vec![];
    let mut orders: Vec<ProposedOrder> = vec![];

    let price_of = |symbol: &str| -> Option<(f64, bool)> {
        input
            .quotes
            .iter()
            .find(|q| q.symbol == symbol && q.price > 0.0)
            .map(|q| (q.price, q.stale))
            .or_else(|| {
                input
                    .positions
                    .iter()
                    .find(|p| p.symbol == symbol && p.current_price > 0.0)
                    .map(|p| (p.current_price, true))
            })
    };

    let positions_value: f64 = input.positions.iter().map(|p| p.market_value).sum();
    let portfolio_value = positions_value + input.cash;
    if portfolio_value <= 0.0 {
        return RebalancePlan {
            orders: vec![],
            skipped,
            warnings: vec!["The account has no value to allocate.".into()],
            total_buy_value: 0.0,
            total_sell_value: 0.0,
            estimated_cash_after: input.cash,
            portfolio_value,
        };
    }

    if !input.market_open {
        warnings.push(
            "The market is currently closed. Orders would be queued until the next session and prices may move."
                .into(),
        );
    }

    let fractional_ok = safety.allow_fractional
        && strategy.rebalance.allow_fractional_shares
        && input.broker_supports_fractional;

    // Union of symbols: strategy targets + currently held positions not in the
    // strategy (drift back to zero if sells are allowed).
    let mut symbols: Vec<String> = strategy.assets.iter().map(|a| a.symbol.clone()).collect();
    for p in &input.positions {
        if !symbols.contains(&p.symbol) {
            symbols.push(p.symbol.clone());
        }
    }

    let drift_threshold = strategy.rebalance.drift_threshold_percent / 100.0;
    let min_order = strategy
        .rebalance
        .minimum_order_value
        .max(safety.min_order_value);

    for symbol in symbols {
        let target_weight = strategy
            .assets
            .iter()
            .find(|a| a.symbol == symbol)
            .map(|a| a.target_weight.min(safety.max_position_weight))
            .unwrap_or(0.0);
        let held = input.positions.iter().find(|p| p.symbol == symbol);
        let current_value = held.map(|p| p.market_value).unwrap_or(0.0);
        let current_weight = current_value / portfolio_value;
        let drift = current_weight - target_weight;

        if input.open_order_symbols.iter().any(|s| s == &symbol) {
            skipped.push(SkippedAsset {
                symbol,
                reason: "An open order already exists for this symbol.".into(),
            });
            continue;
        }

        if drift.abs() < drift_threshold {
            if target_weight > 0.0 {
                skipped.push(SkippedAsset {
                    symbol,
                    reason: format!(
                        "Within the {:.1}% drift threshold (current {:.1}%, target {:.1}%).",
                        drift_threshold * 100.0,
                        current_weight * 100.0,
                        target_weight * 100.0
                    ),
                });
            }
            continue;
        }

        let Some((price, stale)) = price_of(&symbol) else {
            skipped.push(SkippedAsset {
                symbol: symbol.clone(),
                reason: "No price is available for this symbol right now.".into(),
            });
            continue;
        };

        let target_value = target_weight * portfolio_value;
        let delta_value = target_value - current_value; // >0 buy, <0 sell
        let side = if delta_value > 0.0 {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };

        if side == OrderSide::Sell && !safety.allow_sells {
            skipped.push(SkippedAsset {
                symbol,
                reason: "Selling is disabled in your safety settings.".into(),
            });
            continue;
        }

        let mut order_warnings = vec![];
        if stale {
            order_warnings.push("The price for this order may be stale.".into());
        }

        let mut value = delta_value.abs().min(safety.max_order_value);
        if (delta_value.abs() - value).abs() > 1e-9 {
            order_warnings.push(format!(
                "Capped at your ${:.0} maximum order value; full correction needs ${:.2}.",
                safety.max_order_value,
                delta_value.abs()
            ));
        }

        // Selling an entire small residual position ignores the minimum.
        let closing_residual = side == OrderSide::Sell && target_weight == 0.0;
        if value < min_order && !closing_residual {
            skipped.push(SkippedAsset {
                symbol,
                reason: format!("The required trade (${value:.2}) is below the ${min_order:.2} minimum order value."),
            });
            continue;
        }

        let mut quantity = round_qty(value / price, fractional_ok);
        if side == OrderSide::Sell {
            let held_qty = held.map(|p| p.quantity).unwrap_or(0.0);
            if closing_residual || quantity > held_qty {
                quantity = if fractional_ok {
                    held_qty
                } else {
                    held_qty.floor()
                };
            }
        }
        if quantity <= 0.0 {
            skipped.push(SkippedAsset {
                symbol,
                reason: if fractional_ok {
                    "The computed quantity rounds to zero.".into()
                } else {
                    format!("Less than one whole share of ${price:.2} is needed and fractional shares are unavailable.")
                },
            });
            continue;
        }
        value = quantity * price;

        let reason = match side {
            OrderSide::Buy => format!(
                "Buy approximately ${:.2} of {} because its current allocation is {:.1}%, below the {:.1}% target by {:.1} percentage points.",
                value, symbol, current_weight * 100.0, target_weight * 100.0, (target_weight - current_weight) * 100.0
            ),
            OrderSide::Sell if target_weight == 0.0 => format!(
                "Sell the {} position (about ${:.2}) because it is not part of this strategy.",
                symbol, value
            ),
            OrderSide::Sell => format!(
                "Sell approximately ${:.2} of {} because its current allocation is {:.1}%, above the {:.1}% target by {:.1} percentage points.",
                value, symbol, current_weight * 100.0, target_weight * 100.0, (current_weight - target_weight) * 100.0
            ),
        };

        orders.push(ProposedOrder {
            symbol,
            side,
            quantity,
            estimated_value: value,
            current_weight,
            target_weight,
            drift,
            reason,
            warnings: order_warnings,
        });
    }

    // Sells first so their proceeds can fund the buys.
    orders.sort_by(|a, b| {
        let rank = |s: OrderSide| if s == OrderSide::Sell { 0 } else { 1 };
        rank(a.side).cmp(&rank(b.side)).then(
            b.estimated_value
                .partial_cmp(&a.estimated_value)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });

    // Batch cap counts every order (buys and sells). Cash constraint: buys can
    // only spend starting cash plus proceeds of sells kept earlier in the batch
    // (paper brokers settle instantly).
    let mut available = input.buying_power.max(input.cash);
    let mut batch_total = 0.0;
    orders.retain(|o| {
        if batch_total + o.estimated_value > safety.max_batch_value {
            skipped.push(SkippedAsset {
                symbol: o.symbol.clone(),
                reason: format!(
                    "Dropped to stay under your ${:.0} maximum batch value.",
                    safety.max_batch_value
                ),
            });
            return false;
        }
        match o.side {
            OrderSide::Sell => {
                available += o.estimated_value;
                batch_total += o.estimated_value;
                true
            }
            OrderSide::Buy => {
                if o.estimated_value > available {
                    skipped.push(SkippedAsset {
                        symbol: o.symbol.clone(),
                        reason: "Not enough cash remains for this buy after earlier orders.".into(),
                    });
                    return false;
                }
                available -= o.estimated_value;
                batch_total += o.estimated_value;
                true
            }
        }
    });

    let total_buy_value: f64 = orders
        .iter()
        .filter(|o| o.side == OrderSide::Buy)
        .map(|o| o.estimated_value)
        .sum();
    let total_sell_value: f64 = orders
        .iter()
        .filter(|o| o.side == OrderSide::Sell)
        .map(|o| o.estimated_value)
        .sum();

    if orders
        .iter()
        .any(|o| o.warnings.iter().any(|w| w.contains("stale")))
    {
        warnings.push("Some prices may be stale. Review estimated values carefully.".into());
    }

    RebalancePlan {
        estimated_cash_after: input.cash + total_sell_value - total_buy_value,
        total_buy_value,
        total_sell_value,
        orders,
        skipped,
        warnings,
        portfolio_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{parse_yaml, StrategyAsset};
    use chrono::Utc;

    fn quote(symbol: &str, price: f64) -> Quote {
        Quote {
            symbol: symbol.into(),
            price,
            as_of: Utc::now(),
            stale: false,
        }
    }

    fn position(symbol: &str, qty: f64, price: f64) -> Position {
        Position {
            symbol: symbol.into(),
            name: None,
            quantity: qty,
            current_price: price,
            market_value: qty * price,
            cost_basis: None,
            unrealized_pl: None,
        }
    }

    fn base_strategy() -> StrategyDoc {
        parse_yaml(crate::strategy::EXAMPLE_YAML).unwrap()
    }

    fn base_input() -> RebalanceInput {
        RebalanceInput {
            positions: vec![position("AAPL", 10.0, 200.0)], // $2000
            quotes: vec![
                quote("AAPL", 200.0),
                quote("MSFT", 400.0),
                quote("NVDA", 130.0),
                quote("GOOGL", 180.0),
            ],
            cash: 2_000.0,
            buying_power: 2_000.0,
            open_order_symbols: vec![],
            market_open: true,
            broker_supports_fractional: true,
        }
    }

    fn safety() -> SafetySettings {
        SafetySettings::default()
    }

    #[test]
    fn buys_underweight_assets_with_explanations() {
        let plan = plan_rebalance(&base_strategy(), &base_input(), &safety());
        // Portfolio $4000; targets 25% = $1000 each. AAPL at $2000 is overweight.
        let buys: Vec<_> = plan
            .orders
            .iter()
            .filter(|o| o.side == OrderSide::Buy)
            .collect();
        assert_eq!(buys.len(), 3, "MSFT, NVDA, GOOGL should be bought");
        for b in buys {
            assert!(b.reason.contains("below the"));
            assert!(b.estimated_value > 0.0);
        }
        let sell = plan.orders.iter().find(|o| o.symbol == "AAPL").unwrap();
        assert_eq!(sell.side, OrderSide::Sell);
        assert!((sell.estimated_value - 1000.0).abs() < 1.0);
    }

    #[test]
    fn respects_drift_threshold() {
        let mut input = base_input();
        // Perfectly balanced portfolio: 4 x $1000 + no cash.
        input.positions = vec![
            position("AAPL", 5.0, 200.0),
            position("MSFT", 2.5, 400.0),
            position("NVDA", 1000.0 / 130.0, 130.0),
            position("GOOGL", 1000.0 / 180.0, 180.0),
        ];
        input.cash = 0.0;
        input.buying_power = 0.0;
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        assert!(
            plan.orders.is_empty(),
            "no orders inside drift threshold: {:?}",
            plan.orders
        );
        assert_eq!(plan.skipped.len(), 4);
    }

    #[test]
    fn skips_symbols_with_open_orders() {
        let mut input = base_input();
        input.open_order_symbols = vec!["MSFT".into()];
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        assert!(plan.orders.iter().all(|o| o.symbol != "MSFT"));
        assert!(plan
            .skipped
            .iter()
            .any(|s| s.symbol == "MSFT" && s.reason.contains("open order")));
    }

    #[test]
    fn respects_max_order_value() {
        let mut s = safety();
        s.max_order_value = 300.0;
        let plan = plan_rebalance(&base_strategy(), &base_input(), &s);
        for o in &plan.orders {
            assert!(o.estimated_value <= 300.0 + 1e-6, "{o:?}");
        }
    }

    #[test]
    fn respects_max_batch_value() {
        let mut s = safety();
        s.max_batch_value = 500.0;
        let plan = plan_rebalance(&base_strategy(), &base_input(), &s);
        let buys: f64 = plan
            .orders
            .iter()
            .filter(|o| o.side == OrderSide::Buy)
            .map(|o| o.estimated_value)
            .sum();
        let sells: f64 = plan
            .orders
            .iter()
            .filter(|o| o.side == OrderSide::Sell)
            .map(|o| o.estimated_value)
            .sum();
        assert!(sells + buys <= 500.0 + 1e-6);
    }

    #[test]
    fn skips_orders_below_minimum_value() {
        let mut input = base_input();
        // $4,010 portfolio, AAPL nearly on target: tiny required trade.
        input.positions = vec![
            position("AAPL", 5.06, 200.0),
            position("MSFT", 2.5, 400.0),
            position("NVDA", 1000.0 / 130.0, 130.0),
            position("GOOGL", 1000.0 / 180.0, 180.0),
        ];
        input.cash = 0.0;
        let mut strategy = base_strategy();
        strategy.rebalance.drift_threshold_percent = 0.0;
        strategy.rebalance.minimum_order_value = 50.0;
        let plan = plan_rebalance(&strategy, &input, &safety());
        assert!(plan
            .orders
            .iter()
            .all(|o| o.estimated_value >= 50.0 || o.target_weight == 0.0));
    }

    #[test]
    fn whole_shares_when_fractional_unavailable() {
        let mut input = base_input();
        input.broker_supports_fractional = false;
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        for o in &plan.orders {
            assert_eq!(o.quantity.fract(), 0.0, "whole shares only: {o:?}");
        }
    }

    #[test]
    fn sells_disabled_produces_no_sells() {
        let mut s = safety();
        s.allow_sells = false;
        let plan = plan_rebalance(&base_strategy(), &base_input(), &s);
        assert!(plan.orders.iter().all(|o| o.side == OrderSide::Buy));
        assert!(plan
            .skipped
            .iter()
            .any(|k| k.reason.contains("Selling is disabled")));
    }

    #[test]
    fn sells_non_strategy_position_to_zero() {
        let mut input = base_input();
        input.positions.push(position("TSLA", 2.0, 250.0));
        input.quotes.push(quote("TSLA", 250.0));
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        let tsla = plan.orders.iter().find(|o| o.symbol == "TSLA").unwrap();
        assert_eq!(tsla.side, OrderSide::Sell);
        assert_eq!(tsla.quantity, 2.0);
        assert!(tsla.reason.contains("not part of this strategy"));
    }

    #[test]
    fn missing_quote_skips_symbol() {
        let mut input = base_input();
        input.quotes.retain(|q| q.symbol != "MSFT");
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        assert!(plan.orders.iter().all(|o| o.symbol != "MSFT"));
        assert!(plan
            .skipped
            .iter()
            .any(|s| s.symbol == "MSFT" && s.reason.contains("No price")));
    }

    #[test]
    fn stale_quote_warns() {
        let mut input = base_input();
        for q in input.quotes.iter_mut() {
            q.stale = true;
        }
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        assert!(plan.warnings.iter().any(|w| w.contains("stale")));
    }

    #[test]
    fn closed_market_warns() {
        let mut input = base_input();
        input.market_open = false;
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        assert!(plan.warnings.iter().any(|w| w.contains("closed")));
    }

    #[test]
    fn buys_never_exceed_available_cash() {
        let mut input = base_input();
        input.cash = 100.0;
        input.buying_power = 100.0;
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        let sells: f64 = plan
            .orders
            .iter()
            .filter(|o| o.side == OrderSide::Sell)
            .map(|o| o.estimated_value)
            .sum();
        let buys: f64 = plan
            .orders
            .iter()
            .filter(|o| o.side == OrderSide::Buy)
            .map(|o| o.estimated_value)
            .sum();
        assert!(buys <= 100.0 + sells + 1e-6);
    }

    #[test]
    fn max_position_weight_caps_target() {
        let mut strategy = base_strategy();
        strategy.assets = vec![
            StrategyAsset {
                symbol: "AAPL".into(),
                target_weight: 0.30,
            },
            StrategyAsset {
                symbol: "MSFT".into(),
                target_weight: 0.30,
            },
        ];
        strategy.risk_controls.maximum_position_weight = 0.30;
        let mut s = safety();
        s.max_position_weight = 0.10; // user safety tighter than the strategy
        let plan = plan_rebalance(&strategy, &base_input(), &s);
        for o in &plan.orders {
            assert!(o.target_weight <= 0.10 + 1e-9);
        }
    }

    #[test]
    fn deterministic_output() {
        let a = plan_rebalance(&base_strategy(), &base_input(), &safety());
        let b = plan_rebalance(&base_strategy(), &base_input(), &safety());
        assert_eq!(
            serde_json::to_string(&a.orders).unwrap(),
            serde_json::to_string(&b.orders).unwrap()
        );
    }

    #[test]
    fn empty_account_yields_no_orders() {
        let input = RebalanceInput {
            positions: vec![],
            quotes: vec![],
            cash: 0.0,
            buying_power: 0.0,
            open_order_symbols: vec![],
            market_open: true,
            broker_supports_fractional: true,
        };
        let plan = plan_rebalance(&base_strategy(), &input, &safety());
        assert!(plan.orders.is_empty());
    }
}
