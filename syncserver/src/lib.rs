#![warn(rust_2018_idioms)]
#![allow(clippy::try_err)]

mod dockerflow;
pub mod logging;

use std::{error::Error, sync::Arc};

use actix_cors::Cors;
use actix_web::{
    dev,
    http::Method,
    http::StatusCode,
    middleware::{errhandlers::ErrorHandlers, Condition},
    web::ServiceConfig,
    App, HttpServer,
};
use syncserver_settings::Settings;

#[cfg(not(any(feature = "syncstorage", feature = "tokenserver")))]
compile_error!("At least one of the \"syncstorage\" or \"tokenserver\" features must be enabled for this crate.");

pub const SYNC_DOCS_URL: &str =
    "https://mozilla-services.readthedocs.io/en/latest/storage/apis-1.5.html";

pub struct Server;
#[macro_export]
macro_rules! build_app2 {
    ($syncstorage_state: expr, $tokenserver_state: expr, $secrets: expr, $limits: expr, $cors: expr) => {
        App::new()
        // Dockerflow
        // Remember to update .::web::middleware::DOCKER_FLOW_ENDPOINTS
        // when applying changes to endpoint names.
        // .service(web::resource("/__heartbeat__").route(web::get().to(handlers::heartbeat)))
        // .service(web::resource("/__lbheartbeat__").route(web::get().to(
        //     handlers::lbheartbeat, /*
        //                                |_: HttpRequest| {
        //                                // used by the load balancers, just return OK.
        //                                HttpResponse::Ok()
        //                                    .content_type("application/json")
        //                                    .body("{}")
        //                            }
        //                            */
        // )))
        // .service(
        //     web::resource("/__version__").route(web::get().to(|_: HttpRequest| {
        //         // return the contents of the version.json file created by circleci
        //         // and stored in the docker root
        //         HttpResponse::Ok()
        //             .content_type("application/json")
        //             .body(include_str!("../../version.json"))
        //     })),
        // )
        // .service(web::resource("/__error__").route(web::get().to(handlers::test_error)))
        // .service(web::resource("/").route(web::get().to(|_: HttpRequest| {
        //     HttpResponse::Found()
        //         .header(LOCATION, SYNC_DOCS_URL)
        //         .finish()
        // })))
    };
}

#[macro_export]
macro_rules! build_app_without_syncstorage {
    ($state: expr, $secrets: expr, $cors: expr) => {
        App::new()
        // // Dockerflow
        // // Remember to update .::web::middleware::DOCKER_FLOW_ENDPOINTS
        // // when applying changes to endpoint names.
        // .service(
        //     web::resource("/__heartbeat__")
        //         .route(web::get().to(tokenserver::handlers::heartbeat)),
        // )
        // .service(
        //     web::resource("/__lbheartbeat__").route(web::get().to(|_: HttpRequest| {
        //         // used by the load balancers, just return OK.
        //         HttpResponse::Ok()
        //             .content_type("application/json")
        //             .body("{}")
        //     })),
        // )
        // .service(
        //     web::resource("/__version__").route(web::get().to(|_: HttpRequest| {
        //         // return the contents of the version.json file created by circleci
        //         // and stored in the docker root
        //         HttpResponse::Ok()
        //             .content_type("application/json")
        //             .body(include_str!("../../version.json"))
        //     })),
        // )
        // .service(web::resource("/").route(web::get().to(|_: HttpRequest| {
        //     HttpResponse::Found()
        //         .header(LOCATION, SYNC_DOCS_URL)
        //         .finish()
        // })))
    };
}

#[macro_export]
macro_rules! build_app {
    ($settings: expr) => {
        App::new()
            .data(Arc::new($settings.master_secret.clone()))
            // Middleware is applied LIFO
            // These will wrap all outbound responses with matching status codes.
            .wrap(Condition::new(
                cfg!(feature = "syncstorage"),
                ErrorHandlers::new().handler(
                    StatusCode::NOT_FOUND,
                    syncstorage_web::middleware::render_404,
                ),
            ))
            // These are our wrappers
            .wrap(Condition::new(
                cfg!(feature = "syncstorage"),
                syncstorage_web::middleware::WeaveTimestamp::new(),
            ))
            .wrap(Condition::new(
                cfg!(feature = "tokenserver"),
                tokenserver_web::middleware::LoggingWrapper::new(),
            ))
            .wrap(syncstorage_web::middleware::SentryWrapper::default())
            .wrap(syncstorage_web::middleware::RejectUA::default())
            // Followed by the "official middleware" so they run first.
            // actix is getting increasingly tighter about CORS headers. Our server is
            // not a huge risk but does deliver XHR JSON content.
            // For now, let's be permissive and use NGINX (the wrapping server)
            // for finer grained specification.
            .wrap(build_cors($settings))
            .wrap_fn(|req, srv| {
                #[cfg(feature = "syncstorage")]
                syncstorage_web::middleware::emit_http_status_with_tokenserver_origin(req, srv)
            })
            .configure(build_configurator($settings))
    };
}

fn build_configurator(settings: &'static Settings) -> impl Fn(&mut ServiceConfig) + Copy + 'static {
    move |cfg: &mut ServiceConfig| {
        #[cfg(feature = "syncstorage")]
        syncstorage_web::build_configurator(
            &settings.syncstorage,
            settings.statsd_host.as_deref(),
            settings.statsd_port,
        )
        .expect("failed to build syncstorage configurator")(cfg);

        #[cfg(feature = "tokenserver")]
        tokenserver_web::build_configurator(
            &settings.tokenserver,
            settings.statsd_host.as_deref(),
            settings.statsd_port,
        )
        .expect("failed to build tokenserver configurator")(cfg);
    }
}

impl Server {
    pub async fn with_settings(settings: &'static Settings) -> Result<dev::Server, Box<dyn Error>> {
        let server = HttpServer::new(move || build_app!(settings));

        let server = server
            .bind(format!("{}:{}", settings.host, settings.port))
            .expect("Could not get Server in Server::with_settings")
            .run();
        Ok(server)
    }
}

pub fn build_cors(settings: &Settings) -> Cors {
    // Followed by the "official middleware" so they run first.
    // actix is getting increasingly tighter about CORS headers. Our server is
    // not a huge risk but does deliver XHR JSON content.
    // For now, let's be permissive and use NGINX (the wrapping server)
    // for finer grained specification.
    let mut cors = Cors::default();

    if let Some(allowed_origin) = &settings.cors_allowed_origin {
        cors = cors.allowed_origin(allowed_origin);
    }

    if let Some(allowed_methods) = &settings.cors_allowed_methods {
        let mut methods = vec![];
        for method_string in allowed_methods {
            let method = Method::from_bytes(method_string.as_bytes()).unwrap();
            methods.push(method);
        }
        cors = cors.allowed_methods(methods);
    }
    if let Some(allowed_headers) = &settings.cors_allowed_headers {
        cors = cors.allowed_headers(allowed_headers);
    }

    if let Some(max_age) = &settings.cors_max_age {
        cors = cors.max_age(*max_age);
    }

    cors
}
