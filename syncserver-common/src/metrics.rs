use std::net::UdpSocket;
use std::time::Instant;

use cadence::{
    BufferedUdpMetricSink, Counted, Metric, NopMetricSink, QueuingMetricSink, StatsdClient, Timed,
};

use crate::Tags;

#[derive(Debug, Clone)]
pub struct MetricTimer {
    pub label: String,
    pub start: Instant,
    pub tags: Tags,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    pub client: Option<StatsdClient>,
    pub tags: Option<Tags>,
    pub timer: Option<MetricTimer>,
}

impl Drop for Metrics {
    fn drop(&mut self) {
        let tags = self.tags.clone().unwrap_or_default();
        if let Some(client) = self.client.as_ref() {
            if let Some(timer) = self.timer.as_ref() {
                let lapse = (Instant::now() - timer.start).as_millis() as u64;
                trace!("⌚ Ending timer at nanos: {:?} : {:?}", &timer.label, lapse; &tags);
                let mut tagged = client.time_with_tags(&timer.label, lapse);
                // Include any "hard coded" tags.
                // tagged = tagged.with_tag("version", env!("CARGO_PKG_VERSION"));
                let tags = timer.tags.tags.clone();
                let keys = tags.keys();
                for tag in keys {
                    tagged = tagged.with_tag(tag, tags.get(tag).unwrap())
                }
                match tagged.try_send() {
                    Err(e) => {
                        // eat the metric, but log the error
                        warn!("⚠️ Metric {} error: {:?} ", &timer.label, e);
                    }
                    Ok(v) => {
                        trace!("⌚ {:?}", v.as_metric_str());
                    }
                }
            }
        }
    }
}

impl From<&StatsdClient> for Metrics {
    fn from(client: &StatsdClient) -> Self {
        Metrics {
            client: Some(client.clone()),
            tags: None,
            timer: None,
        }
    }
}

impl Metrics {
    pub fn sink() -> StatsdClient {
        StatsdClient::builder("", NopMetricSink).build()
    }

    pub fn noop() -> Self {
        Self {
            client: Some(Self::sink()),
            timer: None,
            tags: None,
        }
    }

    pub fn start_timer(&mut self, label: &str, tags: Option<Tags>) {
        let mut mtags = self.tags.clone().unwrap_or_default();
        if let Some(t) = tags {
            mtags.extend(t)
        }

        trace!("⌚ Starting timer... {:?}", &label; &mtags);
        self.timer = Some(MetricTimer {
            label: label.to_owned(),
            start: Instant::now(),
            tags: mtags,
        });
    }

    // increment a counter with no tags data.
    pub fn incr(&self, label: &str) {
        self.incr_with_tags(label, None)
    }

    pub fn incr_with_tags(&self, label: &str, tags: Option<Tags>) {
        self.count_with_tags(label, 1, tags)
    }

    pub fn incr_with_tag(&self, label: &str, key: &str, value: &str) {
        self.incr_with_tags(label, Some(Tags::with_tag(key, value)))
    }

    pub fn count(&self, label: &str, count: i64) {
        self.count_with_tags(label, count, None)
    }

    pub fn count_with_tags(&self, label: &str, count: i64, tags: Option<Tags>) {
        if let Some(client) = self.client.as_ref() {
            let mut tagged = client.count_with_tags(label, count);
            let mut mtags = self.tags.clone().unwrap_or_default();
            if let Some(tags) = tags {
                mtags.extend(tags);
            }
            for key in mtags.tags.keys().clone() {
                if let Some(val) = mtags.tags.get(key) {
                    tagged = tagged.with_tag(key, val.as_ref());
                }
            }
            // Include any "hard coded" tags.
            // incr = incr.with_tag("version", env!("CARGO_PKG_VERSION"));
            match tagged.try_send() {
                Err(e) => {
                    // eat the metric, but log the error
                    warn!("⚠️ Metric {} error: {:?} ", label, e; mtags);
                }
                Ok(v) => trace!("☑️ {:?}", v.as_metric_str()),
            }
        }
    }
}

pub fn metrics_from_opts(
    label: &str,
    host: Option<&str>,
    port: u16,
) -> Result<StatsdClient, String> {
    let builder = if let Some(statsd_host) = host {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;

        let host = (statsd_host, port);
        let udp_sink = BufferedUdpMetricSink::from(host, socket).map_err(|e| e.to_string())?;
        let sink = QueuingMetricSink::from(udp_sink);
        StatsdClient::builder(label, sink)
    } else {
        StatsdClient::builder(label, NopMetricSink)
    };
    Ok(builder
        .with_error_handler(|err| {
            warn!("⚠️ Metric send error:  {:?}", err);
        })
        .build())
}
