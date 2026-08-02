# Strategy schema (version 1)

Strategies are declarative YAML documents. They are **data, never code** — importing a strategy never executes anything. The formal JSON Schema lives at [docs/strategy.schema.json](docs/strategy.schema.json).

## Example

```yaml
schema_version: 1
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
```

## Fields

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | Must be `1`. |
| `name` | string | yes | Display name. |
| `description` | string | no | Free text. |
| `assets[].symbol` | string | yes | Ticker; trimmed and uppercased on import; letters/digits/dots. |
| `assets[].target_weight` | number | yes | Fraction of portfolio value, `0 < w ≤ 1`. |
| `rebalance.method` | string | no | Only `target_weight` is supported. Default `target_weight`. |
| `rebalance.drift_threshold_percent` | number | no | Skip assets whose weight is within this many percentage points of target. 0–50. Default `2`. |
| `rebalance.minimum_order_value` | number | no | Skip trades smaller than this (dollars), except full closes of non-strategy positions. Default `5`. |
| `rebalance.allow_fractional_shares` | boolean | no | Default `true`. Effective only if both the broker and the user's safety settings allow it. |
| `risk_controls.maximum_position_weight` | number | no | Cap on any single target weight (fraction). Default `0.35`. |
| `risk_controls.maximum_order_value` | number | no | Cap per order in dollars. Default `1000`. |
| `risk_controls.maximum_batch_value` | number | no | Cap on the total value of one rebalance batch. Default `5000`. |

## Validation rules

- No duplicate symbols (case-insensitive).
- All weights strictly positive.
- No asset's weight may exceed `maximum_position_weight`.
- Total weight must not exceed 100% (±0.5% tolerance). A total **below** 100% is allowed and treated as a deliberate cash allocation; totals under 50% are rejected as probably a mistake.
- Unknown `schema_version` or `rebalance.method` values are rejected with a readable message.

The user's global safety settings (Settings → Trading safety limits) always apply **on top of** a strategy's own risk controls; the stricter value wins.

## Versioning

Every save or import writes a new row to `strategy_versions` with the full YAML, and an audit event records who-did-what-when. Export always reflects the current version.
