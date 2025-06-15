use std::collections::{HashMap, HashSet};

use compact_str::CompactString;

use super::parse::LabelName;

#[derive(PartialEq, Eq, Hash)]
struct LabelId(CompactString);

impl LabelId {
    fn new(label: &LabelName) -> Self {
        let mut result = CompactString::with_capacity(0);
        if let Some(namespace) = &label.namespace {
            result.push_str(&namespace);
            result.push('~');
        }
        result.push_str(&label.name);
        if let Some(local) = &label.local {
            result.push('.');
            result.push_str(&local);
        }
        result.make_ascii_lowercase();
        LabelId(result)
    }
}

pub struct Registry {
    label_transforms: HashMap<LabelId, Transform>,
    existing: HashSet<CompactString>,
}

enum Transform {
    Preferred,
    FilteredWithSuffix(CompactString),
}

impl Transform {
    fn apply(&self, label: &LabelName) -> CompactString {
        match self {
            Transform::Preferred => Registry::preferred_name(label),
            Transform::FilteredWithSuffix(suffix) => {
                let mut result = Registry::preferred_name(label);
                result = Registry::filter_label_chars(&result);
                result.push_str(&suffix);
                result
            }
        }
    }
}

impl Registry {
    pub fn new() -> Self {
        let builtin_labels = ["bombed", "energize", "shot", "thud", "touch"];
        let mut label_transforms = HashMap::new();
        let mut existing = HashSet::new();
        for name in builtin_labels {
            let cs = CompactString::new(name);
            label_transforms.insert(LabelId(cs.clone()), Transform::Preferred);
            existing.insert(cs);
        }
        Self {
            label_transforms,
            existing,
        }
    }

    pub fn sanitize(&mut self, label: &LabelName) -> CompactString {
        // Use existing sanitization if one exists
        let id = LabelId::new(label);
        if let Some(transform) = self.label_transforms.get(&id) {
            return transform.apply(&label);
        }

        // Try to use label's preferred name as-is
        let preferred = Transform::Preferred.apply(label);
        if Registry::is_valid_label(&preferred) {
            let key = preferred.to_ascii_lowercase();
            if !self.existing.contains(&key) {
                self.label_transforms.insert(id, Transform::Preferred);
                self.existing.insert(key);
                return preferred;
            }
        }

        // Generate a new name by appending suffixes
        let base = Registry::filter_label_chars(&preferred);
        let base_len = base.len();
        let mut candidate = base;
        let mut suffix = CompactString::with_capacity(0);
        loop {
            candidate.push_str(&suffix);
            let key = candidate.to_ascii_lowercase();
            if !self.existing.contains(&key) {
                self.label_transforms
                    .insert(id, Transform::FilteredWithSuffix(suffix));
                self.existing.insert(key);
                break;
            } else {
                candidate.truncate(base_len);
                increment(&mut suffix);
            }
        }
        candidate
    }

    fn preferred_name(label: &LabelName) -> CompactString {
        let src = if let Some(local) = &label.local {
            local
        } else {
            &label.name
        };
        CompactString::new(src)
    }

    fn is_valid_label(s: &CompactString) -> bool {
        if s.len() == 0 {
            return false;
        }
        let (most, last) = s.split_at(s.len() - 1);
        if !most.chars().all(|c| c == '_' || c.is_ascii_alphabetic()) {
            return false;
        }
        last.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
    }

    fn filter_label_chars(s: &CompactString) -> CompactString {
        let mut result = CompactString::with_capacity(0);
        let mut run_of_digits = false;
        for c in s.chars() {
            if c.is_ascii_digit() {
                if !run_of_digits {
                    result.push('_');
                    run_of_digits = true;
                }
            } else {
                result.push(c);
                run_of_digits = false;
            }
        }
        result
    }
}

