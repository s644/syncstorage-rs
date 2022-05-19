pub mod rejectua;
pub mod sentry;

use std::error::Error;

use actix_http::ResponseError;

pub trait MetricError: ResponseError + Error {
    fn is_reportable(&self) -> bool;
    fn metric_label(&self) -> Option<String>;
    fn backtrace(&self) -> String;
}
