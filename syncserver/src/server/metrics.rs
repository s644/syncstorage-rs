use actix_web::{dev::Payload, web::Data, Error, FromRequest, HttpRequest};
use cadence::StatsdClient;
use futures::future;
use futures::future::Ready;
use syncserver_common::{Metrics, Tags};
use syncserver_db_common::DbPool as DbPoolTrait;

use super::ServerState;
use crate::db::DbPool;
use crate::web::tags::TagsWrapper;

pub struct MetricsWrapper(pub Metrics);

impl FromRequest for MetricsWrapper {
    type Config = ();
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        future::ok(Self(metrics_from_request(
            req,
            req.app_data::<Data<ServerState<DbPool>>>()
                .map(|state| state.metrics.clone()),
        )))
    }
}

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

// TODO: rename from <E> to <T>
impl<E> From<&ServerState<E>> for Metrics
where
    E: DbPoolTrait,
{
    fn from(state: &ServerState<E>) -> Self {
        Metrics {
            client: Some(*state.metrics.clone()),
            tags: None,
            timer: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tags() {
        use actix_web::dev::RequestHead;
        use actix_web::http::{header, uri::Uri};
        use std::collections::HashMap;

        let mut rh = RequestHead::default();
        let path = "/1.5/42/storage/meta/global";
        rh.uri = Uri::from_static(path);
        rh.headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:72.0) Gecko/20100101 Firefox/72.0",
            ),
        );

        let TagsWrapper(tags) = TagsWrapper::from(&rh);

        let mut result = HashMap::<String, String>::new();
        result.insert("ua.os.ver".to_owned(), "NT 10.0".to_owned());
        result.insert("ua.os.family".to_owned(), "Windows".to_owned());
        result.insert("ua.browser.ver".to_owned(), "72.0".to_owned());
        result.insert("ua.name".to_owned(), "Firefox".to_owned());
        result.insert("ua.browser.family".to_owned(), "Firefox".to_owned());
        result.insert("uri.method".to_owned(), "GET".to_owned());

        assert_eq!(tags.tags, result)
    }

    #[test]
    fn no_empty_tags() {
        use actix_web::dev::RequestHead;
        use actix_web::http::{header, uri::Uri};

        let mut rh = RequestHead::default();
        let path = "/1.5/42/storage/meta/global";
        rh.uri = Uri::from_static(path);
        rh.headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("Mozilla/5.0 (curl) Gecko/20100101 curl"),
        );

        let TagsWrapper(tags) = TagsWrapper::from(&rh);
        assert!(!tags.tags.contains_key("ua.os.ver"));
        println!("{:?}", tags);
    }
}
