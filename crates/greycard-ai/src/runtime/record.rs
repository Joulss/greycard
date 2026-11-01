//! Whether a provider can run a given model file, remembered on disk
//! beside the model cache: `<store root>/providers.json`, a sibling
//! of the per-model directories `store.rs` already keeps. So a WebGPU
//! (or CUDA) failure that will repeat every launch — Dawn accepting
//! BiRefNet's graph and then dying on a Split node, as the notes
//! record in §97 — is paid once, not once a launch. Deleting the file
//! clears every remembered answer; nothing else reads or writes it.
//!
//! One entry per model file and provider: a model with more than one
//! graph (SAM's encoder and decoder are two separate files under one
//! registry id) gets a slot each, since a session build can accept
//! one and not the other. The slot is the file's published hash
//! (known from the registry before the file is opened, so nothing
//! here re-hashes a 100 MB file to check the cache) together with the
//! provider; a model update ships under a new hash, so its old entry
//! is simply never matched again, not overwritten in place — a stale
//! row is harmless and small, and deleting the file drops all of
//! them. What else makes an entry still apply, beyond being the exact
//! file: the adapter that would run it, and the build's own set of
//! providers together with its version (a `cuda` feature added, a
//! different set of providers available this run, or a greycard
//! build with a fixed ONNX Runtime, are each a different question),
//! checked by `valid` rather than folded into the slot, so a driver
//! update or a rebuild replaces the one entry for a file and provider
//! instead of leaving an orphan beside it.
//!
//! Success is remembered as well as failure. A success still gets
//! its warm-up (see `runtime::open`), but where Core ML was raced
//! against a GPU provider each side's timed run is kept with it
//! (`Race`), so the next load opens the winner first and races no more.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::runtime::Provider;

/// What a provider did with a model, last time it was tried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum Outcome {
    Worked,
    Failed { reason: String },
}

/// One remembered answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The registry's id for the model, e.g. `birefnet-lite-2024-fp16`.
    pub model: String,
    /// The specific file's published sha256.
    pub hash: String,
    /// `Provider::name()`.
    pub provider: String,
    /// The adapter's identity, as far as it could be read (see
    /// `runtime::Provider::adapter_identity`).
    pub adapter: String,
    /// The build's provider set and version (`build_fingerprint`).
    pub build: String,
    #[serde(flatten)]
    pub outcome: Outcome,
    /// The timed warm run, where the provider was raced against another
    /// (Core ML against the GPU provider after it, `runtime::open`), so
    /// the next load can open the winner alone. Absent in a record
    /// written before there was one, and for a provider never raced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race: Option<Race>,
}

/// One provider's side of a race: how long its timed warm run took, and
/// the shape that run was at, since a winner at one tile need not win
/// at another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Race {
    /// The free dimensions fixed for the session (`shape`), empty for a
    /// model of static shape.
    pub shape: String,
    /// The warm run, in microseconds.
    pub run_us: u64,
}

