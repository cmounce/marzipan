use compact_str::CompactString;
use rustc_hash::{FxHashMap, FxHashSet};

pub struct Registry {
    key_to_suffix: FxHashMap<Lowercase, Suffix>,
    names: FxHashSet<Lowercase>,
    anonymous_counter: CompactString,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Lowercase(CompactString);

impl Lowercase {
    fn new(key: &str) -> Self {
        let mut result = CompactString::new(key);
        result.make_ascii_lowercase();
        Self(result)
    }
}

#[derive(Default)]
struct Suffix(CompactString);

impl Suffix {
    fn apply(&self, key: &str) -> CompactString {
        let mut result = preferred_label_name(key);
        result.push_str(&self.0);
        result
    }
}

impl Registry {
    pub fn new() -> Self {
        let builtin_labels = ["bombed", "energize", "shot", "thud", "touch"];
        let mut key_to_suffix = FxHashMap::default();
        let mut taken = FxHashSet::default();
        for name in builtin_labels {
            let name = Lowercase(name.into());
            key_to_suffix.insert(name.clone(), Suffix::default());
            taken.insert(name);
        }
        Self {
            key_to_suffix,
            names: taken,
            anonymous_counter: CompactString::const_new(""),
        }
    }

    pub fn sanitize(&mut self, key: &str) -> CompactString {
        // Use existing sanitization if one exists
        let key_lower = Lowercase::new(key);
        if let Some(transform) = self.key_to_suffix.get(&key_lower) {
            return transform.apply(key);
        }

        // Append suffixes until we find a name that's not taken yet
        let mut candidate = Lowercase::new(&preferred_label_name(key));
        let mut suffix = CompactString::const_new("");
        let original_len = candidate.0.len();
        loop {
            candidate.0.push_str(&suffix);
            if !self.names.contains(&candidate) {
                self.key_to_suffix.insert(key_lower, Suffix(suffix));
                self.names.insert(candidate.clone());
                break;
            } else {
                candidate.0.truncate(original_len);
                increment(&mut suffix);
            }
        }
        candidate.0
    }

    pub fn gen_anonymous(&mut self) -> CompactString {
        increment(&mut self.anonymous_counter);
        while self
            .names
            .contains(&Lowercase(self.anonymous_counter.clone()))
        {
            increment(&mut self.anonymous_counter);
        }
        self.anonymous_counter.clone()
    }
}

/// Given the key string identifying a label, generate the first-pick name we'd
/// like to assign it. (Results returned by this function are subject to veto if
/// they are already taken.)
fn preferred_label_name(key: &str) -> CompactString {
    // Extract the last part of the key: "ns~global$123.local" becomes "local"
    let base_name = key
        .rsplit_once(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(|(_, base_name)| base_name)
        .unwrap_or(key);

    // Collapse runs of numeric digits/underscores to a single underscore
    let mut result = CompactString::default();
    let mut run = false;
    for c in base_name.chars() {
        if c.is_ascii_alphabetic() {
            result.push(c);
            run = false;
        } else if !run {
            result.push('_');
            run = true;
        }
    }
    result
}

/// Increments a string through label-safe characters.
///
/// The characters tick upward in order (underscore, a-z), odometer style,
/// growing the string on overflow. The result is similar to counting in
/// base 27, but it's not quite the same:
///
/// - The zeroth value is the empty string "" (not "_").
/// - "z" is followed by "__", "_a", "_b", etc (not "a_", "aa", "ab").
///
/// Unlike in place-value systems where "0001" and "1" are equivalent, the goal
/// of this function is to generate every possible string.
fn increment(s: &mut CompactString) {
    let original_len = s.len();

    // Find a character we can increment without carry
    let mut last_char = s.pop();
    while last_char == Some('z') {
        last_char = s.pop();
    }

    if let Some(c) = last_char {
        // Increment character
        let incremented = match c {
            'a'..'z' => (c as u8 + 1) as char,
            '_' => 'a',
            _ => unreachable!(),
        };
        s.push(incremented);

        // Pad to original length
        while s.len() < original_len {
            s.push('_');
        }
    } else {
        // All existing chars roll over, length grows by 1
        for _ in 0..(original_len + 1) {
            s.push('_');
        }
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
        _, a, b, c, d, e, f, g, h, i
        j, k, l, m, n, o, p, q, r, s
        t, u, v, w, x, y, z, __, _a, _b
        _c, _d, _e, _f, _g, _h, _i, _j, _k, _l
        _m, _n, _o, _p, _q, _r, _s, _t, _u, _v
        _w, _x, _y, _z, a_, aa, ab, ac, ad, ae
        af, ag, ah, ai, aj, ak, al, am, an, ao
        ap, aq, ar, as, at, au, av, aw, ax, ay
        az, b_, ba, bb, bc, bd, be, bf, bg, bh
        bi, bj, bk, bl, bm, bn, bo, bp, bq, br
        ");
    }

    #[test]
    fn test_increment_all_len_3() {
        let num_labels = 27 + 27 * 27 + 27 * 27 * 27; // 1 char + 2 chars + 3 chars
        let mut ctr = CompactString::new("");
        let mut seen = HashSet::with_capacity(num_labels);
        for _ in 0..num_labels {
            increment(&mut ctr);
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
        "ns1~foo" => foo_
        "ns2~foo" => fooa
        "foo.thisloop" => thisloop
        "foo.thatloop" => thatloop
        "bar.thisloop" => thisloop_
        "bar.thatloop" => thatloop_
        "foo$1.thisloop" => thisloopa
        "foo$1.thatloop" => thatloopa
        "bar1" => bar_
        "bar2" => bar__
        "bar123" => bar_a
        "BAR123" => BAR_a
        "bar456" => bar_b
        "BAR456" => BAR_b
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