/// Increments a string through label-safe characters.
/// Characters loop through underscore and the letters a-z.
/// Additionally, the final character is allowed to loop through 0-9.
fn increment(s: &mut CompactString) {
    let original_len = s.len();

    // Find a character we can increment without carry
    dbg!(&s);
    let mut last_char = s.pop();
    while !(last_char == None || last_char != Some('z')) {
        last_char = s.pop();
    }
    dbg!(s.len(), last_char);

    if let Some(c) = last_char {
        // Increment character in order: 0-9, then _, then a-z
        let incremented = match c {
            '0'..'9' | 'a'..'z' => (c as u8 + 1) as char,
            '9' => '_',
            '_' => 'a',
            _ => unreachable!(),
        };
        s.push(incremented);

        // Pad to original length, like "___0"
        while s.len() < original_len {
            let is_last = s.len() == original_len - 1;
            s.push(if is_last { '0' } else { '_' });
        }
    } else {
        // We reached maximum value for the string: everything rolls over
        for _ in 0..original_len {
            s.push('_');
        }
        s.push('0');
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use compact_str::CompactString;
    use insta::assert_snapshot;

    use crate::labels::{parse::LabelName, sanitize::Registry};

    use super::increment;

    #[test]
    fn test_increment_100() {
        let mut rows = vec![];
        let mut ctr = CompactString::new("");
        for _ in 0..10 {
            let mut row = vec![];
            for _ in 0..10 {
                increment(&mut ctr);
                row.push(ctr.to_string());
            }
            rows.push(row.join(", "));
        }
        let result = rows.join("\n");
        assert_snapshot!(result, @r"
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9
        _, a, b, c, d, e, f, g, h, i
        j, k, l, m, n, o, p, q, r, s
        t, u, v, w, x, y, z, _0, _1, _2
        _3, _4, _5, _6, _7, _8, _9, __, _a, _b
        _c, _d, _e, _f, _g, _h, _i, _j, _k, _l
        _m, _n, _o, _p, _q, _r, _s, _t, _u, _v
        _w, _x, _y, _z, a0, a1, a2, a3, a4, a5
        a6, a7, a8, a9, a_, aa, ab, ac, ad, ae
        af, ag, ah, ai, aj, ak, al, am, an, ao
        ");
    }

    #[test]
    fn test_increment_all_len_3() {
        // Most chars are alphabetic or underscore (26 + 1 = 27).
        // Last char can include digits (27 + 10 = 37).
        let num_labels = 37 + 27 * 37 + 27 * 27 * 37; // 1 char + 2 chars + 3 chars
        let mut ctr = CompactString::new("");
        let mut seen = HashSet::with_capacity(num_labels);
        for _ in 0..num_labels {
            increment(&mut ctr);
            assert!(Registry::is_valid_label(&ctr));
            assert!(!seen.contains(&ctr));
            assert!(ctr.len() <= 3);
            seen.insert(ctr.clone());
        }

        // Make sure we exhausted all possible length-3 labels
        increment(&mut ctr);
        assert_eq!(ctr.len(), 4);
    }

    #[test]
    fn test_sanitize_simple() {
        let mut reg = Registry::new();
        let mut results = vec![];
        let simple = |s: &str| -> LabelName {
            LabelName {
                name: s.into(),
                namespace: None,
                local: None,
            }
        };
        let full = |ns: &str, name: &str, local: &str| -> LabelName {
            let mut result = LabelName {
                namespace: None,
                name: name.into(),
                local: None,
            };
            if ns.len() > 0 {
                result.namespace = Some(ns.into());
            }
            if local.len() > 0 {
                result.local = Some(local.into());
            }
            result
        };
        let inputs = [
            simple("foo"),
            full("alt", "foo", ""),
            full("", "foo", "thisloop"),
            full("", "foo", "thatloop"),
            full("", "bar", "thisloop"),
            full("", "bar", "thatloop"),
            simple("bar1"),
            simple("bar2"),
            simple("bar123"),
            simple("BAR123"),
            simple("bar456"),
            simple("BAR456"),
            simple("foo2bar"),
        ];
        for label in inputs {
            let sanitized = reg.sanitize(&label);
            assert_eq!(sanitized, reg.sanitize(&label));
            results.push(format!("{:?} => {}", &label, sanitized));
        }
        let result = results.join("\n");
        assert_snapshot!(result, @r#"
        LabelName { namespace: None, name: "foo", local: None } => foo
        LabelName { namespace: Some("alt"), name: "foo", local: None } => foo0
        LabelName { namespace: None, name: "foo", local: Some("thisloop") } => thisloop
        LabelName { namespace: None, name: "foo", local: Some("thatloop") } => thatloop
        LabelName { namespace: None, name: "bar", local: Some("thisloop") } => thisloop0
        LabelName { namespace: None, name: "bar", local: Some("thatloop") } => thatloop0
        LabelName { namespace: None, name: "bar1", local: None } => bar1
        LabelName { namespace: None, name: "bar2", local: None } => bar2
        LabelName { namespace: None, name: "bar123", local: None } => bar_
        LabelName { namespace: None, name: "BAR123", local: None } => BAR_
        LabelName { namespace: None, name: "bar456", local: None } => bar_0
        LabelName { namespace: None, name: "BAR456", local: None } => BAR_0
        LabelName { namespace: None, name: "foo2bar", local: None } => foo_bar
        "#);
    }
}