/// The fixed dimensions as a race records them: `h=608,w=608`.
pub fn shape(fixed: &[(&str, i64)]) -> String {
    fixed
        .iter()
        .map(|(name, size)| format!("{name}={size}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Contents {
    entries: Vec<Entry>,
}

fn contents(store_root: &Path) -> Option<Contents> {
    let text = std::fs::read_to_string(path(store_root)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Where the record lives for a store rooted at `store_root`.
pub fn path(store_root: &Path) -> PathBuf {
    store_root.join("providers.json")
}

/// The remembered entries, or none if the file is missing, unreadable
/// or will not parse: this is only ever worth a slower launch, never
/// an error.
pub fn read(store_root: &Path) -> Vec<Entry> {
    contents(store_root).map(|c| c.entries).unwrap_or_default()
}

/// Write `entries` back, beside and renamed so a crash leaves nothing
/// that reads as a record. The temp name carries this process's id
/// and a counter of its own, so the editor and a CLI run (or two
/// editor windows) writing at once never tear each other's temp file
/// — the rename itself is still a plain last-write-wins, which is
/// only ever worth a slower next launch for whichever write lost.
/// Best effort throughout; removes its temp file on any failure.
pub fn write(store_root: &Path, entries: &[Entry]) {
    let contents = Contents {
        entries: entries.to_vec(),
    };
    let out = path(store_root);
    let Some(parent) = out.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(text) = serde_json::to_string_pretty(&contents) else {
        return;
    };
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!("providers.json.{}.{n}.partial", std::process::id()));
    if std::fs::write(&tmp, text).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if std::fs::rename(&tmp, &out).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// The remembered entry for this exact file (by its hash) and
/// provider, if there is one.
pub fn find<'a>(entries: &'a [Entry], hash: &str, provider: Provider) -> Option<&'a Entry> {
    entries
        .iter()
        .find(|e| e.hash == hash && e.provider == provider.name())
}

/// Whether `entry` was made under the same adapter and build as now,
/// so it can be trusted without a fresh probe. The file itself is
/// already exact: `find` matched on its hash.
pub fn valid(entry: &Entry, adapter: &str, build: &str) -> bool {
    entry.adapter == adapter && entry.build == build
}

/// Replace the slot for `entry`'s file and provider, or add it.
pub fn upsert(entries: &mut Vec<Entry>, entry: Entry) {
    match entries
        .iter_mut()
        .find(|e| e.hash == entry.hash && e.provider == entry.provider)
    {
        Some(slot) => *slot = entry,
        None => entries.push(entry),
    }
}

/// Names the build well enough that a different one does not trust
/// this build's answer: the providers this build tried, in order, the
/// crate's own version, and `ort::info()`'s build string (branch,
/// commit, build type — cheap: it reads a string ONNX Runtime already
/// built in, no session or environment needed), so a greycard build
/// carrying a fixed or regressed ONNX Runtime is a different question
/// even at the same `CARGO_PKG_VERSION`. Asked only once the runtime
/// is loaded (`runtime::open` and `runtime::remembered_failure` see
/// to it): `ort::info` reads the library.
pub fn build_fingerprint(providers: &[Provider]) -> String {
    let names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
    format!(
        "{} greycard {} {}",
        names.join(","),
        env!("CARGO_PKG_VERSION"),
        ort::info()
    )
}

/// An `Entry` with an adapter and build given directly, for a test —
/// here and in `runtime`'s — that wants one without asking real
/// hardware for it. `#[cfg(test)]`, not built into the crate.
#[cfg(test)]
pub(crate) fn test_entry(
    model: &str,
    hash: &str,
    provider: Provider,
    adapter: &str,
    build: &str,
    outcome: Outcome,
) -> Entry {
    Entry {
        model: model.to_string(),
        hash: hash.to_string(),
        provider: provider.name().to_string(),
        adapter: adapter.to_string(),
        build: build.to_string(),
        outcome,
        race: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(model: &str, hash: &str, provider: Provider, outcome: Outcome) -> Entry {
        test_entry(
            model,
            hash,
            provider,
            "Test GPU|Vulkan|1.2.3",
            "WebGPU,CPU greycard 0.1.0",
            outcome,
        )
    }

    #[test]
    fn a_round_trip_keeps_the_entries() {
        let dir = std::env::temp_dir().join(format!(
            "greycard-provider-record-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(read(&dir).is_empty());

        let entries = vec![
            entry(
                "birefnet-lite-2024-fp16",
                "abc123",
                Provider::WebGpu,
                Outcome::Worked,
            ),
            entry(
                "big-lama-carve-fp32",
                "def456",
                Provider::WebGpu,
                Outcome::Failed {
                    reason: "Split node".to_string(),
                },
            ),
        ];
        write(&dir, &entries);
        assert!(path(&dir).is_file());
        let back = read(&dir);
        assert_eq!(back, entries);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A record written before races were kept reads, its entries
    /// with no race; one with a race keeps it through a round trip.
    #[test]
    fn a_race_is_optional_in_the_file_and_kept_when_there() {
        let old = r#"{"entries":[{"model":"m","hash":"h","provider":"CoreML",
            "adapter":"Apple M5|macOS 27.0.1","build":"b","outcome":"worked"}]}"#;
        let back: Contents = serde_json::from_str(old).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].race, None);
        assert_eq!(back.entries[0].outcome, Outcome::Worked);

        let mut e = entry("m", "h", Provider::CoreMl, Outcome::Worked);
        e.race = Some(Race {
            shape: shape(&[("h", 608), ("w", 608)]),
            run_us: 206_000,
        });
        let text = serde_json::to_string(&Contents {
            entries: vec![e.clone()],
        })
        .unwrap();
        assert!(text.contains(r#""shape":"h=608,w=608""#), "{text}");
        let back: Contents = serde_json::from_str(&text).unwrap();
        assert_eq!(back.entries, vec![e]);
        // No race, no key: an entry never raced writes as it did.
        let plain =
            serde_json::to_string(&entry("m", "h", Provider::WebGpu, Outcome::Worked)).unwrap();
        assert!(!plain.contains("race"), "{plain}");
    }

    #[test]
    fn a_missing_or_broken_file_reads_as_empty() {
        let dir = std::env::temp_dir().join(format!(
            "greycard-provider-record-missing-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(read(&dir).is_empty());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(path(&dir), b"not json").unwrap();
        assert!(read(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_matches_the_files_hash_and_provider() {
        let entries = vec![
            entry(
                "sam2.1-hiera-small-fp32",
                "encoder-hash",
                Provider::WebGpu,
                Outcome::Worked,
            ),
            entry(
                "sam2.1-hiera-small-fp32",
                "decoder-hash",
                Provider::WebGpu,
                Outcome::Worked,
            ),
        ];
        assert_eq!(
            find(&entries, "encoder-hash", Provider::WebGpu),
            Some(&entries[0])
        );
        assert_eq!(
            find(&entries, "decoder-hash", Provider::WebGpu),
            Some(&entries[1])
        );
        assert_eq!(find(&entries, "encoder-hash", Provider::Cpu), None);
        assert_eq!(find(&entries, "no-such-hash", Provider::WebGpu), None);
    }

    #[test]
    fn upsert_replaces_the_same_slot_and_adds_a_new_one() {
        let mut entries = vec![entry(
            "birefnet-lite-2024-fp16",
            "abc123",
            Provider::WebGpu,
            Outcome::Worked,
        )];
        upsert(
            &mut entries,
            entry(
                "birefnet-lite-2024-fp16",
                "abc123",
                Provider::WebGpu,
                Outcome::Failed {
                    reason: "Split node".to_string(),
                },
            ),
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].outcome,
            Outcome::Failed {
                reason: "Split node".to_string()
            }
        );
        // A different file (a model update's new hash) is a new slot,
        // not a replacement: the old one is simply never matched
        // again.
        upsert(
            &mut entries,
            entry(
                "birefnet-lite-2024-fp16",
                "def456",
                Provider::WebGpu,
                Outcome::Worked,
            ),
        );
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn a_changed_adapter_or_build_invalidates_the_entry() {
        let e = entry(
            "birefnet-lite-2024-fp16",
            "abc123",
            Provider::WebGpu,
            Outcome::Worked,
        );
        assert!(valid(
            &e,
            "Test GPU|Vulkan|1.2.3",
            "WebGPU,CPU greycard 0.1.0"
        ));
        assert!(!valid(
            &e,
            "Other GPU|Metal|9.9.9",
            "WebGPU,CPU greycard 0.1.0"
        ));
        assert!(!valid(
            &e,
            "Test GPU|Vulkan|1.2.3",
            "CUDA,WebGPU,CPU greycard 0.1.0"
        ));
    }

    #[test]
    fn a_models_two_files_do_not_share_a_slot() {
        // SAM's encoder and decoder share a registry id but are two
        // files: one failing WebGPU must not be read for the other.
        let mut entries = Vec::new();
        upsert(
            &mut entries,
            entry(
                "sam2.1-hiera-small-fp32",
                "encoder-hash",
                Provider::WebGpu,
                Outcome::Failed {
                    reason: "x".to_string(),
                },
            ),
        );
        upsert(
            &mut entries,
            entry(
                "sam2.1-hiera-small-fp32",
                "decoder-hash",
                Provider::WebGpu,
                Outcome::Worked,
            ),
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(
            find(&entries, "encoder-hash", Provider::WebGpu).map(|e| &e.outcome),
            Some(&Outcome::Failed {
                reason: "x".to_string()
            })
        );
        assert_eq!(
            find(&entries, "decoder-hash", Provider::WebGpu).map(|e| &e.outcome),
            Some(&Outcome::Worked)
        );
    }

    /// Reads the loaded runtime's build string: only where it can be
    /// had (scripts/ort-dylib.sh has run on a processor it suits).
    #[test]
    fn the_build_fingerprint_names_the_providers_in_order() {
        if let Err(why) = crate::runtime::init() {
            crate::runtime::absent_in_test(why);
            return;
        }
        let f = build_fingerprint(&[Provider::WebGpu, Provider::Cpu]);
        assert!(f.starts_with("WebGPU,CPU"));
        assert!(f.contains(env!("CARGO_PKG_VERSION")));
        assert_ne!(f, build_fingerprint(&[Provider::Cpu, Provider::WebGpu]));
    }
}
