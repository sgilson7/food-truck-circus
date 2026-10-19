//! Copy tests (Builder D15, D16), adopted from the Builder template for the
//! strings this project added under the privacy kit (SECOND-ORDER, the
//! blocked-device message). They read GLOSSARY.md, TONE.md and every string
//! in data/, and fail when the words drift.
//!
//! Scope: the strings whose key starts with `blocked_`. The older strings in
//! data/ were written before this project had a glossary or a tone file, and
//! holding them to it is a separate change (SECOND-ORDER).
//!
//! These read text, not meaning. A slogan or a claim the tool cannot back
//! passes them; the person's reading covers those.

use std::fs;
use std::path::{Path, PathBuf};

/// The key prefix of the strings these tests hold to the glossary and tone.
const SCOPE: &str = "blocked_";

/// Tone rule 2: under 25 words a sentence.
const MAX_WORDS: usize = 25;

/// The host a school's IT allows; every blocked message names it.
const HOST: &str = "sgilson7.github.io";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn read(name: &str) -> String {
    fs::read_to_string(root().join(name)).unwrap_or_else(|e| panic!("cannot read {name}: {e}"))
}

fn data_json_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root().join("data")];
    while let Some(d) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else {
            continue;
        };
        for entry in read {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().map(|x| x == "json").unwrap_or(false) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// (key path, last key, text) for every string value.
fn strings_in(value: &serde_json::Value, at: &str, key: &str, out: &mut Vec<(String, String, String)>) {
    match value {
        serde_json::Value::String(s) => out.push((at.to_string(), key.to_string(), s.clone())),
        serde_json::Value::Array(items) => {
            for (k, v) in items.iter().enumerate() {
                strings_in(v, &format!("{at}[{k}]"), key, out);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if !k.starts_with('_') {
                    strings_in(v, &format!("{at}.{k}"), k, out);
                }
            }
        }
        _ => {}
    }
}

/// (where, text) for every reader-visible string in scope.
fn scoped_strings() -> Vec<(String, String)> {
    let mut all = Vec::new();
    for path in data_json_files() {
        let text = fs::read_to_string(&path).expect("read data file");
        let value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()));
        let name = path.strip_prefix(root()).unwrap_or(&path).display().to_string();
        strings_in(&value, &name, "", &mut all);
    }
    all.into_iter()
        .filter(|(_, key, _)| key.starts_with(SCOPE))
        .map(|(at, _, text)| (at, text))
        .collect()
}

