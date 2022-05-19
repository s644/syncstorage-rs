use std::collections::{BTreeMap, HashMap};

use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
use serde_json::value::Value;
use slog::{Key, Record, KV};

#[derive(Clone, Debug, Default)]
pub struct Tags {
    pub tags: HashMap<String, String>,
    pub extra: HashMap<String, String>,
}

impl Serialize for Tags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_map(Some(self.tags.len()))?;
        for tag in self.tags.clone() {
            if !tag.1.is_empty() {
                seq.serialize_entry(&tag.0, &tag.1)?;
            }
        }
        seq.end()
    }
}

/// Tags are extra data to be recorded in metric and logging calls.
///
/// If additional tags are required or desired, you will need to add them to the
/// mutable extensions, e.g.
/// ```compile_fail
///      let mut tags = Tags::default();
///      tags.add_tag("SomeLabel", "whatever");
///      tags.commit(&mut request.extensions_mut());
/// ```
impl Tags {
    pub fn extend(&mut self, new_tags: Self) {
        self.tags.extend(new_tags.tags);
        self.extra.extend(new_tags.extra);
    }

    pub fn with_tags(tags: HashMap<String, String>) -> Tags {
        if tags.is_empty() {
            return Tags::default();
        }
        Tags {
            tags,
            extra: HashMap::new(),
        }
    }

    pub fn with_tag(key: &str, value: &str) -> Self {
        let mut tags = Tags::default();

        tags.tags.insert(key.to_owned(), value.to_owned());

        tags
    }

    pub fn add_extra(&mut self, key: &str, value: &str) {
        if !value.is_empty() {
            self.extra.insert(key.to_owned(), value.to_owned());
        }
    }

    pub fn add_tag(&mut self, key: &str, value: &str) {
        if !value.is_empty() {
            self.tags.insert(key.to_owned(), value.to_owned());
        }
    }

    pub fn get(&self, label: &str) -> String {
        let none = "None".to_owned();
        self.tags.get(label).map(String::from).unwrap_or(none)
    }

    pub fn tag_tree(self) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();

        for (k, v) in self.tags {
            result.insert(k.clone(), v.clone());
        }
        result
    }

    pub fn extra_tree(self) -> BTreeMap<String, Value> {
        let mut result = BTreeMap::new();

        for (k, v) in self.extra {
            result.insert(k.clone(), Value::from(v));
        }
        result
    }
}

impl From<Tags> for BTreeMap<String, String> {
    fn from(tags: Tags) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();

        for (k, v) in tags.tags {
            result.insert(k.clone(), v.clone());
        }
        result
    }
}

impl KV for Tags {
    fn serialize(&self, _rec: &Record<'_>, serializer: &mut dyn slog::Serializer) -> slog::Result {
        for (key, val) in &self.tags {
            serializer.emit_str(Key::from(key.clone()), val)?;
        }
        Ok(())
    }
}
