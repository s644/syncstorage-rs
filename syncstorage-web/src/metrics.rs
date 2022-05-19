use actix_web::{dev::Payload, web::Data, Error, FromRequest, HttpRequest};
use futures::future;
use futures::future::Ready;
use syncstorage_common::Metrics;

use super::ServerState;

pub struct MetricsWrapper(pub Metrics);

impl FromRequest for MetricsWrapper {
    type Config = ();
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        future::ok(Self(syncserver_web_common::metrics_from_request(
            req,
            req.app_data::<Data<ServerState>>()
                .map(|state| state.statsd_client.clone()),
        )))
    }
}

impl From<&ServerState> for Metrics {
    fn from(state: &ServerState) -> Self {
        Metrics {
            client: Some(*state.statsd_client.clone()),
            tags: None,
            timer: None,
        }
    }
}
