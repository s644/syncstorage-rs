use std::error::Error as StdError;
use std::task::{Context, Poll};
use std::{cell::RefCell, rc::Rc};

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web::Data,
    Error, HttpMessage,
};
use futures::future::{self, LocalBoxFuture};
use sentry::protocol::Event;
use sentry_backtrace::parse_stacktrace;
use syncserver_common::ReportableError;
use tokenserver_common::error::TokenserverError;

use crate::error::ApiError;
use crate::server::{metrics::Metrics, ServerState};
use crate::web::tags::Tags;

pub struct SentryWrapper;

impl SentryWrapper {
    pub fn new() -> Self {
        SentryWrapper::default()
    }
}

impl Default for SentryWrapper {
    fn default() -> Self {
        Self
    }
}

impl<S, B> Transform<S> for SentryWrapper
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SentryWrapperMiddleware<S>;
    type Future = LocalBoxFuture<'static, Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        Box::pin(future::ok(SentryWrapperMiddleware {
            service: Rc::new(RefCell::new(service)),
        }))
    }
}

#[derive(Debug)]
pub struct SentryWrapperMiddleware<S> {
    service: Rc<RefCell<S>>,
}

pub fn report(tags: &Tags, mut event: Event<'static>) {
    let tags = tags.clone();
    event.tags = tags.clone().tag_tree();
    event.extra = tags.extra_tree();
    trace!("Sentry: Sending error: {:?}", &event);
    sentry::capture_event(event);
}

impl<S, B> Service for SentryWrapperMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, sreq: ServiceRequest) -> Self::Future {
        let mut tags = Tags::from(sreq.head());
        sreq.extensions_mut().insert(tags.clone());
        let metrics = sreq
            .app_data::<Data<ServerState>>()
            .map(|state| Metrics::from(state.get_ref()));

        let fut = self.service.call(sreq);

        Box::pin(async move {
            let mut sresp = fut.await?;
            // handed an actix_error::error::Error;
            // Fetch out the tags (in case any have been added.) NOTE: request extensions
            // are NOT automatically passed to responses. You need to check both.
            if let Some(t) = sresp.request().extensions().get::<Tags>() {
                trace!("Sentry: found tags in request: {:?}", &t.tags);
                for (k, v) in t.tags.clone() {
                    tags.tags.insert(k, v);
                }
                for (k, v) in t.extra.clone() {
                    tags.extra.insert(k, v);
                }
            };
            if let Some(t) = sresp.response().extensions().get::<Tags>() {
                trace!("Sentry: found tags in response: {:?}", &t.tags);
                for (k, v) in t.tags.clone() {
                    tags.tags.insert(k, v);
                }
                for (k, v) in t.extra.clone() {
                    tags.extra.insert(k, v);
                }
            };
            //dbg!(&tags);
            match sresp.response().error() {
                None => {
                    // Middleware errors are eaten by current versions of Actix. Errors are now added
                    // to the extensions. Need to check both for any errors and report them.
                    if let Some(events) = sresp
                        .request()
                        .extensions_mut()
                        .remove::<Vec<Event<'static>>>()
                    {
                        for event in events {
                            trace!("Sentry: found an error stored in request: {:?}", &event);
                            report(&tags, event);
                        }
                    }
                    if let Some(events) = sresp
                        .response_mut()
                        .extensions_mut()
                        .remove::<Vec<Event<'static>>>()
                    {
                        for event in events {
                            trace!("Sentry: Found an error stored in response: {:?}", &event);
                            report(&tags, event);
                        }
                    }
                }
                Some(e) => {
                    if let Some(apie) = e.as_error::<ApiError>() {
                        process_error(apie, metrics.as_ref(), &tags);
                    } else if let Some(tokenserver_error) = e.as_error::<TokenserverError>() {
                        process_error(tokenserver_error, metrics.as_ref(), &tags);
                    }
                }
            }
            Ok(sresp)
        })
    }
}

fn process_error<E>(err: &E, metrics: Option<&Metrics>, tags: &Tags)
where
    E: ReportableError + StdError + 'static,
{
    if let Some(metrics) = metrics {
        if let Some(label) = err.metric_label() {
            metrics.incr(&label);
        }
    }

    if err.is_sentry_event() {
        report(tags, event_from_error(err));
    } else {
        trace!("Sentry: Not reporting error: {:?}", err);
    }
}

/// Custom `sentry::event_from_error` for `ReportableError`
///
/// `sentry::event_from_error` can't access `std::Error` backtraces as its
/// `backtrace()` method is currently Rust nightly only. This function works
/// against `ReportableError` instead to access its backtrace.
pub fn event_from_error<E>(err: &E) -> Event<'static>
where
    E: ReportableError + StdError + 'static,
{
    let mut exceptions = vec![exception_from_error_with_backtrace(err)];

    let mut source = err.source();
    while let Some(err) = source {
        let exception = if let Some(err) = err.downcast_ref::<E>() {
            exception_from_error_with_backtrace(err)
        } else {
            exception_from_error(err)
        };
        exceptions.push(exception);
        source = err.source();
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: sentry::protocol::Level::Error,
        ..Default::default()
    }
}

/// Custom `exception_from_error` support function for `ReportableError`
///
/// Based moreso on sentry_failure's `exception_from_single_fail`.
fn exception_from_error_with_backtrace<E>(err: &E) -> sentry::protocol::Exception
where
    E: ReportableError + StdError,
{
    let mut exception = exception_from_error(err);
    exception.stacktrace = parse_stacktrace(&err.error_backtrace());
    exception
}

/// Exact copy of sentry's unfortunately private `exception_from_error`
fn exception_from_error<E: StdError + ?Sized>(err: &E) -> sentry::protocol::Exception {
    let dbg = format!("{:?}", err);
    sentry::protocol::Exception {
        ty: sentry::parse_type_from_debug(&dbg).to_owned(),
        value: Some(err.to_string()),
        ..Default::default()
    }
}
