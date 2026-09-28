//! Minimal, dependency-free regular-expression engine for the XSD `pattern`
//! facet.
//!
//! C++ `XSDValidator` delegates `pattern` checks to `std::regex`
//! (`std::regex_match`, i.e. *full* match). This crate is deliberately
//! dependency-free (no external crates), so this module supplies the subset of
//! regex syntax that XML Schema patterns actually use. A pattern is parsed into
//! an AST, compiled to an NFA (Thompson construction), and simulated with a set
//! of active states — linear in the input length, with no backtracking blow-up.
//!
//! Supported syntax:
//! - literals, `.` (any char), escaped metacharacters (`\.` `\[` …)
//! - character classes `[...]` with ranges (`[a-z]`) and negation (`[^0-9]`)
//! - predefined classes `\d \D \w \W \s \S`
//! - quantifiers `* + ? {n} {n,} {n,m}`
//! - alternation `|` and groups `( … )` (incl. `(?: … )`)
//! - anchors `^` / `$` (redundant under full match)
//!
//! `\p{…}` / `\P{…}` (Unicode property classes) are accepted but matched
//! permissively (as "any character") to avoid false negatives; Unicode
//! properties, lookaround and backreferences are not supported.

use std::collections::BTreeSet;

/// One member of a character class.
#[derive(Debug, Clone)]
enum ClassItem {
    /// A single character.
    Single(char),
    /// An inclusive range `lo..=hi`.
    Range(char, char),
}

/// A single-character matcher.
#[derive(Debug, Clone)]
enum Matcher {
    /// `.` — any character.
    Any,
    /// A literal character.
    Char(char),
    /// A character class, optionally negated.
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
    /// Permissive fallback for unsupported constructs (matches anything).
    Permissive,
}

impl Matcher {
    fn matches(&self, c: char) -> bool {
        match self {
            Matcher::Any | Matcher::Permissive => true,
            Matcher::Char(x) => *x == c,
            Matcher::Class { negated, items } => {
                let hit = items.iter().any(|it| match it {
                    ClassItem::Single(x) => *x == c,
                    ClassItem::Range(a, b) => *a <= c && c <= *b,
                });
                hit != *negated
            }
        }
    }
}

/// A parsed regular-expression node.
#[derive(Debug, Clone)]
enum Re {
    /// Matches the empty string.
    Empty,
    /// Matches one character.
    Match(Matcher),
    /// A sequence of sub-expressions.
    Concat(Vec<Re>),
    /// An alternation of sub-expressions.
    Alt(Vec<Re>),
    /// A quantified sub-expression: `min` required, `max` optional (`None` = unbounded).
    Repeat {
        node: Box<Re>,
        min: u32,
        max: Option<u32>,
    },
}

/// A member produced while scanning a character class.
enum ClassChar {
    /// A single literal / escaped character.
    One(char),
    /// A predefined class expanded to items (`\d`, `\w`, `\s`).
    Set(Vec<ClassItem>),
}

fn word_items() -> Vec<ClassItem> {
    vec![
        ClassItem::Range('a', 'z'),
        ClassItem::Range('A', 'Z'),
        ClassItem::Range('0', '9'),
        ClassItem::Single('_'),
    ]
}

fn space_items() -> Vec<ClassItem> {
    vec![
        ClassItem::Single(' '),
        ClassItem::Single('\t'),
        ClassItem::Single('\n'),
        ClassItem::Single('\r'),
        ClassItem::Single('\u{0c}'),
    ]
}

