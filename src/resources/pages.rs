//! A cursor pager: follows `next_cursor` until the API stops returning one.

use std::fmt;
use std::marker::PhantomData;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::types::{CursorResponse, Timestamp};

/// Walks a cursor-paged route page by page, returned by the `iterate*`
/// methods (for example `client.hyperliquid.trade_intent.iterate()`).
///
/// Every request sends the same parameters, plus the cursor the previous page
/// returned. The walk ends only when a page carries no `next_cursor`: a page
/// can be short, or even empty, and still be followed by more, so the pager
/// never stops on the size of a page. If the API returned the cursor it was
/// just sent, the next call fails instead of requesting the same page again.
///
/// ```no_run
/// # use oxarchive::OxArchive;
/// # use oxarchive::resources::trade_intent::TradeIntentParams;
/// # async fn example() -> oxarchive::Result<()> {
/// # let client = OxArchive::new("key")?;
/// let mut pages = client.hyperliquid.trade_intent.iterate("BTC", TradeIntentParams {
///     start: Some("2026-11-02T00:00:00Z".into()),
///     end: Some("2026-11-03T00:00:00Z".into()),
///     ..Default::default()
/// })?;
/// while let Some(page) = pages.next_page().await? {
///     for bucket in &page.data {
///         println!("{} open_long={}", bucket.timestamp, bucket.open_long);
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct CursorPages<T> {
    http: HttpClient,
    path: String,
    query: Vec<(&'static str, String)>,
    state: State,
    _rows: PhantomData<fn() -> T>,
}

enum State {
    /// The next request: without a cursor (the first page) or with one.
    Next(Option<String>),
    /// The last page returned the cursor it was requested with.
    Repeated(String),
    /// The last page carried no cursor.
    Done,
}

impl<T> fmt::Debug for CursorPages<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (next_cursor, done) = match &self.state {
            State::Next(cursor) => (cursor.as_deref(), false),
            State::Repeated(cursor) => (Some(cursor.as_str()), false),
            State::Done => (None, true),
        };
        f.debug_struct("CursorPages")
            .field("path", &self.path)
            .field("query", &self.query)
            .field("next_cursor", &next_cursor)
            .field("done", &done)
            .finish()
    }
}

impl<T: DeserializeOwned> CursorPages<T> {
    /// A pager over `path` with `query`, starting at `cursor` (the first page
    /// when `None`). `query` must not carry a cursor.
    pub(crate) fn new(
        http: HttpClient,
        path: String,
        query: Vec<(&'static str, String)>,
        cursor: Option<String>,
    ) -> Self {
        Self {
            http,
            path,
            query,
            state: State::Next(cursor),
            _rows: PhantomData,
        }
    }

    /// The parameters every request sends, without the cursor.
    pub fn query(&self) -> &[(&'static str, String)] {
        &self.query
    }

    /// Fetch the next page, or `Ok(None)` once a page has come back without
    /// a `next_cursor`.
    ///
    /// The page keeps its `meta` (notices, `built_through`, coverage), and
    /// its `data` can be empty while more pages follow.
    pub async fn next_page(&mut self) -> Result<Option<CursorResponse<Vec<T>>>> {
        let cursor = match &self.state {
            State::Done => return Ok(None),
            State::Repeated(cursor) => {
                return Err(Error::Deserialize(format!(
                    "the API returned the cursor {cursor} for the request that sent it; \
                     stopping rather than requesting the same page again"
                )))
            }
            State::Next(cursor) => cursor.clone(),
        };
        let mut query = self.query.clone();
        if let Some(c) = &cursor {
            query.push(("cursor", c.clone()));
        }
        let page: CursorResponse<Vec<T>> = self.http.get_with_cursor(&self.path, &query).await?;
        self.state = match &page.next_cursor {
            None => State::Done,
            Some(next) if cursor.as_deref() == Some(next.as_str()) => State::Repeated(next.clone()),
            Some(next) => State::Next(Some(next.clone())),
        };
        Ok(Some(page))
    }

    /// Fetch every remaining page and return their rows in order.
    pub async fn collect_all(mut self) -> Result<Vec<T>> {
        let mut rows = Vec::new();
        while let Some(page) = self.next_page().await? {
            rows.extend(page.data);
        }
        Ok(rows)
    }
}

/// `params` with `end` fixed to now when unset, after checking that `start`
/// is set (`has_start`).
pub(crate) fn pin_window<P>(
    has_start: bool,
    mut params: P,
    end: impl FnOnce(&mut P) -> &mut Option<Timestamp>,
) -> Result<P> {
    if !has_start {
        return Err(Error::InvalidParam(
            "iterate needs a start: the window must stay the same on every page".to_string(),
        ));
    }
    let end = end(&mut params);
    if end.is_none() {
        *end = Some(Timestamp::Millis(chrono::Utc::now().timestamp_millis()));
    }
    Ok(params)
}
