use std::collections::{HashMap, HashSet};

use compact_str::CompactString;

#[derive(PartialEq, Eq, Hash)]
struct LabelId(CompactString);

impl LabelId {
    fn new(label: &str) -> Self {
        let mut result = CompactString::new(label);
        result.make_ascii_lowercase();
        Self(result)
    }
}

pub struct Registry {
    label_transforms: HashMap<LabelId, Transform>,
    existing: HashSet<CompactString>,
    anonymous_counter: CompactString,
}

enum Transform {
    Preferred,
    FilteredWithSuffix(CompactString),
}

impl Transform {
    fn apply(&self, label: &str) -> CompactString {
        let preferred = preferred_label_name(label);
        match self {
            Transform::Preferred => preferred,
            Transform::FilteredWithSuffix(suffix) => {
                let mut result = preferred;
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
            anonymous_counter: "_".into(),
        }
    }

    pub fn sanitize(&mut self, label: &str) -> CompactString {
        // Use existing sanitization if one exists
        let id = LabelId::new(label);
        if let Some(transform) = self.label_transforms.get(&id) {
            return transform.apply(label);
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

    pub fn gen_anonymous(&mut self) -> CompactString {
        while self.existing.contains(&self.anonymous_counter) {
            increment(&mut self.anonymous_counter);
        }
        let result = self.anonymous_counter.clone();
        increment(&mut self.anonymous_counter);
        result
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

/// Given an unsanitized full name, generate the first-pick name we'd like to
/// assign this label. (Results returned by this function are subject to veto
/// if they are invalid or already taken.)
fn preferred_label_name(s: &str) -> CompactString {
    if let Some((_, suffix)) = s.rsplit_once(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        // Prevent namespaces and local labels from starting with a digit.
        // This ensures stuff like `#take gems 100.99orless` won't compile
        // to `#take gems 10099orless` (would parse incorrectly).
        let mut result = CompactString::const_new("_");
        result.push_str(suffix);
        result
    } else {
        CompactString::new(s)
    }
}

/// Increments a string through label-safe characters.
/// Characters loop through underscore and the letters a-z.
/// Additionally, the final character is allowed to loop through 0-9.
fn increment(s: &mut CompactString) {
    let original_len = s.len();

    // Find a character we can increment without carry
    let mut last_char = s.pop();
    while !(last_char == None || last_char != Some('z')) {
        last_char = s.pop();
    }

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

    use crate::labels::sanitize::Registry;

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
        let inputs = [
            "foo",
            "ns1~foo",
            "ns2~foo",
            "foo.thisloop",
            "foo.thatloop",
            "bar.thisloop",
            "bar.thatloop",
            "foo$1.thisloop",
            "foo$1.thatloop",
            "bar1",
            "bar2",
            "bar123",
            "BAR123",
            "bar456",
            "BAR456",
            "foo2bar",
        ];
        for label in inputs {
            let sanitized = reg.sanitize(label);
            assert_eq!(sanitized, reg.sanitize(label));
            results.push(format!("{:?} => {}", &label, sanitized));
        }
        let result = results.join("\n");
        assert_snapshot!(result, @r#"
        "foo" => foo
        "ns1~foo" => _foo
        "ns2~foo" => _foo0
        "foo.thisloop" => _thisloop
        "foo.thatloop" => _thatloop
        "bar.thisloop" => _thisloop0
        "bar.thatloop" => _thatloop0
        "foo$1.thisloop" => _thisloop1
        "foo$1.thatloop" => _thatloop1
        "bar1" => bar1
        "bar2" => bar2
        "bar123" => bar_
        "BAR123" => BAR_
        "bar456" => bar_0
        "BAR456" => BAR_0
        "foo2bar" => foo_bar
        "#);
    }

    #[test]
    fn test_gen_anonymous() {
        let mut registry = Registry::new();
        for letter in ["a", "e", "i"] {
            registry.sanitize(letter);
        }
        let result: Vec<_> = (0..10).map(|_| registry.gen_anonymous()).collect();
        let result = result.join(", ");

        // It's safer if anonymous labels don't start with a digit.
        // If one ever did, it could change `#take gems 123@f` to something
        // like `#take gems 1230`, altering how the ZZT-OOP parses.
        assert_snapshot!(result, @"_, b, c, d, f, g, h, j, k, l");
    }
}
