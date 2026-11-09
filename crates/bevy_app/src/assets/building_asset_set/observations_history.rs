//! Owned immutable rows: serialize each appended row once, not the old history.
use serde_json::Value;
use std::{
    ops::Deref,
    time::{Duration, Instant},
};

pub(super) const BYTE_CAP: usize = 64 * 1024 * 1024;

#[derive(Default)]
pub(super) struct History<const EVENTS: bool> {
    rows: Vec<Value>,
    encoded: Vec<u8>,
    failure: Option<String>,
    encoding_time: Duration,
    #[cfg(test)]
    encodes: usize,
}

impl<const EVENTS: bool> History<EVENTS> {
    pub(super) fn push(&mut self, mut row: Value) {
        let started = Instant::now();
        if EVENTS {
            if let Some(object) = row.as_object_mut() {
                object.insert("event_index".into(), Value::from(self.rows.len()));
            } else {
                self.failure
                    .get_or_insert_with(|| "production event row is not an object".into());
            }
        }
        // Keep the actual row even on failure, never silently drop evidence or
        // turn a later publication into success after an encoding/cap error.
        self.rows.push(row);
        if self.failure.is_none() {
            let result = serde_json::to_vec(self.rows.last().expect("just appended"));
            #[cfg(test)]
            {
                self.encodes += 1;
            }
            match result {
                Ok(bytes) => {
                    let separator = usize::from(self.rows.len() > 1);
                    if total_len(&[self.encoded.len(), separator, bytes.len()], BYTE_CAP).is_err() {
                        self.failure = Some("production observation byte cap exceeded".into());
                    } else {
                        if separator == 1 {
                            self.encoded.push(b',');
                        }
                        self.encoded.extend_from_slice(&bytes);
                    }
                }
                Err(error) => self.failure = Some(error.to_string()),
            }
        }
        self.encoding_time += started.elapsed();
    }

    pub(super) fn encoding_time(&self) -> Duration {
        self.encoding_time
    }
}

// There is deliberately no DerefMut, mutable iterator, or replacement API.
impl<const EVENTS: bool> Deref for History<EVENTS> {
    type Target = [Value];
    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl<const EVENTS: bool> Extend<Value> for History<EVENTS> {
    fn extend<T: IntoIterator<Item = Value>>(&mut self, rows: T) {
        for row in rows {
            self.push(row);
        }
    }
}

fn total_len(lengths: &[usize], cap: usize) -> Result<usize, String> {
    lengths.iter().try_fold(0usize, |sum, &length| {
        sum.checked_add(length)
            .filter(|&total| total <= cap)
            .ok_or_else(|| "production observation byte cap exceeded".into())
    })
}

pub(super) fn encode_record(
    header: &Value,
    samples: &History<false>,
    events: &History<true>,
    cap: usize,
) -> Result<Vec<u8>, String> {
    if let Some(failure) = samples.failure.as_ref().or(events.failure.as_ref()) {
        return Err(failure.clone());
    }
    let object = header
        .as_object()
        .ok_or("production observation header is not an object")?;
    if object.contains_key("samples") || object.contains_key("events") {
        return Err("production observation header contains history".into());
    }
    let mut prefix = serde_json::to_vec(header).map_err(|error| error.to_string())?;
    assert_eq!(prefix.pop(), Some(b'}'));
    let sample_key: &[u8] = if object.is_empty() {
        b"\"samples\":["
    } else {
        b",\"samples\":["
    };
    let event_key = b"],\"events\":[";
    let length = total_len(
        &[
            prefix.len(),
            sample_key.len(),
            samples.encoded.len(),
            event_key.len(),
            events.encoded.len(),
            2,
        ],
        cap.min(BYTE_CAP),
    )?;
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(sample_key);
    bytes.extend_from_slice(&samples.encoded);
    bytes.extend_from_slice(event_key);
    bytes.extend_from_slice(&events.encoded);
    bytes.extend_from_slice(b"]}");
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn append_and_republication_preserve_every_row_and_never_reencode_old_rows() {
        let mut samples = History::<false>::default();
        let mut events = History::<true>::default();
        for index in 0..1200 {
            samples.push(json!({"sample_index":index,"text":"escape\n\"日本語","owners":[]}));
        }
        events.extend(
            (0..4096).map(
                |index| json!({"event_index":9999,"sample_index":index/4,"operation":"Pressed"}),
            ),
        );
        let original_samples = samples.encoded.clone();
        let original_events = events.encoded.clone();
        for failure in [Value::Null, json!("sticky failure")] {
            let header = json!({"failure":failure,"accepted":false,"promotion_authority":false});
            let bytes = encode_record(&header, &samples, &events, BYTE_CAP).unwrap();
            let mut expected = header;
            expected["samples"] = Value::from(samples.to_vec());
            expected["events"] = Value::from(events.to_vec());
            assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
            assert_eq!(samples.encodes, 1200);
            assert_eq!(events.encodes, 4096);
        }
        samples.push(json!({"sample_index":1200}));
        events.push(json!({"operation":"Released"}));
        assert!(samples.encoded.starts_with(&original_samples));
        assert!(events.encoded.starts_with(&original_events));
        assert_eq!(events[4096]["event_index"], 4096);
        assert_eq!(samples.len(), 1201); // observer owns the existing row caps
        assert_eq!(events.len(), 4097); // overflow is not silently dropped here
    }

    #[test]
    fn exact_finished_byte_boundary_and_overflow_are_rejected() {
        let samples = History::default();
        let events = History::default();
        for header in [json!({}), json!({"failure":null,"accepted":false})] {
            let bytes = encode_record(&header, &samples, &events, BYTE_CAP).unwrap();
            assert!(encode_record(&header, &samples, &events, bytes.len()).is_ok());
            assert!(encode_record(&header, &samples, &events, bytes.len() - 1).is_err());
            assert!(encode_record(&header, &samples, &events, 0).is_err());
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap()["samples"],
                json!([])
            );
        }
        assert!(total_len(&[usize::MAX, 1], usize::MAX).is_err());
        for header in [json!(null), json!({"samples":[]}), json!({"events":[]})] {
            assert!(encode_record(&header, &samples, &events, BYTE_CAP).is_err());
        }
    }

    #[test]
    fn bad_event_encoding_is_sticky_and_cannot_publish_partial_success() {
        let samples = History::default();
        let mut events = History::default();
        events.push(json!({"operation":"before"}));
        let prior = events.encoded.clone();
        events.push(Value::Null);
        events.push(json!({"operation":"after"}));
        assert_eq!(events.len(), 3);
        assert_eq!(events.encoded, prior);
        assert!(encode_record(&json!({"accepted":false}), &samples, &events, BYTE_CAP).is_err());
    }

    #[test]
    fn cumulative_append_byte_overflow_is_sticky_without_losing_prior_bytes() {
        let mut samples = History::default();
        let events = History::default();
        samples.push(json!({"prefix":"y".repeat(128)}));
        let prior = samples.encoded.clone();
        // This row is individually below64MiB, but cannot fit after prefix.
        samples.push(json!({"x":"x".repeat(BYTE_CAP-32)}));
        samples.push(json!({"sample_index":2}));
        assert_eq!(samples.len(), 3);
        assert_eq!(samples.encoded, prior);
        assert_eq!(samples.encodes, 2);
        assert_eq!(
            encode_record(&json!({"failure":null}), &samples, &events, BYTE_CAP).unwrap_err(),
            "production observation byte cap exceeded"
        );
    }
}