/// Recursive-descent parser for the supported regex subset.
struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(pattern: &str) -> Self {
        Self {
            chars: pattern.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn parse(&mut self) -> Re {
        self.parse_alt()
    }

    fn parse_alt(&mut self) -> Re {
        let mut alts = vec![self.parse_concat()];
        while self.peek() == Some('|') {
            self.pos += 1;
            alts.push(self.parse_concat());
        }
        if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            Re::Alt(alts)
        }
    }

    fn parse_concat(&mut self) -> Re {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            items.push(self.parse_repeat());
        }
        match items.len() {
            0 => Re::Empty,
            1 => items.pop().unwrap(),
            _ => Re::Concat(items),
        }
    }

    fn parse_repeat(&mut self) -> Re {
        let atom = self.parse_atom();
        match self.peek() {
            Some('*') => {
                self.pos += 1;
                Re::Repeat {
                    node: Box::new(atom),
                    min: 0,
                    max: None,
                }
            }
            Some('+') => {
                self.pos += 1;
                Re::Repeat {
                    node: Box::new(atom),
                    min: 1,
                    max: None,
                }
            }
            Some('?') => {
                self.pos += 1;
                Re::Repeat {
                    node: Box::new(atom),
                    min: 0,
                    max: Some(1),
                }
            }
            Some('{') => {
                let save = self.pos;
                match self.try_bounds() {
                    Some((min, max)) => Re::Repeat {
                        node: Box::new(atom),
                        min,
                        max,
                    },
                    None => {
                        // Not a valid `{m,n}` quantifier: rewind so the `{` is
                        // re-parsed as a literal atom on the next iteration.
                        self.pos = save;
                        atom
                    }
                }
            }
            _ => atom,
        }
    }

    /// Parse `{m}`, `{m,}` or `{m,n}` (cursor is at `{`).
    fn try_bounds(&mut self) -> Option<(u32, Option<u32>)> {
        self.pos += 1; // consume '{'
        let min = self.parse_uint()?;
        match self.peek() {
            Some(',') => {
                self.pos += 1;
                let max = self.parse_uint();
                if self.peek() == Some('}') {
                    self.pos += 1;
                    Some((min, max))
                } else {
                    None
                }
            }
            Some('}') => {
                self.pos += 1;
                Some((min, Some(min)))
            }
            _ => None,
        }
    }

    fn parse_uint(&mut self) -> Option<u32> {
        let start = self.pos;
        let mut n: u32 = 0;
        while let Some(c) = self.peek() {
            if let Some(d) = c.to_digit(10) {
                n = n.saturating_mul(10).saturating_add(d);
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            None
        } else {
            Some(n)
        }
    }

    fn parse_atom(&mut self) -> Re {
        match self.bump() {
            None => Re::Empty,
            Some('(') => {
                // Tolerate `(?: … )`; other `(?…` prefixes are treated as plain groups
                // (lookaround is unsupported).
                if self.peek() == Some('?') {
                    self.pos += 1;
                    if self.peek() == Some(':') {
                        self.pos += 1;
                    }
                }
                let inner = self.parse_alt();
                if self.peek() == Some(')') {
                    self.pos += 1;
                }
                inner
            }
            Some('[') => self.parse_class(),
            Some('.') => Re::Match(Matcher::Any),
            Some('^') | Some('$') => Re::Empty,
            Some('\\') => self.parse_escape(),
            Some(c) => Re::Match(Matcher::Char(c)),
        }
    }

    fn parse_escape(&mut self) -> Re {
        match self.bump() {
            Some('d') => Re::Match(class(false, vec![ClassItem::Range('0', '9')])),
            Some('D') => Re::Match(class(true, vec![ClassItem::Range('0', '9')])),
            Some('w') => Re::Match(class(false, word_items())),
            Some('W') => Re::Match(class(true, word_items())),
            Some('s') => Re::Match(class(false, space_items())),
            Some('S') => Re::Match(class(true, space_items())),
            Some('p') | Some('P') => {
                // `\p{…}` / `\P{…}`: consume the braced name, match permissively.
                if self.peek() == Some('{') {
                    self.pos += 1;
                    while let Some(c) = self.bump() {
                        if c == '}' {
                            break;
                        }
                    }
                }
                Re::Match(Matcher::Permissive)
            }
            Some(c) => Re::Match(Matcher::Char(c)),
            None => Re::Empty,
        }
    }

    fn parse_class(&mut self) -> Re {
        let negated = if self.peek() == Some('^') {
            self.pos += 1;
            true
        } else {
            false
        };
        let mut items = Vec::new();
        let mut at_start = true;
        loop {
            match self.peek() {
                None => break,
                // A ']' immediately after '[' (or '[^') is a literal.
                Some(']') if !at_start => {
                    self.pos += 1;
                    break;
                }
                _ => {}
            }
            at_start = false;
            match self.class_char() {
                ClassChar::One(ch) => {
                    let next = self.chars.get(self.pos + 1).copied();
                    if self.peek() == Some('-') && next != Some(']') && next.is_some() {
                        self.pos += 1; // consume '-'
                        match self.class_char() {
                            ClassChar::One(hi) => items.push(ClassItem::Range(ch, hi)),
                            ClassChar::Set(set) => {
                                items.push(ClassItem::Single(ch));
                                items.extend(set);
                            }
                        }
                    } else {
                        items.push(ClassItem::Single(ch));
                    }
                }
                ClassChar::Set(set) => items.extend(set),
            }
        }
        Re::Match(class(negated, items))
    }

    fn class_char(&mut self) -> ClassChar {
        match self.bump() {
            Some('\\') => match self.bump() {
                Some('d') => ClassChar::Set(vec![ClassItem::Range('0', '9')]),
                Some('w') => ClassChar::Set(word_items()),
                Some('s') => ClassChar::Set(space_items()),
                Some(c) => ClassChar::One(c),
                None => ClassChar::One('\\'),
            },
            Some(c) => ClassChar::One(c),
            None => ClassChar::One('\0'),
        }
    }
}

