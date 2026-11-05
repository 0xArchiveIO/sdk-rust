//! News: headlines and our own market events as stories, with what the first
//! market named did in the minutes after. Hyperliquid, HIP-3 and Lighter;
//! Build, Pro, Scale and Enterprise plans.

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::resources::pages::{pin_window_optional, CursorPages};
use crate::types::{CursorResponse, NewsStory, Timestamp};

/// The largest `limit` the news routes accept.
pub const NEWS_MAX_LIMIT: i64 = 500;

/// The most markets `symbols` takes.
pub const NEWS_MAX_SYMBOLS: usize = 20;

/// Filters, window and page for [`NewsResource::list`] and
/// [`NewsResource::iterate`].
///
/// Pass `next_cursor` back as `cursor` with every other field unchanged, and
/// stop when it is `None`.
#[derive(Debug, Default, Clone)]
pub struct NewsParams {
    /// Only stories whose lead report names one of these markets on this
    /// venue (at most 20; case does not matter). An entry may hold several
    /// markets separated by commas.
    pub symbols: Vec<String>,
    /// Only one topic, for example `hack` or `liquidation`.
    pub topic: Option<String>,
    /// Only stories of at least this weight, 1 to 3 (default 1).
    pub min_weight: Option<u8>,
    /// Earliest `first_seen_at`, inclusive (default: 24 hours before `end`).
    pub start: Option<Timestamp>,
    /// Latest `first_seen_at`, inclusive (default: now).
    pub end: Option<Timestamp>,
    /// The feed as we knew it at this time, with the reactions and reports
    /// recorded by then (default: now; a time in the future is now).
    pub as_of: Option<Timestamp>,
    /// The previous page's `next_cursor`, passed back unchanged.
    pub cursor: Option<String>,
    /// Stories per page, 1 to 500 (default 100).
    pub limit: Option<i64>,
}

impl NewsParams {
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        if let Some(l) = self.limit {
            if !(1..=NEWS_MAX_LIMIT).contains(&l) {
                return Err(Error::InvalidParam(format!(
                    "news limit must be between 1 and {NEWS_MAX_LIMIT}"
                )));
            }
        }
        if let Some(w) = self.min_weight {
            if !(1..=3).contains(&w) {
                return Err(Error::InvalidParam(
                    "news min_weight must be 1, 2 or 3".to_string(),
                ));
            }
        }
        let symbols: Vec<&str> = self
            .symbols
            .iter()
            .flat_map(|s| s.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if symbols.len() > NEWS_MAX_SYMBOLS {
            return Err(Error::InvalidParam(format!(
                "news symbols takes at most {NEWS_MAX_SYMBOLS} markets"
            )));
        }
        let mut qp = vec![];
        if !symbols.is_empty() {
            qp.push(("symbols", symbols.join(",")));
        }
        if let Some(t) = &self.topic {
            qp.push(("topic", t.clone()));
        }
        if let Some(w) = self.min_weight {
            qp.push(("min_weight", w.to_string()));
        }
        if let Some(s) = &self.start {
            qp.push(("start", s.to_millis().to_string()));
        }
        if let Some(e) = &self.end {
            qp.push(("end", e.to_millis().to_string()));
        }
        if let Some(a) = &self.as_of {
            qp.push(("as_of", a.to_millis().to_string()));
        }
        if let Some(l) = self.limit {
            qp.push(("limit", l.to_string()));
        }
        Ok(qp)
    }
}

/// News stories for one venue, as `client.hyperliquid.news`,
/// `client.hyperliquid.hip3.news` and `client.lighter.news`: the stories
/// whose lead report names a market on that venue, newest first by
/// `first_seen_at`.
///
/// Each [`NewsStory`] carries the reports that joined it, the markets it
/// names and, when a market was matched, a `reaction`: the 1 and 5 minute
/// windows after the story was first seen. A reaction is a measurement, not
/// a cause. Story moves are mostly noise: treat a move as typical unless
/// `move_level` is `unusual` or `large`.
///
/// Public-display stories only. Build, Pro, Scale and Enterprise plans; on
/// Free the API answers 403 and the error carries
/// [`PlanRequired`](crate::error::PlanRequired).
#[derive(Debug, Clone)]
pub struct NewsResource {
    http: HttpClient,
    path: String,
}

impl NewsResource {
    pub(crate) fn new(http: HttpClient, prefix: &str) -> Self {
        Self {
            http,
            path: format!("{prefix}/news"),
        }
    }

    /// Get one page of stories (`GET {venue}/news`).
    pub async fn list(&self, params: NewsParams) -> Result<CursorResponse<Vec<NewsStory>>> {
        let mut qp = params.query()?;
        if let Some(c) = &params.cursor {
            qp.push(("cursor", c.clone()));
        }
        self.http.get_with_cursor(&self.path, &qp).await
    }

    /// Walk every story of `[start, end]` that matches the filters.
    ///
    /// `start` is optional (the API reads the 24 hours before `end`). When
    /// `end` is `None` it is fixed to the current time now, before the first
    /// request, and every page sends that same `end`. A `cursor` in `params`
    /// resumes from that page. The pager stops only when a page carries no
    /// `next_cursor`.
    pub fn iterate(&self, params: NewsParams) -> Result<CursorPages<NewsStory>> {
        let params = pin_window_optional(params, |p| &mut p.end);
        let qp = params.query()?;
        Ok(CursorPages::new(
            self.http.clone(),
            self.path.clone(),
            qp,
            params.cursor,
        ))
    }
}
