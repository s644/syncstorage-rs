use actix_web::{
    dev::Payload, http::header::HeaderMap, web::Data, Error, FromRequest, HttpMessage, HttpRequest,
    HttpResponse,
};
use serde::Serialize;
use syncstorage_web::api::extractors::{DbStatus as SyncstorageDbStatus, QuotaInfo};
use tokenserver_web::extractors::DbStatus as TokenserverDbStatus;

#[derive(Serialize)]
enum Status {
    Ok,
    Err,
}

#[derive(Serialize)]
pub struct HeartbeatResponse {
    database: SyncstorageDbStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    database_msg: Option<&'static str>,
    status: Status,
    version: String,
    quota: QuotaInfo,
    tokenserver: TokenserverHeartbeatResponse,
}

#[derive(Serialize)]
pub struct TokenserverHeartbeatResponse {
    database: TokenserverDbStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    database_msg: Option<&'static str>,
    status: Status,
}

/** Returns a status message indicating the state of the current server
 *
 */
pub async fn heartbeat(
    quota: QuotaInfo,
    syncstorage_db_status: SyncstorageDbStatus,
    tokenserver_db_status: TokenserverDbStatus,
) -> HttpResponse {
    let response = HeartbeatResponse {
        database: syncstorage_db_status,
        database_msg: match syncstorage_db_status {
            SyncstorageDbStatus::Err => Some("check failed without error"),
            _ => None,
        },
        status: match syncstorage_db_status {
            SyncstorageDbStatus::Ok => Status::Ok,
            _ => Status::Err,
        },
        version: env!("CARGO_PKG_VERSION").to_owned(),
        quota,
        tokenserver: TokenserverHeartbeatResponse {
            database: tokenserver_db_status,
            database_msg: match syncstorage_db_status {
                SyncstorageDbStatus::Err => Some("check failed without error"),
                _ => None,
            },
            status: match syncstorage_db_status {
                SyncstorageDbStatus::Ok => Status::Ok,
                _ => Status::Err,
            },
        },
    };

    if matches!(syncstorage_db_status, SyncstorageDbStatus::Unknown)
        || matches!(tokenserver_db_status, TokenserverDbStatus::Unknown)
    {
        HttpResponse::ServiceUnavailable().json(response)
    } else {
        HttpResponse::Ok().json(response)
    }
}

// pub async fn lbheartbeat(req: HttpRequest) -> Result<HttpResponse, ApiError> {
//     let mut resp: HashMap<String, Value> = HashMap::new();

//     let state = match req.app_data::<Data<ServerState>>() {
//         Some(s) => s,
//         None => {
//             error!("⚠️ Could not load the app state");
//             return Ok(HttpResponse::InternalServerError().body(""));
//         }
//     };

//     let deadarc = state.deadman.clone();
//     let mut deadman = *deadarc.read().await;
//     let db_state = if cfg!(test) {
//         use actix_web::http::header::HeaderValue;
//         use std::str::FromStr;
//         use syncstorage_db_common::PoolState;

//         let test_pool = PoolState {
//             connections: u32::from_str(
//                 req.headers()
//                     .get("TEST_CONNECTIONS")
//                     .unwrap_or(&HeaderValue::from_static("0"))
//                     .to_str()
//                     .unwrap_or("0"),
//             )
//             .unwrap_or_default(),
//             idle_connections: u32::from_str(
//                 req.headers()
//                     .get("TEST_IDLES")
//                     .unwrap_or(&HeaderValue::from_static("0"))
//                     .to_str()
//                     .unwrap_or("0"),
//             )
//             .unwrap_or_default(),
//         };
//         // dbg!(&test_pool, deadman.max_size);
//         test_pool
//     } else {
//         state.db_pool.clone().state()
//     };

//     let active = db_state.connections - db_state.idle_connections;
//     let mut status_code = StatusCode::OK;

//     if let Some(max_size) = deadman.max_size {
//         if active >= max_size && db_state.idle_connections == 0 {
//             if deadman.clock_start.is_none() {
//                 deadman.clock_start = Some(time::Instant::now());
//             }
//             status_code = StatusCode::INTERNAL_SERVER_ERROR;
//         } else if deadman.clock_start.is_some() {
//             deadman.clock_start = None
//         }
//         deadman.previous_count = db_state.idle_connections as usize;
//         {
//             *deadarc.write().await = deadman;
//         }
//         resp.insert("active_connections".to_string(), Value::from(active));
//         resp.insert(
//             "idle_connections".to_string(),
//             Value::from(db_state.idle_connections),
//         );
//         if let Some(clock) = deadman.clock_start {
//             let duration: time::Duration = time::Instant::now() - clock;
//             resp.insert(
//                 "duration_ms".to_string(),
//                 Value::from(duration.whole_milliseconds()),
//             );
//         };
//     }

//     Ok(HttpResponseBuilder::new(status_code).json(json!(resp)))
// }

// // try returning an API error
// pub async fn test_error(
//     _req: HttpRequest,
//     _ter: TestErrorRequest,
// ) -> Result<HttpResponse, ApiError> {
//     // generate an error for sentry.

//     // ApiError will call the middleware layer to auto-append the tags.
//     error!("Test Error");
//     let err = ApiError::from(ApiErrorKind::Internal("Oh Noes!".to_owned()));

//     Err(err)
// }