fn class(negated: bool, items: Vec<ClassItem>) -> Matcher {
    Matcher::Class { negated, items }
}

// ==== NFA (Thompson construction) ====

#[derive(Debug, Clone)]
enum Edge {
    /// Epsilon transition.
    Eps(usize),
    /// Consume a matching character, then move to the target state.
    Consume(Matcher, usize),
}

#[derive(Debug, Default)]
struct Nfa {
    states: Vec<Vec<Edge>>,
    start: usize,
    accept: usize,
}

impl Nfa {
    fn new_state(&mut self) -> usize {
        self.states.push(Vec::new());
        self.states.len() - 1
    }
}

/// Compile `re` into `nfa`, returning its `(start, accept)` states.
fn compile(re: &Re, nfa: &mut Nfa) -> (usize, usize) {
    match re {
        Re::Empty => {
            let s = nfa.new_state();
            (s, s)
        }
        Re::Match(m) => {
            let s = nfa.new_state();
            let e = nfa.new_state();
            nfa.states[s].push(Edge::Consume(m.clone(), e));
            (s, e)
        }
        Re::Concat(items) => {
            if items.is_empty() {
                let s = nfa.new_state();
                return (s, s);
            }
            let (start, mut end) = compile(&items[0], nfa);
            for it in &items[1..] {
                let (s2, e2) = compile(it, nfa);
                nfa.states[end].push(Edge::Eps(s2));
                end = e2;
            }
            (start, end)
        }
        Re::Alt(alts) => {
            let s = nfa.new_state();
            let e = nfa.new_state();
            for a in alts {
                let (s2, e2) = compile(a, nfa);
                nfa.states[s].push(Edge::Eps(s2));
                nfa.states[e2].push(Edge::Eps(e));
            }
            (s, e)
        }
        Re::Repeat { node, min, max } => compile_repeat(node, *min, *max, nfa),
    }
}

fn compile_repeat(node: &Re, min: u32, max: Option<u32>, nfa: &mut Nfa) -> (usize, usize) {
    let start = nfa.new_state();
    let mut cur = start;
    // Mandatory copies.
    for _ in 0..min {
        let (a, b) = compile(node, nfa);
        nfa.states[cur].push(Edge::Eps(a));
        cur = b;
    }
    match max {
        None => {
            let (a, b) = compile(node, nfa);
            let end = nfa.new_state();
            nfa.states[cur].push(Edge::Eps(a));
            nfa.states[cur].push(Edge::Eps(end));
            nfa.states[b].push(Edge::Eps(a));
            nfa.states[b].push(Edge::Eps(end));
            (start, end)
        }
        Some(m) => {
            let extra = m.saturating_sub(min);
            let end = nfa.new_state();
            nfa.states[cur].push(Edge::Eps(end));
            for _ in 0..extra {
                let (a, b) = compile(node, nfa);
                nfa.states[cur].push(Edge::Eps(a));
                cur = b;
                nfa.states[cur].push(Edge::Eps(end));
            }
            (start, end)
        }
    }
}

