//! Loaded YAML documents a delta compile keeps by content digest instead of parsing again.

use crate::compile_context::models::DocumentStore;
use std::path::PathBuf;

use sqlbuild_cache::digest::main::bytes_digest::bytes_digest;
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::models::NativeStore;
use sqlbuild_config::models::{ConfigDate, ConfigDateTime, ConfigTime, ConfigValue};
use sqlbuild_core::text::main::decode_text::decode_text;
use sqlbuild_discovery::yaml_files::models::YamlFileOutcome;

use crate::compile_context::_helpers::delta::injection;
use crate::compile_context::models::Decoder;
use crate::compile_context::models::Encoder;

/// The source YAML key that lists sources, and the key naming each source.
const SOURCES_KEY: &str = "sources";
const NAME_KEY: &str = "name";

/// The native store kind holding loaded YAML documents.
pub(crate) const YAML_STORE_KIND: &str = "yaml-documents";
/// The store file beside the compile reuse slots.
pub(crate) const YAML_STORE_FILE_NAME: &str = "yaml-documents.bin";
/// Bumped whenever the stored encoding changes; older stores open empty.
pub(crate) const YAML_STORE_ENVIRONMENT: &str = "yaml-documents-v1";

const NULL: usize = 0;
const BOOL: usize = 1;
const INTEGER: usize = 2;
const BIG_INTEGER: usize = 3;
const FLOAT: usize = 4;
const STRING: usize = 5;
const DATE: usize = 6;
const DATE_TIME: usize = 7;
const TIME: usize = 8;
const LIST: usize = 9;
const MAP: usize = 10;
const NO_OFFSET: usize = 0;
const OFFSET: usize = 1;

/// Where a delta compile keeps loaded YAML documents.
pub(crate) fn yaml_store_path(cache_dir: &std::path::Path) -> PathBuf {
    cache_dir.join(YAML_STORE_FILE_NAME)
}

/// The store key of a document's text.
pub(crate) fn document_key(contents: &str) -> ContentDigest {
    bytes_digest(contents.as_bytes())
}

impl<'a> Encoder<'a> {
    fn date(&mut self, date: &ConfigDate) {
        self.count(usize::from(date.year));
        self.count(usize::from(date.month));
        self.count(usize::from(date.day));
    }

    fn time(&mut self, time: &ConfigTime) {
        self.count(usize::from(time.hour));
        self.count(usize::from(time.minute));
        self.count(usize::from(time.second));
        self.bytes(&time.microsecond.to_le_bytes());
    }

    fn document(&mut self, value: &'a ConfigValue) {
        match value {
            ConfigValue::Null => self.count(NULL),
            ConfigValue::Bool(flag) => {
                self.count(BOOL);
                self.flag(*flag);
            }
            ConfigValue::Integer(number) => {
                self.count(INTEGER);
                self.bytes(&number.to_le_bytes());
            }
            ConfigValue::BigInteger(digits) => {
                self.count(BIG_INTEGER);
                self.string(digits);
            }
            ConfigValue::Float(number) => {
                self.count(FLOAT);
                self.bytes(&number.to_bits().to_le_bytes());
            }
            ConfigValue::String(text) => {
                self.count(STRING);
                self.string(text);
            }
            ConfigValue::Date(date) => {
                self.count(DATE);
                self.date(date);
            }
            ConfigValue::DateTime(moment) => {
                self.count(DATE_TIME);
                self.date(&moment.date);
                self.time(&moment.time);
                match moment.utc_offset_seconds {
                    Some(offset) => {
                        self.count(OFFSET);
                        self.bytes(&offset.to_le_bytes());
                    }
                    None => self.count(NO_OFFSET),
                }
            }
            ConfigValue::Time(time) => {
                self.count(TIME);
                self.time(time);
            }
            ConfigValue::List(items) => {
                self.count(LIST);
                self.count(items.len());
                for item in items {
                    self.document(item);
                }
            }
            ConfigValue::Map(entries) => {
                self.count(MAP);
                self.count(entries.len());
                for (key, item) in entries {
                    self.document(key);
                    self.document(item);
                }
            }
        }
    }
}

/// A loaded document as stored.
pub(crate) fn encoded_document(value: &ConfigValue) -> Vec<u8> {
    let mut encoder: Encoder<'_> = Encoder::default();
    encoder.document(value);
    encoder.finish()
}

