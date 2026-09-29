//! Wallet classification: precomputed daily behavior metrics for active
//! wallets, with filtering, sorting and offset paging.

use crate::error::Result;
use crate::http::HttpClient;
use crate::types::WalletClassification;

/// Filters, sort and page for [`WalletsResource::classify`].
///
/// Every field is optional; the API defaults are noted on each.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct WalletClassifyParams {
    /// Minimum order count (default 100).
    pub min_orders: Option<i64>,
    /// Minimum fill volume, in USD (default 0).
    pub min_volume_usd: Option<f64>,
    /// Metric to sort by (default `total_orders`). Also accepted:
    /// `total_fills`, `total_volume`, `total_volume_usd`, `cancel_rate`,
    /// `fill_rate`, `maker_ratio`, `avg_order_size_usd`,
    /// `avg_order_notional`, `max_order_size_usd`, `max_order_notional`,
    /// `active_hours`, `unique_coins`, `total_fees`, `total_fees_usd`,
    /// `realized_pnl`, `realized_pnl_usd`, `median_cancel_speed_ms`,
    /// `twap_fills`, `total_priority_gas`, `total_priority_gas_paid`,
    /// `total_builder_fees` and `total_builder_fees_paid`.
    pub sort: Option<String>,
    /// Sort order: `desc` (the default) or `asc`.
    pub order: Option<String>,
    /// Wallets per page, 1 to 1,000 (default 100).
    pub limit: Option<i64>,
    /// Wallets to skip, for paging (default 0, capped at 100,000). Page
    /// until `offset` reaches the response's `total`.
    pub offset: Option<i64>,
    /// Keep only wallets that did, or did not, use TWAP orders.
    pub uses_twap: Option<bool>,
    /// Keep only wallets that did, or did not, pay priority gas.
    pub uses_priority_gas: Option<bool>,
    /// Minimum cancel rate, from 0.0 to 1.0.
    pub min_cancel_rate: Option<f64>,
    /// Maximum cancel rate, from 0.0 to 1.0.
    pub max_cancel_rate: Option<f64>,
    /// Daily snapshot date (default: yesterday, UTC).
    pub date: Option<chrono::NaiveDate>,
}

impl WalletClassifyParams {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        let mut qp = vec![];
        if let Some(v) = self.min_orders {
            qp.push(("min_orders", v.to_string()));
        }
        if let Some(v) = self.min_volume_usd {
            qp.push(("min_volume_usd", v.to_string()));
        }
        if let Some(v) = &self.sort {
            qp.push(("sort", v.clone()));
        }
        if let Some(v) = &self.order {
            qp.push(("order", v.clone()));
        }
        if let Some(v) = self.limit {
            qp.push(("limit", v.to_string()));
        }
        if let Some(v) = self.offset {
            qp.push(("offset", v.to_string()));
        }
        if let Some(v) = self.uses_twap {
            qp.push(("uses_twap", v.to_string()));
        }
        if let Some(v) = self.uses_priority_gas {
            qp.push(("uses_priority_gas", v.to_string()));
        }
        if let Some(v) = self.min_cancel_rate {
            qp.push(("min_cancel_rate", v.to_string()));
        }
        if let Some(v) = self.max_cancel_rate {
            qp.push(("max_cancel_rate", v.to_string()));
        }
        if let Some(v) = self.date {
            qp.push(("date", v.format("%Y-%m-%d").to_string()));
        }
        qp
    }
}

/// Access to wallet classification, as `client.hyperliquid.wallets`
/// (`/v1/hyperliquid/wallets`) and `client.hyperliquid.hip3.wallets`
/// (`/v1/hyperliquid/hip3/wallets`).
#[derive(Debug, Clone)]
pub struct WalletsResource {
    http: HttpClient,
    prefix: String,
}

impl WalletsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            prefix: prefix.to_string(),
        }
    }

    /// Get precomputed daily behavior metrics for active wallets
    /// (`GET {prefix}/wallets/classify`).
    ///
    /// One page of wallets matching the filters, in the requested sort
    /// order, with `total` counting every match. Page with `offset`.
    pub async fn classify(&self, params: WalletClassifyParams) -> Result<WalletClassification> {
        self.http
            .get(
                &format!("{}/wallets/classify", self.prefix),
                &params.to_query(),
            )
            .await
    }
}