fn eps_closure(nfa: &Nfa, s: usize, out: &mut BTreeSet<usize>) {
    if !out.insert(s) {
        return;
    }
    for e in &nfa.states[s] {
        if let Edge::Eps(t) = e {
            eps_closure(nfa, *t, out);
        }
    }
}

/// Whether `value` fully matches `pattern` (C++ `std::regex_match` semantics).
///
/// An invalid or unsupported pattern never panics; unsupported constructs are
/// matched permissively so validation fails open rather than reporting a bogus
/// mismatch.
pub fn regex_full_match(pattern: &str, value: &str) -> bool {
    let re = Parser::new(pattern).parse();
    let mut nfa = Nfa::default();
    let (start, accept) = compile(&re, &mut nfa);
    nfa.start = start;
    nfa.accept = accept;

    let mut cur = BTreeSet::new();
    eps_closure(&nfa, start, &mut cur);
    for ch in value.chars() {
        let mut next = BTreeSet::new();
        for &s in &cur {
            for e in &nfa.states[s] {
                if let Edge::Consume(m, t) = e {
                    if m.matches(ch) {
                        next.insert(*t);
                    }
                }
            }
        }
        cur.clear();
        for &s in &next {
            eps_closure(&nfa, s, &mut cur);
        }
    }
    cur.contains(&nfa.accept)
}

#[cfg(test)]
mod tests {
    use super::regex_full_match;

    #[test]
    fn exact_quantifier() {
        assert!(regex_full_match("[0-9]{10,13}", "1234567890"));
        assert!(regex_full_match("[0-9]{10,13}", "1234567890123"));
        assert!(!regex_full_match("[0-9]{10,13}", "123456789")); // too short
        assert!(!regex_full_match("[0-9]{10,13}", "12345678901234")); // too long
    }

    #[test]
    fn classes_and_ranges() {
        assert!(regex_full_match("[a-c]+", "abcabc"));
        assert!(!regex_full_match("[a-c]+", "abd"));
        assert!(regex_full_match("[^0-9]", "x"));
        assert!(!regex_full_match("[^0-9]", "7"));
    }

    #[test]
    fn alternation_and_groups() {
        assert!(regex_full_match("colou?r", "color"));
        assert!(regex_full_match("colou?r", "colour"));
        assert!(!regex_full_match("colou?r", "colr"));
        assert!(regex_full_match("(cat|dog)s?", "dogs"));
        assert!(!regex_full_match("(cat|dog)s?", "cows"));
    }

    #[test]
    fn predefined_classes() {
        assert!(regex_full_match("\\d{3}", "042"));
        assert!(!regex_full_match("\\d{3}", "04a"));
        assert!(regex_full_match("\\w+", "a_1"));
        assert!(!regex_full_match("\\w+", "a-1"));
        assert!(regex_full_match("a\\sb", "a b"));
    }

    #[test]
    fn anchors_and_dot() {
        assert!(regex_full_match("^ab$", "ab"));
        assert!(regex_full_match(".", "x"));
        assert!(!regex_full_match(".", ""));
        assert!(regex_full_match("a.*z", "abcz"));
    }

    #[test]
    fn unsupported_unicode_property_fails_open() {
        // `\p{...}` is matched permissively (any char) rather than erroring.
        assert!(regex_full_match("\\p{L}+", "abc"));
        assert!(regex_full_match("\\p{L}+", "123"));
    }

    #[test]
    fn open_ended_and_zero() {
        assert!(regex_full_match("a{2,}", "aaaa"));
        assert!(!regex_full_match("a{2,}", "a"));
        assert!(regex_full_match("a{0}", ""));
        assert!(!regex_full_match("a{0}", "a"));
    }
}
