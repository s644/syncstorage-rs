#[macro_use]
extern crate slog_scope;
#[macro_use]
extern crate validator_derive;

pub mod api;
mod db;
mod error;
mod metrics;
pub mod middleware;

#[cfg(test)]
mod test;

use std::{sync::Arc, time::Duration};

use actix_web::web::{self, ServiceConfig};
use api::handlers;
use cadence::StatsdClient;
use error::ApiError;
use syncstorage_common::Metrics;
use syncstorage_db_common::DbPool;
#[cfg(feature = "mysql")]
use syncstorage_mysql::MysqlDbPool;
use syncstorage_settings::{Deadman, ServerLimits, Settings};
#[cfg(feature = "spanner")]
use syncstorage_spanner::SpannerDbPool;
use tokio::sync::RwLock;

#[cfg(any(
    not(any(feature = "mysql", feature = "spanner")),
    all(feature = "mysql", feature = "spanner"),
))]
compile_error!(
    "Exactly of one of the \"mysql\" and \"spanner\" features must be enabled for this crate."
);

pub const BSO_ID_REGEX: &str = r"[ -~]{1,64}";
pub const COLLECTION_ID_REGEX: &str = r"[a-zA-Z0-9._-]{1,32}";

pub fn build_configurator<'a>(
    settings: &'a Settings,
    statsd_host: Option<&'a str>,
    statsd_port: u16,
) -> Result<impl Fn(&mut ServiceConfig) + Copy + 'a, ApiError> {
    Ok(move |cfg: &mut ServiceConfig| {
        pub fn cfg_path(path: &str) -> String {
            const MYSQL_UID_REGEX: &str = r"[0-9]{1,10}";
            const SYNC_VERSION_PATH: &str = "1.5";

            let path = path
                .replace(
                    "{collection}",
                    &format!("{{collection:{}}}", COLLECTION_ID_REGEX),
                )
                .replace("{bso}", &format!("{{bso:{}}}", BSO_ID_REGEX));
            format!("/{}/{{uid:{}}}{}", SYNC_VERSION_PATH, MYSQL_UID_REGEX, path)
        }

        let statsd_client = syncstorage_common::statsd_client_from_opts(
            &settings.statsd_label,
            statsd_host,
            statsd_port,
        )
        .unwrap();
        // .map_err(ApiErrorKind::Internal)?;
        // TODO:

        #[cfg(feature = "spanner")]
        let db_pool = futures::executor::block_on(SpannerDbPool::new(
            settings,
            &Metrics::from(&statsd_client),
        ))
        .unwrap();

        #[cfg(feature = "mysql")]
        let db_pool = MysqlDbPool::new(settings, &Metrics::from(&statsd_client)).unwrap();

        syncstorage_db_common::spawn_pool_periodic_reporter(
            Duration::from_secs(10),
            statsd_client.clone(),
            db_pool.clone(),
        )
        .unwrap(); // TODO: remove unwrap

        let deadman = Arc::new(RwLock::new(Deadman {
            max_size: settings.database_pool_max_size,
            ..Default::default()
        }));

        let limits = Arc::new(settings.limits.clone());
        let state = ServerState {
            db_pool: Box::new(db_pool),
            limits: Arc::clone(&limits),
            limits_json: serde_json::to_string(&*limits).expect("ServerLimits failed to serialize"),
            statsd_client: Box::new(statsd_client),
            quota_enabled: settings.enable_quota,
            deadman: Arc::clone(&deadman),
        };

        cfg.data(state)
            .service(
                web::resource(&cfg_path("/info/collections"))
                    .route(web::get().to(handlers::get_collections)),
            )
            .service(
                web::resource(&cfg_path("/info/collection_counts"))
                    .route(web::get().to(handlers::get_collection_counts)),
            )
            .service(
                web::resource(&cfg_path("/info/collection_usage"))
                    .route(web::get().to(handlers::get_collection_usage)),
            )
            .service(
                web::resource(&cfg_path("/info/configuration"))
                    .route(web::get().to(handlers::get_configuration)),
            )
            .service(
                web::resource(&cfg_path("/info/quota")).route(web::get().to(handlers::get_quota)),
            )
            .service(web::resource(&cfg_path("")).route(web::delete().to(handlers::delete_all)))
            .service(
                web::resource(&cfg_path("/storage")).route(web::delete().to(handlers::delete_all)),
            )
            .service(
                web::resource(&cfg_path("/storage/{collection}"))
                    .app_data(
                        // Declare the payload limit for "normal" collections.
                        web::PayloadConfig::new(limits.max_request_bytes as usize),
                    )
                    .app_data(
                        // Declare the payload limits for "JSON" payloads
                        // (Specify "text/plain" for legacy client reasons)
                        web::JsonConfig::default()
                            .limit(limits.max_request_bytes as usize)
                            .content_type(|ct| ct == mime::TEXT_PLAIN),
                    )
                    .route(web::delete().to(handlers::delete_collection))
                    .route(web::get().to(handlers::get_collection))
                    .route(web::post().to(handlers::post_collection)),
            )
            .service(
                web::resource(&cfg_path("/storage/{collection}/{bso}"))
                    .app_data(web::PayloadConfig::new(limits.max_request_bytes as usize))
                    .app_data(
                        web::JsonConfig::default()
                            .limit(limits.max_request_bytes as usize)
                            .content_type(|ct| ct == mime::TEXT_PLAIN),
                    )
                    .route(web::delete().to(handlers::delete_bso))
                    .route(web::get().to(handlers::get_bso))
                    .route(web::put().to(handlers::put_bso)),
            );
    })
}

/// This is the global HTTP state object that will be made available to all
/// HTTP API calls.
pub struct ServerState {
    pub db_pool: Box<dyn DbPool>,

    /// Server-enforced limits for request payloads.
    pub limits: Arc<ServerLimits>,

    /// limits rendered as JSON
    pub limits_json: String,

    /// Metric reporting
    pub statsd_client: Box<StatsdClient>,

    pub quota_enabled: bool,

    pub deadman: Arc<RwLock<Deadman>>,
}
