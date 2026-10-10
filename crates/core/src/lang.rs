//! French, laid over the English data.
//!
//! The French files in data/fr/ hold only words: for each list (missions,
//! costumes, tricks, venues, legs) the fields to replace, keyed by the item's
//! id, and a few whole values (the trick-name templates). Everything else, the
//! props, the goals, the numbers, comes from the English files, so the two
//! languages cannot drift apart in anything but their words. The copy file is
//! whole in both languages; a test holds their keys equal.

use serde_json::{Map, Value};

/// The English data with an overlay's words laid over it. `lists` names the
/// arrays whose items carry an `id`.
pub fn overlay(base: &str, over: &str, lists: &[&str]) -> Result<String, String> {
    let mut b: Value = serde_json::from_str(base).map_err(|e| e.to_string())?;
    let o: Value = serde_json::from_str(over).map_err(|e| e.to_string())?;
    let Value::Object(om) = &o else {
        return Err("an overlay is an object".into());
    };
    for (key, val) in om {
        if key.starts_with('_') {
            continue;
        }
        if lists.contains(&key.as_str()) {
            let Some(Value::Array(items)) = b.get_mut(key) else {
                return Err(format!("no list {key}"));
            };
            let Value::Object(by_id) = val else {
                return Err(format!("{key}: words by id"));
            };
            for item in items.iter_mut() {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if let (Some(Value::Object(words)), Value::Object(fields)) = (by_id.get(&id), item) {
                    for (f, w) in words {
                        fields.insert(f.clone(), w.clone());
                    }
                }
            }
        } else if let Value::Object(bm) = &mut b {
            bm.insert(key.clone(), val.clone());
        }
    }
    serde_json::to_string(&b).map_err(|e| e.to_string())
}

/// Ids in an English list that the overlay gives no words for.
pub fn missing(base: &str, over: &str, list: &str) -> Vec<String> {
    let b: Value = serde_json::from_str(base).unwrap_or(Value::Null);
    let o: Value = serde_json::from_str(over).unwrap_or(Value::Null);
    let empty = Map::new();
    let by_id = o.get(list).and_then(|v| v.as_object()).unwrap_or(&empty);
    b.get(list)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.get("id").and_then(|v| v.as_str()))
                .filter(|id| !by_id.contains_key(*id))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Decimal points between digits become commas: 2.25 → 2,25.
pub fn decimal_commas(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for i in 0..c.len() {
        let between = i > 0 && i + 1 < c.len() && c[i - 1].is_ascii_digit() && c[i + 1].is_ascii_digit();
        out.push(if c[i] == '.' && between { ',' } else { c[i] });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(v: &Value, at: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    if !k.starts_with('_') {
                        keys(x, &format!("{at}.{k}"), out);
                    }
                }
            }
            Value::Array(a) => out.push(format!("{at}[{}]", a.len())),
            _ => out.push(at.to_string()),
        }
    }

    #[test]
    fn the_french_copy_has_every_key_the_english_has_and_no_other() {
        let en: Value = serde_json::from_str(include_str!("../../../data/copy.json")).unwrap();
        let fr: Value = serde_json::from_str(include_str!("../../../data/fr/copy.json")).unwrap();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        keys(&en, "", &mut a);
        keys(&fr, "", &mut b);
        let only_en: Vec<_> = a.iter().filter(|k| !b.contains(k)).collect();
        let only_fr: Vec<_> = b.iter().filter(|k| !a.contains(k)).collect();
        assert!(
            only_en.is_empty() && only_fr.is_empty(),
            "English only: {only_en:?}\nFrench only: {only_fr:?}"
        );
    }

    #[test]
    fn every_mission_costume_trick_venue_and_leg_has_french_words() {
        let pairs = [
            (
                include_str!("../../../data/missions.json"),
                include_str!("../../../data/fr/missions.json"),
                "missions",
            ),
            (
                include_str!("../../../data/costumes.json"),
                include_str!("../../../data/fr/costumes.json"),
                "costumes",
            ),
            (
                include_str!("../../../data/tricks.json"),
                include_str!("../../../data/fr/tricks.json"),
                "tricks",
            ),
            (
                include_str!("../../../data/venues.json"),
                include_str!("../../../data/fr/venues.json"),
                "venues",
            ),
            (
                include_str!("../../../data/venues.json"),
                include_str!("../../../data/fr/venues.json"),
                "legs",
            ),
        ];
        for (en, fr, list) in pairs {
            let m = missing(en, fr, list);
            assert!(m.is_empty(), "{list}: no French for {m:?}");
        }
    }

    #[test]
    fn an_overlay_replaces_words_and_keeps_everything_else() {
        let base = r#"{"missions":[{"id":"a","name":"Up","goal":{"kind":"x"}}],"note":"en"}"#;
        let over = r#"{"missions":{"a":{"name":"Haut"}},"note":"fr"}"#;
        let v: Value = serde_json::from_str(&overlay(base, over, &["missions"]).unwrap()).unwrap();
        assert_eq!(v["missions"][0]["name"], "Haut");
        assert_eq!(v["missions"][0]["goal"]["kind"], "x");
        assert_eq!(v["note"], "fr");
    }

    #[test]
    fn decimal_points_become_commas_only_between_digits() {
        assert_eq!(decimal_commas("2.25 m, x = -1.5."), "2,25 m, x = -1,5.");
        assert_eq!(decimal_commas("art/venue/fr/rtp.png"), "art/venue/fr/rtp.png");
    }
}