/// The phrases listed between the lint markers in TONE.md, one per bullet.
fn tone_phrases() -> Vec<String> {
    let tone = read("TONE.md");
    let start = tone
        .find("<!-- lint:start -->")
        .expect("TONE.md has no <!-- lint:start --> marker");
    let end = tone
        .find("<!-- lint:end -->")
        .expect("TONE.md has no <!-- lint:end --> marker");
    tone[start..end]
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- "))
        .map(|p| p.trim().trim_matches('`').to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// (term, definition, ruled-out words) for each `### Term` entry in GLOSSARY.md.
fn glossary() -> Vec<(String, String, Vec<String>)> {
    let text = read("GLOSSARY.md");
    let mut out: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut in_comment = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("<!--") {
            in_comment = !l.ends_with("-->");
            continue;
        }
        if in_comment {
            in_comment = !l.ends_with("-->");
            continue;
        }
        if let Some(term) = l.strip_prefix("### ") {
            out.push((term.trim().to_string(), String::new(), Vec::new()));
        } else if let Some(last) = out.last_mut() {
            if let Some(words) = l.strip_prefix("Not:") {
                last.2 = words
                    .split(',')
                    .map(|w| w.trim().to_string())
                    .filter(|w| !w.is_empty())
                    .collect();
            } else if !l.is_empty() {
                last.1 = format!("{} {l}", last.1).trim().to_string();
            }
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\''
}

/// Whether `text` contains `phrase`, ignoring case; a wordy phrase must stand
/// as whole words.
fn contains_phrase(text: &str, phrase: &str) -> bool {
    let hay = text.to_lowercase();
    let needle = phrase.to_lowercase();
    if needle.is_empty() {
        return false;
    }
    let wordy = needle.chars().next().map(is_word_char).unwrap_or(false)
        && needle.chars().last().map(is_word_char).unwrap_or(false);
    let mut from = 0;
    while let Some(at) = hay[from..].find(&needle) {
        let start = from + at;
        let end = start + needle.len();
        let before = hay[..start].chars().last();
        let after = hay[end..].chars().next();
        if !wordy || (!before.map(is_word_char).unwrap_or(false) && !after.map(is_word_char).unwrap_or(false))
        {
            return true;
        }
        from = start + needle.chars().next().map(char::len_utf8).unwrap_or(1);
    }
    false
}

/// The sentences of a string: split after `.`, `?` or `!` followed by a space
/// (so `sgilson7.github.io` stays one word).
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        cur.push(*c);
        let next_is_space = chars.get(i + 1).map(|n| n.is_whitespace()).unwrap_or(true);
        if matches!(c, '.' | '?' | '!') && next_is_space {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

#[test]
fn the_blocked_device_messages_exist_and_name_the_host() {
    let strings = scoped_strings();
    assert!(
        !strings.is_empty(),
        "no string with a key starting {SCOPE} in data/"
    );
    for (place, text) in &strings {
        assert!(
            text.contains(HOST),
            "{place}: names no host for a school's IT to allow ({HOST}): {text}"
        );
    }
}

#[test]
fn every_glossary_entry_has_a_definition() {
    let entries = glossary();
    assert!(!entries.is_empty(), "GLOSSARY.md has no entry");
    for (term, definition, _) in entries {
        assert!(!definition.is_empty(), "GLOSSARY.md: {term} has no definition");
    }
}

#[test]
fn no_scoped_string_uses_a_word_the_glossary_rules_out() {
    let entries = glossary();
    let mut problems = Vec::new();
    for (place, text) in scoped_strings() {
        for (term, _, not) in &entries {
            for word in not {
                if contains_phrase(&text, word) {
                    problems.push(format!(
                        "{place}: \"{word}\" is ruled out for \"{term}\" in GLOSSARY.md: {text}"
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "glossary words:\n{}", problems.join("\n"));
}

#[test]
fn no_scoped_string_uses_a_phrase_the_tone_file_rules_out() {
    let phrases = tone_phrases();
    let mut problems = Vec::new();
    for (place, text) in scoped_strings() {
        for p in &phrases {
            if contains_phrase(&text, p) {
                problems.push(format!("{place}: \"{p}\" (TONE.md): {text}"));
            }
        }
    }
    assert!(problems.is_empty(), "tone:\n{}", problems.join("\n"));
}

#[test]
fn every_scoped_sentence_is_short() {
    let mut problems = Vec::new();
    for (place, text) in scoped_strings() {
        for s in sentences(&text) {
            let words = s.split_whitespace().count();
            if words >= MAX_WORDS {
                problems.push(format!("{place}: {words} words (TONE.md rule 2): {s}"));
            }
        }
    }
    assert!(problems.is_empty(), "long sentences:\n{}", problems.join("\n"));
}

// ---- The lint is a test too. ----

#[test]
fn the_tone_lint_catches_each_listed_phrase() {
    let phrases = tone_phrases();
    assert!(!phrases.is_empty(), "TONE.md lists no phrases to lint");
    for p in &phrases {
        let sample = format!("Here we {p} the plan.");
        assert!(contains_phrase(&sample, p), "the lint misses {p:?}");
    }
}

#[test]
fn phrase_matching_respects_word_edges() {
    assert!(contains_phrase("We delve into it.", "delve"));
    assert!(contains_phrase("Done!", "!"));
    assert!(!contains_phrase("The landscaper came.", "landscape"));
}

#[test]
fn sentences_split_at_full_stops_but_not_inside_a_host() {
    assert_eq!(sentences("Allow sgilson7.github.io. Then reload.").len(), 2);
}
