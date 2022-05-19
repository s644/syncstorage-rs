#[macro_use]
extern crate slog_scope;

pub mod middleware;
pub mod tags;
pub mod user_agent;

use actix_web::HttpRequest;
use cadence::StatsdClient;
use syncstorage_common::{Metrics, Tags};

use tags::TagsWrapper;

pub fn metrics_from_request(req: &HttpRequest, client: Option<Box<StatsdClient>>) -> Metrics {
    let exts = req.extensions();
    let TagsWrapper(def_tags) = TagsWrapper::from(req.head());
    let tags = exts.get::<Tags>().unwrap_or(&def_tags);

    if client.is_none() {
        warn!("⚠️ metric error: No App State");
    }

    Metrics {
        client: client.as_deref().cloned(),
        tags: Some(tags.clone()),
        timer: None,
    }
}
