//! Text tools: case conversion, counting, placeholder text, line diff, CSV and JSON. Pure functions of their input, like every tool
//! (see `tools.rs`, and its S5 test which reads this file too).

use super::Output;

// ---- case ----

/// Split text into words: at anything that is not a letter or digit, and inside camelCase and acronyms ("HTMLParser" is HTML, Parser).
fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if !cur.is_empty() {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let lower_to_upper = prev.is_lowercase() && c.is_uppercase();
            let acronym_end =
                prev.is_uppercase() && c.is_uppercase() && next.is_some_and(|n| n.is_lowercase());
            if lower_to_upper || acronym_end {
                out.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn capital(w: &str) -> String {
    let mut cs = w.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
        None => String::new(),
    }
}

pub fn case(input: &str) -> Result<Output, String> {
    let w = words(input);
    if w.is_empty() {
        return Err("type or paste some text".into());
    }
    let lower: Vec<String> = w.iter().map(|x| x.to_lowercase()).collect();
    let mut o = Output::default();
    o.field(
        "camelCase",
        lower
            .iter()
            .enumerate()
            .map(|(i, x)| if i == 0 { x.clone() } else { capital(x) })
            .collect::<String>(),
    );
    o.field(
        "PascalCase",
        w.iter().map(|x| capital(x)).collect::<String>(),
    );
    o.field("snake_case", lower.join("_"));
    o.field("SCREAMING_SNAKE_CASE", lower.join("_").to_uppercase());
    o.field("kebab-case", lower.join("-"));
    o.field("dot.case", lower.join("."));
    o.field(
        "Title Case",
        w.iter().map(|x| capital(x)).collect::<Vec<_>>().join(" "),
    );
    let sentence = lower.join(" ");
    o.field(
        "Sentence case",
        capital(&sentence.chars().take(1).collect::<String>())
            + &sentence.chars().skip(1).collect::<String>(),
    );
    o.field("UPPER CASE", input.trim().to_uppercase());
    o.field("lower case", input.trim().to_lowercase());
    // a slug keeps only ASCII letters and digits
    let slug: Vec<String> = lower
        .iter()
        .map(|x| {
            x.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|x| !x.is_empty())
        .collect();
    o.field("slug", slug.join("-"));
    Ok(o)
}

// ---- count ----

pub fn count(input: &str) -> Result<Output, String> {
    if input.is_empty() {
        return Err("type or paste some text".into());
    }
    let words = input.split_whitespace().count();
    let chars = input.chars().count();
    let no_space = input.chars().filter(|c| !c.is_whitespace()).count();
    let lines = input.lines().count();
    let paragraphs = input.split("\n\n").filter(|p| !p.trim().is_empty()).count();
    let mut sentences = 0;
    let mut in_end = false;
    for c in input.chars() {
        let end = matches!(c, '.' | '!' | '?');
        if end && !in_end {
            sentences += 1;
        }
        in_end = end;
    }
    if sentences == 0 && words > 0 {
        sentences = 1;
    }
    let minutes = words.div_ceil(200);
    let mut o = Output::default();
    o.field("Words", words.to_string());
    o.field("Characters", chars.to_string());
    o.field("Characters without spaces", no_space.to_string());
    o.field("Lines", lines.to_string());
    o.field("Sentences", sentences.to_string());
    o.field("Paragraphs", paragraphs.to_string());
    o.field(
        "Reading time",
        if words == 0 {
            "none".to_string()
        } else {
            format!("about {minutes} min at 200 words a minute")
        },
    );
    Ok(o)
}

// ---- lorem ----

const LOREM: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "in",
    "reprehenderit",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "eu",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
];

/// `n` words in a fixed, repeatable order (the classic start first), so the same request always gives the same text.
fn lorem_words(n: usize, offset: usize) -> Vec<&'static str> {
    (0..n)
        .map(|i| LOREM[(offset + i * 7 + i / LOREM.len()) % LOREM.len()])
        .collect()
}

fn lorem_sentence(index: usize) -> String {
    if index == 0 {
        return "Lorem ipsum dolor sit amet, consectetur adipiscing elit.".into();
    }
    let n = 8 + (index * 5) % 9;
    let ws = lorem_words(n, index * 13);
    let mut s = ws.join(" ");
    s.replace_range(..1, &s[..1].to_uppercase());
    if n > 10 {
        // a comma after the fifth word
        let at = ws[..5].join(" ").len();
        s.insert(at, ',');
    }
    s.push('.');
    s
}