impl Decoder<'_> {
    fn fixed<const N: usize>(&mut self) -> Option<[u8; N]> {
        let Ok(bytes) = self.bytes()?.try_into() else {
            return None;
        };
        Some(bytes)
    }

    fn small<T: TryFrom<usize>>(&mut self) -> Option<T> {
        let Ok(value) = T::try_from(self.count()?) else {
            return None;
        };
        Some(value)
    }

    fn date(&mut self) -> Option<ConfigDate> {
        Some(ConfigDate {
            year: self.small()?,
            month: self.small()?,
            day: self.small()?,
        })
    }

    fn time(&mut self) -> Option<ConfigTime> {
        Some(ConfigTime {
            hour: self.small()?,
            minute: self.small()?,
            second: self.small()?,
            microsecond: u32::from_le_bytes(self.fixed()?),
        })
    }

    fn document(&mut self) -> Option<ConfigValue> {
        Some(match self.count()? {
            NULL => ConfigValue::Null,
            BOOL => ConfigValue::Bool(self.flag()?),
            INTEGER => ConfigValue::Integer(i64::from_le_bytes(self.fixed()?)),
            BIG_INTEGER => ConfigValue::BigInteger(self.string()?),
            FLOAT => ConfigValue::Float(f64::from_bits(u64::from_le_bytes(self.fixed()?))),
            STRING => ConfigValue::String(self.string()?),
            DATE => ConfigValue::Date(self.date()?),
            DATE_TIME => ConfigValue::DateTime(ConfigDateTime {
                date: self.date()?,
                time: self.time()?,
                utc_offset_seconds: match self.count()? {
                    NO_OFFSET => None,
                    OFFSET => Some(i32::from_le_bytes(self.fixed()?)),
                    _ => return None,
                },
            }),
            TIME => ConfigValue::Time(self.time()?),
            LIST => ConfigValue::List(self.list(Self::document)?),
            MAP => ConfigValue::Map(
                self.list(|decoder| Some((decoder.document()?, decoder.document()?)))?,
            ),
            _ => return None,
        })
    }
}

/// A stored document, or `None` when it cannot be decoded.
pub(crate) fn decoded_document(stored: &[u8]) -> Option<ConfigValue> {
    let mut decoder: Decoder<'_> = Decoder::new(stored)?;
    let document: ConfigValue = decoder.document()?;
    decoder.finish()?;
    Some(document)
}

/// In a delta compile, the document store beside its reuse slots, when it opens.
pub(crate) fn open_documents(py: pyo3::Python<'_>) -> Option<DocumentStore> {
    let path: PathBuf = crate::compile_context::_helpers::delta::injection()?
        .yaml_store
        .clone()?;
    match crate::cache::main::open_store::open_store(
        py,
        &path,
        YAML_STORE_KIND,
        YAML_STORE_ENVIRONMENT,
    ) {
        Ok(store) => Some(DocumentStore { store, path }),
        Err(_unopened) => None,
    }
}

/// The stored document of the file at `path` when its current text is stored.
pub(crate) fn stored_outcome(
    store: &mut NativeStore,
    path: &std::path::Path,
) -> Option<YamlFileOutcome> {
    let bytes: Vec<u8> = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_unreadable) => return None,
    };
    let contents: String = match decode_text(&bytes) {
        Ok(contents) => contents,
        Err(_undecodable) => return None,
    };
    let value: ConfigValue = decoded_document(store.get(&document_key(&contents))?)?;
    Some(YamlFileOutcome::Loaded { contents, value })
}

/// Save the documents this compile loaded; failing to only means parsing them again.
pub(crate) fn save_documents(py: pyo3::Python<'_>, documents: &DocumentStore) {
    let _ = crate::cache::main::save_store::save_store(py, &documents.store, &documents.path, &[]);
}

/// In a delta reading only some sources, drop every other loaded source YAML entry.
pub(crate) fn retain_read_sources(
    mut outcomes: Vec<sqlbuild_discovery::yaml_files::models::YamlFileOutcome>,
) -> Vec<sqlbuild_discovery::yaml_files::models::YamlFileOutcome> {
    use sqlbuild_config::models::ConfigValue;
    use sqlbuild_discovery::yaml_files::models::YamlFileOutcome;
    let Some(injection) = injection() else {
        return outcomes;
    };
    let Some(read) = injection.source_names.as_ref() else {
        return outcomes;
    };
    fn text(value: &ConfigValue) -> Option<&str> {
        match value {
            ConfigValue::String(text) => Some(text.as_str()),
            _ => None,
        }
    }
    for outcome in &mut outcomes {
        let YamlFileOutcome::Loaded {
            value: ConfigValue::Map(entries),
            ..
        } = outcome
        else {
            continue;
        };
        for (key, value) in entries {
            if text(key) != Some(SOURCES_KEY) {
                continue;
            }
            let ConfigValue::List(sources) = value else {
                continue;
            };
            sources.retain(|source| {
                let name: Option<&str> = source.as_map().and_then(|entries| {
                    entries
                        .iter()
                        .find(|(key, _)| text(key) == Some(NAME_KEY))
                        .and_then(|(_, name)| text(name))
                });
                name.is_none_or(|name| read.contains(name))
            });
        }
    }
    outcomes
}