pub fn lorem(input: &str) -> Result<Output, String> {
    let t = input.trim().to_lowercase();
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let unit = t[digits.len()..].trim();
    let n: usize = if digits.is_empty() {
        1
    } else {
        digits.parse().map_err(|_| "that number is too big")?
    };
    let unit = if unit.is_empty() && !digits.is_empty() {
        "w"
    } else if unit.is_empty() {
        "p"
    } else {
        unit
    };
    let mut o = Output::default();
    match unit.chars().next() {
        Some('p') => {
            let n = n.clamp(1, 50);
            let mut s = 0;
            let paragraphs: Vec<String> = (0..n)
                .map(|_| {
                    let count = 4 + s % 3;
                    (0..count)
                        .map(|_| {
                            let x = lorem_sentence(s);
                            s += 1;
                            x
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            o.block(
                format!("{n} paragraph{}", if n == 1 { "" } else { "s" }),
                paragraphs.join("\n\n"),
            );
        }
        Some('s') => {
            let n = n.clamp(1, 200);
            o.block(
                format!("{n} sentence{}", if n == 1 { "" } else { "s" }),
                (0..n).map(lorem_sentence).collect::<Vec<_>>().join(" "),
            );
        }
        Some('w') => {
            let n = n.clamp(1, 2000);
            let mut text = lorem_words(n, 0).join(" ");
            text.replace_range(..1, &text[..1].to_uppercase());
            o.block(format!("{n} word{}", if n == 1 { "" } else { "s" }), text);
        }
        _ => {
            return Err("say how much, like 3p (paragraphs), 5s (sentences) or 50w (words)".into())
        }
    }
    Ok(o)
}

// ---- diff ----

/// Two texts separated by a line that is only `---`; a line diff of the first against the second.
pub fn diff(input: &str) -> Result<Output, String> {
    let mut a: Vec<&str> = Vec::new();
    let mut b: Vec<&str> = Vec::new();
    let mut second = false;
    for line in input.lines() {
        if !second && line.trim() == "---" {
            second = true;
        } else if second {
            b.push(line);
        } else {
            a.push(line);
        }
    }
    if !second {
        return Err("put the first text, a line with only ---, then the second text".into());
    }
    if a.len() > 1500 || b.len() > 1500 {
        return Err("that is too many lines to compare (1500 at most each)".into());
    }
    // longest common subsequence of lines
    let (n, m) = (a.len(), b.len());
    let mut t = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i * (m + 1) + j] = if a[i] == b[j] {
                t[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let (mut added, mut removed, mut same) = (0, 0, 0);
    let mut out = String::new();
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push_str(&format!("  {}\n", a[i]));
            same += 1;
            i += 1;
            j += 1;
        } else if j < m && (i == n || t[i * (m + 1) + j + 1] >= t[(i + 1) * (m + 1) + j]) {
            out.push_str(&format!("+ {}\n", b[j]));
            added += 1;
            j += 1;
        } else {
            out.push_str(&format!("- {}\n", a[i]));
            removed += 1;
            i += 1;
        }
    }
    let mut o = Output::default();
    o.field(
        "Summary",
        format!("{added} added, {removed} removed, {same} unchanged"),
    );
    if added == 0 && removed == 0 {
        o.note("The two texts are the same.");
    }
    o.block(
        "Differences (+ added, - removed)",
        out.trim_end().to_string(),
    );
    Ok(o)
}

// ---- csv <-> json ----

fn parse_csv(text: &str, delim: char) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut cell) = (Vec::new(), Vec::new(), String::new());
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' && cell.is_empty() {
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            rows.push(std::mem::take(&mut row));
        } else {
            cell.push(c);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    rows
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn csv_cell(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn csv(input: &str) -> Result<Output, String> {
    let t = input.trim();
    if t.is_empty() {
        return Err("paste CSV, TSV or a JSON array of objects".into());
    }
    let mut o = Output::default();
    if t.starts_with('[') {
        let v: serde_json::Value = serde_json::from_str(t)
            .map_err(|e| format!("not valid JSON (line {}, column {})", e.line(), e.column()))?;
        let items = v.as_array().ok_or("expected a JSON array")?;
        let mut cols: Vec<String> = Vec::new();
        for it in items {
            let m = it.as_object().ok_or("every item must be an object")?;
            for k in m.keys() {
                if !cols.contains(k) {
                    cols.push(k.clone());
                }
            }
        }
        cols.sort();
        let cell = |v: Option<&serde_json::Value>| match v {
            None | Some(serde_json::Value::Null) => String::new(),
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
        };
        let mut out = cols
            .iter()
            .map(|c| csv_cell(c, ','))
            .collect::<Vec<_>>()
            .join(",");
        for it in items {
            out.push('\n');
            out.push_str(
                &cols
                    .iter()
                    .map(|c| csv_cell(&cell(it.get(c)), ','))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        o.note("The columns are in alphabetical order.");
        o.block(format!("CSV ({} rows)", items.len()), out);
        return Ok(o);
    }
    let delim = if t.lines().next().unwrap_or("").contains('\t') {
        '\t'
    } else if t.lines().next().unwrap_or("").contains(';')
        && !t.lines().next().unwrap_or("").contains(',')
    {
        ';'
    } else {
        ','
    };
    let rows = parse_csv(t, delim);
    let header = rows.first().ok_or("there is no data")?;
    let mut items: Vec<String> = Vec::new();
    for r in rows.iter().skip(1) {
        let fields: Vec<String> = header
            .iter()
            .enumerate()
            .map(|(i, h)| {
                format!(
                    "{}: {}",
                    json_string(h),
                    json_string(r.get(i).map_or("", |s| s.as_str()))
                )
            })
            .collect();
        items.push(format!("  {{ {} }}", fields.join(", ")));
    }
    o.note("Every value is kept as text.");
    o.block(
        format!("JSON ({} rows)", items.len()),
        if items.is_empty() {
            "[]".to_string()
        } else {
            format!("[\n{}\n]", items.join(",\n"))
        },
    );
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(o: &Output, label: &str) -> String {
        o.fields
            .iter()
            .find(|f| f.label == label)
            .unwrap_or_else(|| panic!("no field {label}"))
            .value
            .clone()
    }

    #[test]
    fn case_conversion_splits_words_everywhere() {
        let o = case("HTMLParser fooBar_baz-qux 42 ok").unwrap();
        assert_eq!(value(&o, "snake_case"), "html_parser_foo_bar_baz_qux_42_ok");
        assert_eq!(value(&o, "camelCase"), "htmlParserFooBarBazQux42Ok");
        assert_eq!(value(&o, "PascalCase"), "HtmlParserFooBarBazQux42Ok");
        assert_eq!(value(&o, "kebab-case"), "html-parser-foo-bar-baz-qux-42-ok");
        assert_eq!(value(&o, "Title Case"), "Html Parser Foo Bar Baz Qux 42 Ok");
        assert_eq!(
            value(&o, "Sentence case"),
            "Html parser foo bar baz qux 42 ok"
        );
        assert_eq!(
            value(&o, "SCREAMING_SNAKE_CASE"),
            "HTML_PARSER_FOO_BAR_BAZ_QUX_42_OK"
        );
        assert_eq!(
            value(&case("Héllo, Wörld! ✓").unwrap(), "slug"),
            "hllo-wrld",
            "a slug keeps ASCII letters and digits only"
        );
        assert!(case("  ...  ").is_err());
    }

    #[test]
    fn counting() {
        let o = count("One two three. Four five!\n\nSecond paragraph?").unwrap();
        assert_eq!(value(&o, "Words"), "7");
        assert_eq!(value(&o, "Sentences"), "3");
        assert_eq!(value(&o, "Paragraphs"), "2");
        assert_eq!(value(&o, "Lines"), "3");
        assert!(value(&o, "Reading time").contains("1 min"));
        assert_eq!(value(&count("héllo").unwrap(), "Characters"), "5");
        assert!(count("").is_err());
    }

    #[test]
    fn lorem_is_repeatable_and_bounded() {
        let a = lorem("2p").unwrap().fields[0].value.clone();
        assert_eq!(a, lorem("2p").unwrap().fields[0].value);
        assert!(a.starts_with("Lorem ipsum dolor sit amet, consectetur adipiscing elit."));
        assert_eq!(a.matches("\n\n").count(), 1);
        assert_eq!(
            lorem("5").unwrap().fields[0].value.split(' ').count(),
            5,
            "a bare number is words"
        );
        let three = lorem("3 sentences").unwrap().fields[0].value.clone();
        assert_eq!(three.matches('.').count(), 3, "{three}");
        assert_eq!(lorem("").unwrap().fields[0].label, "1 paragraph");
        assert!(lorem("99999p").unwrap().fields[0].label.starts_with("50 "));
        assert!(lorem("3x").is_err());
    }

    #[test]
    fn diff_shows_what_was_added_and_removed() {
        let o = diff("a\nb\nc\n---\na\nc\nd").unwrap();
        assert_eq!(value(&o, "Summary"), "1 added, 1 removed, 2 unchanged");
        assert_eq!(o.fields[1].value, "  a\n- b\n  c\n+ d");
        assert!(diff("same\n---\nsame").unwrap().notes[0].contains("same"));
        assert!(diff("no separator").is_err());
        assert!(diff(&format!("{}\n---\nx", "line\n".repeat(2000))).is_err());
    }

    #[test]
    fn csv_to_json_and_back() {
        let o = csv("name,age\n\"Smith, Jo\",31\nAnn,\"say \"\"hi\"\"\"").unwrap();
        let json = value(&o, "JSON (2 rows)");
        assert!(
            json.contains("\"name\": \"Smith, Jo\", \"age\": \"31\""),
            "{json}"
        );
        assert!(json.contains("\"age\": \"say \\\"hi\\\"\""), "{json}");
        let back = csv(r#"[{"b":1,"a":"x,y"},{"a":"z","c":true}]"#).unwrap();
        assert_eq!(value(&back, "CSV (2 rows)"), "a,b,c\n\"x,y\",1,\nz,,true");
        assert_eq!(
            parse_csv("a;b\n1;2", ';'),
            vec![vec!["a", "b"], vec!["1", "2"]]
        );
        assert!(value(&csv("a\tb\n1\t2").unwrap(), "JSON (1 rows)").contains("\"b\": \"2\""));
        assert!(csv("[1,2]").is_err());
        assert!(csv("").is_err());
    }
}
