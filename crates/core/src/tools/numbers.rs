//! Number and time tools: number bases, byte sizes, a calculator, file permissions, IPv4 networks, colours, timestamps and cron. Pure functions
//! of their input (and the time, for the last two), like every tool (see `tools.rs`).

use super::{format_utc, Output};

// ---- bases ----

pub fn bases(input: &str) -> Result<Output, String> {
    let t = input.trim().to_lowercase().replace('_', "");
    if t.is_empty() {
        return Err("type a number: 255, 0xff, 0b1010, 0o17, or 16 ff for ff in base 16".into());
    }
    let n: u128 = if let Some((base, digits)) = t
        .split_once(' ')
        .filter(|(b, _)| b.chars().all(|c| c.is_ascii_digit()))
    {
        let base: u32 = base
            .parse()
            .map_err(|_| "the base must be a number from 2 to 36")?;
        if !(2..=36).contains(&base) {
            return Err("the base must be from 2 to 36".into());
        }
        u128::from_str_radix(digits.trim(), base)
            .map_err(|_| format!("that is not a base {base} number"))?
    } else if let Some(h) = t.strip_prefix("0x") {
        u128::from_str_radix(h, 16).map_err(|_| "that is not a hex number")?
    } else if let Some(b) = t.strip_prefix("0b") {
        u128::from_str_radix(b, 2).map_err(|_| "that is not a binary number")?
    } else if let Some(o) = t.strip_prefix("0o") {
        u128::from_str_radix(o, 8).map_err(|_| "that is not an octal number")?
    } else {
        t.parse()
            .map_err(|_| "that is not a whole number (negative numbers are not supported)")?
    };
    let mut base36 = String::new();
    let mut x = n;
    loop {
        base36.insert(0, char::from_digit((x % 36) as u32, 36).unwrap_or('0'));
        x /= 36;
        if x == 0 {
            break;
        }
    }
    let mut o = Output::default();
    o.field("Decimal", n.to_string());
    o.field("Hexadecimal", format!("0x{n:x}"));
    o.field("Binary", format!("0b{n:b}"));
    o.field("Octal", format!("0o{n:o}"));
    o.field("Base 36", base36);
    Ok(o)
}

// ---- bytes ----

fn trim_number(x: f64) -> String {
    if x >= 100.0 || x.fract() == 0.0 {
        format!("{x:.0}")
    } else {
        let s = format!("{x:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn bytes(input: &str) -> Result<Output, String> {
    let t = input.trim().to_lowercase().replace(',', "");
    if t.is_empty() {
        return Err("type a size: 1536000, 1.5 GiB or 10mb".into());
    }
    let split = t.find(|c: char| c.is_alphabetic()).unwrap_or(t.len());
    let (num, unit) = t.split_at(split);
    let value: f64 = num.trim().parse().map_err(|_| "that is not a size")?;
    let unit = unit.trim();
    let factor: f64 = match unit {
        "" | "b" | "byte" | "bytes" => 1.0,
        "kb" | "k" => 1e3,
        "mb" | "m" => 1e6,
        "gb" | "g" => 1e9,
        "tb" | "t" => 1e12,
        "pb" | "p" => 1e15,
        "kib" => 1024.0,
        "mib" => 1024f64.powi(2),
        "gib" => 1024f64.powi(3),
        "tib" => 1024f64.powi(4),
        "pib" => 1024f64.powi(5),
        _ => {
            return Err("the unit must be B, KB, MB, GB, TB, PB or KiB, MiB, GiB, TiB, PiB".into())
        }
    };
    let b = value * factor;
    if !b.is_finite() || !(0.0..=1e21).contains(&b) {
        return Err("that size is out of range".into());
    }
    let mut o = Output::default();
    o.field("Bytes", format!("{b:.0}"));
    for (label, f) in [("KB", 1e3), ("MB", 1e6), ("GB", 1e9), ("TB", 1e12)] {
        o.field(label, trim_number(b / f));
    }
    for (label, p) in [("KiB", 1), ("MiB", 2), ("GiB", 3), ("TiB", 4)] {
        o.field(label, trim_number(b / 1024f64.powi(p)));
    }
    Ok(o)
}

// ---- calc ----

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: u32,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            let r = self.term()?;
            v = if c == b'+' { v + r } else { v - r };
        }
        Ok(v)
    }
    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.unary()?;
        while let Some(c @ (b'*' | b'/' | b'%')) = self.peek() {
            self.i += 1;
            let r = self.unary()?;
            v = match c {
                b'*' => v * r,
                b'/' if r == 0.0 => return Err("division by zero".into()),
                b'/' => v / r,
                _ if r == 0.0 => return Err("remainder of a division by zero".into()),
                _ => v % r,
            };
        }
        Ok(v)
    }
    fn unary(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(b'-') => {
                self.i += 1;
                Ok(-self.unary()?)
            }
            Some(b'+') => {
                self.i += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Result<f64, String> {
        let base = self.atom()?;
        if self.peek() == Some(b'^') {
            self.i += 1;
            let e = self.unary()?;
            return Ok(base.powf(e));
        }
        Ok(base)
    }
    fn atom(&mut self) -> Result<f64, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("that is nested too deeply".into());
        }
        let r = self.atom_inner();
        self.depth -= 1;
        r
    }
    fn atom_inner(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let v = self.expr()?;
                if self.peek() != Some(b')') {
                    return Err("a closing bracket is missing".into());
                }
                self.i += 1;
                Ok(v)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let start = self.i;
                while self.i < self.s.len()
                    && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.')
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[start..self.i])
                    .unwrap_or("")
                    .parse()
                    .map_err(|_| "that is not a number".to_string())
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let start = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.s[start..self.i])
                    .unwrap_or("")
                    .to_lowercase();
                if self.peek() == Some(b'(') {
                    self.i += 1;
                    let a = self.expr()?;
                    if self.peek() != Some(b')') {
                        return Err("a closing bracket is missing".into());
                    }
                    self.i += 1;
                    return match name.as_str() {
                        "sqrt" if a < 0.0 => Err("the square root of a negative number".into()),
                        "sqrt" => Ok(a.sqrt()),
                        "abs" => Ok(a.abs()),
                        "sin" => Ok(a.sin()),
                        "cos" => Ok(a.cos()),
                        "tan" => Ok(a.tan()),
                        "ln" if a <= 0.0 => {
                            Err("the logarithm of zero or a negative number".into())
                        }
                        "ln" => Ok(a.ln()),
                        "log" if a <= 0.0 => {
                            Err("the logarithm of zero or a negative number".into())
                        }
                        "log" => Ok(a.log10()),
                        "exp" => Ok(a.exp()),
                        "floor" => Ok(a.floor()),
                        "ceil" => Ok(a.ceil()),
                        "round" => Ok(a.round()),
                        _ => Err(format!("there is no function called {name}")),
                    };
                }
                match name.as_str() {
                    "pi" => Ok(std::f64::consts::PI),
                    "e" => Ok(std::f64::consts::E),
                    _ => Err(format!("{name} is not known (the constants are pi and e)")),
                }
            }
            _ => Err("a number or a bracket was expected".into()),
        }
    }
}

/// Show a number without a trailing `.0` and without float noise (12 significant digits).
fn show_number(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        return format!("{x:.0}");
    }
    let s = format!("{x:.12e}");
    // parse back through the exponent form to drop the noise, then print plainly
    let rounded: f64 = s.parse().unwrap_or(x);
    let plain = format!("{rounded}");
    if plain.len() > 24 {
        s
    } else {
        plain
    }
}

pub fn calc(input: &str) -> Result<Output, String> {
    let t = input.trim();
    if t.is_empty() {
        return Err("type a sum, like (2+3)*4, sqrt(16), 2^10 or 7%4".into());
    }
    let mut p = Parser {
        s: t.as_bytes(),
        i: 0,
        depth: 0,
    };
    let v = p.expr()?;
    if p.peek().is_some() {
        return Err("there is something after the end of the sum".into());
    }
    if !v.is_finite() {
        return Err("that is too big or not a number".into());
    }
    let mut o = Output::default();
    o.field("Result", show_number(v));
    if v == v.trunc() && v.abs() < 1e18 && v >= 0.0 {
        o.field("Hexadecimal", format!("0x{:x}", v as u64));
    }
    Ok(o)
}

// ---- chmod and umask ----

fn symbolic(mode: u32) -> String {
    let mut s = String::new();
    for shift in [6, 3, 0] {
        let b = (mode >> shift) & 7;
        s.push(if b & 4 != 0 { 'r' } else { '-' });
        s.push(if b & 2 != 0 { 'w' } else { '-' });
        s.push(if b & 1 != 0 { 'x' } else { '-' });
    }
    s
}

fn describe_bits(b: u32) -> String {
    let mut v = vec![];
    if b & 4 != 0 {
        v.push("read");
    }
    if b & 2 != 0 {
        v.push("write");
    }
    if b & 1 != 0 {
        v.push("execute");
    }
    if v.is_empty() {
        "nothing".into()
    } else {
        v.join(", ")
    }
}

pub fn chmod(input: &str) -> Result<Output, String> {
    let t = input.trim().to_lowercase();
    if t.is_empty() {
        return Err("type 755, rwxr-xr-x, or umask 022".into());
    }
    let mut o = Output::default();
    if let Some(m) = t.strip_prefix("umask") {
        let m = m.trim();
        if m.len() != 3 || !m.chars().all(|c| ('0'..='7').contains(&c)) {
            return Err("a umask is three octal digits, like 022".into());
        }
        let mask = u32::from_str_radix(m, 8).map_err(|_| "a umask is three octal digits")?;
        let (file, dir) = (0o666 & !mask, 0o777 & !mask);
        o.field("New files", format!("{file:03o}  {}", symbolic(file)));
        o.field("New directories", format!("{dir:03o}  {}", symbolic(dir)));
        return Ok(o);
    }
    let mode = if t.len() == 9 && t.chars().all(|c| matches!(c, 'r' | 'w' | 'x' | '-')) {
        let mut m = 0;
        for (i, c) in t.chars().enumerate() {
            let want = ['r', 'w', 'x'][i % 3];
            if c == want {
                m |= 1 << (8 - i);
            } else if c != '-' {
                return Err(
                    "symbolic permissions go r, w, x in each group of three, like rwxr-xr-x".into(),
                );
            }
        }
        m
    } else if (3..=4).contains(&t.len()) && t.chars().all(|c| ('0'..='7').contains(&c)) {
        u32::from_str_radix(&t, 8).map_err(|_| "not an octal mode")? & 0o777
    } else {
        return Err("type 755, rwxr-xr-x, or umask 022".into());
    };
    o.field("Octal", format!("{mode:03o}"));
    o.field("Symbolic", symbolic(mode));
    o.field("Command", format!("chmod {mode:03o} <file>"));
    o.field("Owner", describe_bits((mode >> 6) & 7));
    o.field("Group", describe_bits((mode >> 3) & 7));
    o.field("Everyone else", describe_bits(mode & 7));
    Ok(o)
}

// ---- cidr (IPv4) ----

fn parse_ip(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.trim().split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut n = 0u32;
    for p in parts {
        if p.is_empty() || p.len() > 3 || !p.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        n = (n << 8) | p.parse::<u32>().ok().filter(|v| *v <= 255)?;
    }
    Some(n)
}

fn ip_text(n: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        n >> 24,
        (n >> 16) & 255,
        (n >> 8) & 255,
        n & 255
    )
}

fn parse_block(s: &str) -> Result<(u32, u32), String> {
    let (ip, prefix) = match s.trim().split_once('/') {
        Some((a, p)) => (
            a,
            p.trim()
                .parse::<u32>()
                .map_err(|_| "the prefix must be a number from 0 to 32")?,
        ),
        None => (s.trim(), 32),
    };
    if s.contains(':') {
        return Err("IPv6 is not supported yet; this reads IPv4 like 10.0.0.0/24".into());
    }
    if prefix > 32 {
        return Err("the prefix must be from 0 to 32".into());
    }
    let ip = parse_ip(ip).ok_or("that is not an IPv4 address like 10.0.0.5")?;
    Ok((ip, prefix))
}

fn mask(prefix: u32) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

fn kind(ip: u32) -> &'static str {
    let a = (ip >> 24) as u8;
    let b = ((ip >> 16) & 255) as u8;
    match (a, b) {
        (10, _) | (192, 168) => "private (RFC 1918)",
        (172, 16..=31) => "private (RFC 1918)",
        (127, _) => "loopback",
        (169, 254) => "link-local",
        (100, 64..=127) => "carrier-grade NAT (shared)",
        (224..=239, _) => "multicast",
        (240..=255, _) => "reserved",
        (0, _) => "this network",
        _ => "public",
    }
}

pub fn cidr(input: &str) -> Result<Output, String> {
    let t = input.trim();
    if t.is_empty() {
        return Err("type 10.0.0.0/24, or 10.0.0.5 in 10.0.0.0/24".into());
    }
    let mut o = Output::default();
    if let Some((addr, block)) = t.split_once(" in ") {
        let (ip, _) = parse_block(addr)?;
        let (net, prefix) = parse_block(block)?;
        let inside = ip & mask(prefix) == net & mask(prefix);
        o.field(
            "Answer",
            if inside {
                "yes, it is inside"
            } else {
                "no, it is outside"
            },
        );
        o.field("Address", ip_text(ip));
        o.field(
            "Network",
            format!("{}/{}", ip_text(net & mask(prefix)), prefix),
        );
        return Ok(o);
    }
    let (ip, prefix) = parse_block(t)?;
    let m = mask(prefix);
    let net = ip & m;
    let bcast = net | !m;
    o.field("Network", format!("{}/{}", ip_text(net), prefix));
    o.field("Netmask", ip_text(m));
    o.field("Wildcard", ip_text(!m));
    o.field("Broadcast", ip_text(bcast));
    let hosts: u64 = match prefix {
        32 => 1,
        31 => 2,
        _ => (1u64 << (32 - prefix)) - 2,
    };
    let (first, last) = match prefix {
        32 => (net, net),
        31 => (net, bcast),
        _ => (net + 1, bcast - 1),
    };
    o.field("First address", ip_text(first));
    o.field("Last address", ip_text(last));
    o.field("Usable addresses", hosts.to_string());
    o.field("Type", kind(net));
    if ip != net {
        o.note("The address has bits set after the prefix; the network is shown.");
    }
    Ok(o)
}

// ---- colour ----

fn hsl_of(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 {
        d / (2.0 - mx - mn)
    } else {
        d / (mx + mn)
    };
    let h = if mx == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn rgb_of(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let h = (h % 360.0 + 360.0) % 360.0 / 360.0;
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

fn numbers_in(s: &str) -> Vec<f64> {
    s.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse().ok())
        .collect()
}

pub fn color(input: &str) -> Result<Output, String> {
    let t = input.trim().to_lowercase();
    if t.is_empty() {
        return Err(
            "type a colour: #ff8800, rgb(255,136,0), hsl(32,100%,50%) or a name like orange".into(),
        );
    }
    let named = [
        ("black", "000000"),
        ("white", "ffffff"),
        ("red", "ff0000"),
        ("green", "008000"),
        ("blue", "0000ff"),
        ("yellow", "ffff00"),
        ("orange", "ffa500"),
        ("purple", "800080"),
        ("grey", "808080"),
        ("gray", "808080"),
        ("pink", "ffc0cb"),
        ("cyan", "00ffff"),
        ("magenta", "ff00ff"),
    ];
    let (r, g, b) = if let Some((_, hex)) = named.iter().find(|(n, _)| *n == t) {
        let v = u32::from_str_radix(hex, 16).unwrap_or(0);
        (
            (v >> 16) as f64 / 255.0,
            ((v >> 8) & 255) as f64 / 255.0,
            (v & 255) as f64 / 255.0,
        )
    } else if let Some(h) = t.strip_prefix('#').or(Some(t.as_str())
        .filter(|x| x.chars().all(|c| c.is_ascii_hexdigit()) && matches!(x.len(), 3 | 6)))
    {
        let h = if h.len() == 3 {
            h.chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            h.to_string()
        };
        if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("a hex colour has three or six digits, like #f80 or #ff8800".into());
        }
        let v = u32::from_str_radix(&h, 16).map_err(|_| "not a hex colour")?;
        (
            (v >> 16) as f64 / 255.0,
            ((v >> 8) & 255) as f64 / 255.0,
            (v & 255) as f64 / 255.0,
        )
    } else if t.starts_with("rgb") {
        let n = numbers_in(&t);
        if n.len() < 3 || n.iter().take(3).any(|x| !(0.0..=255.0).contains(x)) {
            return Err("rgb needs three numbers from 0 to 255".into());
        }
        (n[0] / 255.0, n[1] / 255.0, n[2] / 255.0)
    } else if t.starts_with("hsl") {
        let n = numbers_in(&t);
        if n.len() < 3 || !(0.0..=100.0).contains(&n[1]) || !(0.0..=100.0).contains(&n[2]) {
            return Err("hsl needs a hue and two percentages, like hsl(32, 100%, 50%)".into());
        }
        rgb_of(n[0], n[1] / 100.0, n[2] / 100.0)
    } else {
        return Err("that is not a colour I know".into());
    };
    let (r8, g8, b8) = (
        (r * 255.0).round() as u32,
        (g * 255.0).round() as u32,
        (b * 255.0).round() as u32,
    );
    let (h, s, l) = hsl_of(r, g, b);
    let mut o = Output::default();
    o.field("Hex", format!("#{r8:02x}{g8:02x}{b8:02x}"));
    o.field("RGB", format!("rgb({r8}, {g8}, {b8})"));
    o.field(
        "HSL",
        format!(
            "hsl({}, {}%, {}%)",
            h.round() as i32,
            (s * 100.0).round() as i32,
            (l * 100.0).round() as i32
        ),
    );
    o.field("RGB, 0 to 1", format!("{:.3}, {:.3}, {:.3}", r, g, b));
    Ok(o)
}

// ---- timestamp ----

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-11-06`, `2026-11-06T12:30`, `2026-11-06 12:30:15Z`, `…+02:00`: seconds since 1970 (UTC).
fn parse_iso(t: &str) -> Option<i64> {
    let t = t.trim();
    let (date, rest) = t
        .split_once(['T', ' '])
        .map_or((t, ""), |(a, b)| (a, b.trim()));
    let dp: Vec<&str> = date.split('-').collect();
    if dp.len() != 3 {
        return None;
    }
    let (y, m, d) = (
        dp[0].parse::<i64>().ok()?,
        dp[1].parse::<i64>().ok()?,
        dp[2].parse::<i64>().ok()?,
    );
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || dp[0].len() != 4 {
        return None;
    }
    let mut secs = days_from_civil(y, m, d) * 86_400;
    let mut offset = 0i64;
    let mut time = rest.to_uppercase();
    if let Some(p) = time.strip_suffix('Z') {
        time = p.to_string();
    } else if let Some(i) = time.rfind(['+', '-']).filter(|i| *i > 0) {
        let off = time[i + 1..].replace(':', "");
        if off.len() == 4 && off.chars().all(|c| c.is_ascii_digit()) {
            let mins = off[..2].parse::<i64>().ok()? * 60 + off[2..].parse::<i64>().ok()?;
            offset = if time.as_bytes()[i] == b'-' {
                -mins
            } else {
                mins
            } * 60;
            time.truncate(i);
        }
    }
    let time = time.trim();
    if !time.is_empty() {
        let tp: Vec<&str> = time.split(':').collect();
        let (h, mi, s) = (
            tp.first()?.parse::<i64>().ok()?,
            tp.get(1).map_or(Some(0), |x| x.parse::<i64>().ok())?,
            tp.get(2)
                .map_or(Some(0), |x| x.parse::<f64>().ok().map(|f| f as i64))?,
        );
        if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..61).contains(&s) {
            return None;
        }
        secs += h * 3600 + mi * 60 + s;
    }
    Some(secs - offset)
}

pub fn timestamp(input: &str, now: i64) -> Result<Output, String> {
    let t = input.trim().to_lowercase();
    let secs: i64 = if t.is_empty() || t == "now" {
        now
    } else if let Some(rest) = t.strip_prefix(['+', '-']) {
        let sign = if t.starts_with('-') { -1 } else { 1 };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let unit = &rest[digits.len()..];
        let n: i64 = digits
            .parse()
            .map_err(|_| "an offset looks like +3d, -2h, +90m or +30s")?;
        let per = match unit.trim() {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86_400,
            "w" => 604_800,
            _ => return Err("an offset looks like +3d, -2h, +90m or +30s".into()),
        };
        now + sign * n * per
    } else if t.chars().all(|c| c.is_ascii_digit() || c == '.')
        && t.chars().any(|c| c.is_ascii_digit())
    {
        let n: f64 = t.parse().map_err(|_| "that is not a number")?;
        // 13 digits or more is milliseconds
        if n >= 1e11 {
            (n / 1000.0) as i64
        } else {
            n as i64
        }
    } else {
        parse_iso(input.trim())
            .ok_or("type now, a Unix time, a date like 2026-11-06T12:00Z, or an offset like +3d")?
    };
    if !(-62_135_596_800..=253_402_300_799).contains(&secs) {
        return Err("that is outside years 0001 to 9999".into());
    }
    let days = secs.div_euclid(86_400);
    let weekday = [
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
    ][days.rem_euclid(7) as usize];
    let d = secs - now;
    let rel = if d == 0 {
        "now".to_string()
    } else {
        let (n, past) = (d.abs(), d < 0);
        let text = match n {
            0..=59 => format!("{n} s"),
            60..=3599 => format!("{} min", n / 60),
            3600..=86_399 => format!("{} h", n / 3600),
            _ => format!("{} days", n / 86_400),
        };
        if past {
            format!("{text} ago")
        } else {
            format!("in {text}")
        }
    };
    let utc = format_utc(secs);
    let mut o = Output::default();
    o.field("Unix seconds", secs.to_string());
    o.field("Unix milliseconds", (secs * 1000).to_string());
    o.field(
        "ISO 8601 (UTC)",
        format!("{}T{}Z", &utc[..10], &utc[11..19]),
    );
    o.field("Readable (UTC)", format!("{weekday}, {utc}"));
    o.field("Compared with now", rel);
    o.note("Times are shown in UTC.");
    Ok(o)
}

// ---- cron ----

struct Field {
    allowed: Vec<bool>,
    any: bool,
    text: String,
}

fn parse_cron_field(
    spec: &str,
    lo: u32,
    hi: u32,
    names: &[&str],
    what: &str,
) -> Result<Field, String> {
    let mut allowed = vec![false; (hi + 1) as usize];
    let value = |s: &str| -> Result<u32, String> {
        if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(s)) {
            return Ok(i as u32 + if lo == 1 && names.len() == 12 { 1 } else { 0 });
        }
        s.parse::<u32>()
            .ok()
            .filter(|v| (lo..=hi).contains(v) || (what == "day of week" && *v == 7))
            .ok_or_else(|| format!("{s} is not a valid {what} (use {lo} to {hi})"))
    };
    let mut parts_text = vec![];
    for part in spec.split(',') {
        let (range, step) = part
            .split_once('/')
            .map_or((part, 1u32), |(r, s)| (r, s.parse().unwrap_or(0)));
        if step == 0 {
            return Err(format!(
                "the step in the {what} field must be a positive number"
            ));
        }
        let (a, b, text) = if range == "*" {
            (
                lo,
                hi,
                if step == 1 {
                    format!("every {what}")
                } else {
                    format!("every {step} {what}s")
                },
            )
        } else if let Some((x, y)) = range.split_once('-') {
            let (x, y) = (value(x)?, value(y)?);
            if x > y {
                return Err(format!("the {what} range {range} runs backwards"));
            }
            (
                x,
                y,
                if step == 1 {
                    format!("{what} {x} to {y}")
                } else {
                    format!("every {step} {what}s from {x} to {y}")
                },
            )
        } else {
            let x = value(range)?;
            (
                x,
                if step > 1 { hi } else { x },
                if step > 1 {
                    format!("every {step} {what}s from {x}")
                } else {
                    format!("{what} {x}")
                },
            )
        };
        let mut v = a;
        while v <= b {
            allowed[(if what == "day of week" && v == 7 {
                0
            } else {
                v
            }) as usize] = true;
            v += step;
        }
        parts_text.push(text);
    }
    Ok(Field {
        allowed,
        any: spec == "*",
        text: parts_text.join(", "),
    })
}

pub fn cron(input: &str, now: i64) -> Result<Output, String> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() != 5 {
        return Err("a cron expression has five fields: minute hour day-of-month month day-of-week, like */15 9-17 * * 1-5".into());
    }
    let months = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let days = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
    let minute = parse_cron_field(parts[0], 0, 59, &[], "minute")?;
    let hour = parse_cron_field(parts[1], 0, 23, &[], "hour")?;
    let dom = parse_cron_field(parts[2], 1, 31, &[], "day of month")?;
    let month = parse_cron_field(parts[3], 1, 12, &months, "month")?;
    let dow = parse_cron_field(parts[4], 0, 6, &days, "day of week")?;
    let mut o = Output::default();
    o.field("Expression", parts.join(" "));
    o.field("Minute", minute.text.clone());
    o.field("Hour", hour.text.clone());
    o.field("Day of month", dom.text.clone());
    o.field("Month", month.text.clone());
    o.field("Day of week", dow.text.clone());
    // the next five times, checked a minute at a time for up to four years (cron's rule: if both day fields are restricted, either may match)
    let mut found = vec![];
    let mut t = (now / 60 + 1) * 60;
    let end = t + 4 * 366 * 86_400;
    while t < end && found.len() < 5 {
        let days_since = t.div_euclid(86_400);
        let sod = t.rem_euclid(86_400);
        let (h, mi) = ((sod / 3600) as usize, (sod % 3600 / 60) as usize);
        if minute.allowed[mi] && hour.allowed[h] {
            // civil date of this day
            let z = days_since + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = (doy - (153 * mp + 2) / 5 + 1) as usize;
            let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as usize;
            let _ = era;
            let wd = (days_since + 4).rem_euclid(7) as usize;
            let day_ok = if !dom.any && !dow.any {
                dom.allowed[d] || dow.allowed[wd]
            } else {
                dom.allowed[d] && dow.allowed[wd]
            };
            if month.allowed[m] && day_ok {
                found.push(t);
            }
        }
        t += 60;
    }
    if found.is_empty() {
        o.note("It does not run in the next four years.");
    }
    o.block(
        "Next runs (UTC)",
        found
            .iter()
            .map(|t| format_utc(*t))
            .collect::<Vec<_>>()
            .join("\n"),
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
            .unwrap_or_else(|| panic!("no field {label}: {:?}", o.fields))
            .value
            .clone()
    }

    #[test]
    fn bases() {
        let o = super::bases("255").unwrap();
        assert_eq!(
            (
                value(&o, "Hexadecimal").as_str(),
                value(&o, "Binary").as_str(),
                value(&o, "Octal").as_str(),
                value(&o, "Base 36").as_str()
            ),
            ("0xff", "0b11111111", "0o377", "73")
        );
        assert_eq!(value(&super::bases("0xFF").unwrap(), "Decimal"), "255");
        assert_eq!(value(&super::bases("0b1010").unwrap(), "Decimal"), "10");
        assert_eq!(value(&super::bases("16 ff").unwrap(), "Decimal"), "255");
        assert_eq!(
            value(
                &super::bases("340282366920938463463374607431768211455").unwrap(),
                "Hexadecimal"
            ),
            "0xffffffffffffffffffffffffffffffff"
        );
        for bad in [
            "",
            "-5",
            "abc",
            "0xzz",
            "99 1",
            "1 2 3",
            "340282366920938463463374607431768211456",
        ] {
            assert!(super::bases(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn byte_sizes() {
        let o = bytes("1.5 GiB").unwrap();
        assert_eq!(value(&o, "Bytes"), "1610612736");
        assert_eq!(value(&o, "GiB"), "1.5");
        assert_eq!(value(&bytes("10mb").unwrap(), "Bytes"), "10000000");
        assert_eq!(value(&bytes("1536000").unwrap(), "KiB"), "1500");
        for bad in ["", "abc", "5 parsecs", "-3 kb", "1e30 pb"] {
            assert!(bytes(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_calculator_follows_the_usual_rules_and_refuses_nonsense() {
        let r = |s: &str| calc(s).map(|o| o.fields[0].value.clone());
        assert_eq!(r("(2+3)*4").unwrap(), "20");
        assert_eq!(r("2^10").unwrap(), "1024");
        assert_eq!(r("2^3^2").unwrap(), "512", "powers go right to left");
        assert_eq!(
            r("-2^2").unwrap(),
            "-4",
            "the power binds tighter than the minus"
        );
        assert_eq!(r("7%4").unwrap(), "3");
        assert_eq!(r("sqrt(16)+abs(-3)").unwrap(), "7");
        assert_eq!(r("0.1+0.2").unwrap(), "0.3");
        assert_eq!(r("pi*2").unwrap(), "6.28318530718");
        assert_eq!(r("10/4").unwrap(), "2.5");
        for bad in [
            "",
            "1/0",
            "1 +",
            "(1",
            "2 3",
            "foo(1)",
            "sqrt(-1)",
            "ln(0)",
            "x",
            "1e999999",
            &"(".repeat(200),
        ] {
            assert!(r(bad).is_err(), "{bad:?}");
        }
        assert_eq!(value(&calc("255").unwrap(), "Hexadecimal"), "0xff");
    }

    #[test]
    fn permissions() {
        let o = chmod("755").unwrap();
        assert_eq!(value(&o, "Symbolic"), "rwxr-xr-x");
        assert_eq!(value(&o, "Command"), "chmod 755 <file>");
        assert_eq!(value(&o, "Group"), "read, execute");
        assert_eq!(value(&chmod("rw-r--r--").unwrap(), "Octal"), "644");
        assert_eq!(value(&chmod("0644").unwrap(), "Octal"), "644");
        let u = chmod("umask 022").unwrap();
        assert_eq!(value(&u, "New files"), "644  rw-r--r--");
        assert_eq!(value(&u, "New directories"), "755  rwxr-xr-x");
        for bad in [
            "",
            "999",
            "rwxrwxrwz",
            "umask 99",
            "umask 0222",
            "75",
            "rwx",
        ] {
            assert!(chmod(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn ipv4_networks() {
        let o = cidr("10.0.0.0/24").unwrap();
        assert_eq!(value(&o, "Netmask"), "255.255.255.0");
        assert_eq!(value(&o, "Broadcast"), "10.0.0.255");
        assert_eq!(value(&o, "First address"), "10.0.0.1");
        assert_eq!(value(&o, "Usable addresses"), "254");
        assert_eq!(value(&o, "Type"), "private (RFC 1918)");
        let o = cidr("192.168.1.77/26").unwrap();
        assert_eq!(value(&o, "Network"), "192.168.1.64/26");
        assert!(!o.notes.is_empty());
        assert_eq!(value(&cidr("8.8.8.8").unwrap(), "Usable addresses"), "1");
        assert_eq!(value(&cidr("8.8.8.8/32").unwrap(), "Type"), "public");
        assert_eq!(
            value(&cidr("10.0.0.0/31").unwrap(), "Usable addresses"),
            "2"
        );
        assert_eq!(
            value(&cidr("0.0.0.0/0").unwrap(), "Usable addresses"),
            "4294967294"
        );
        assert_eq!(
            value(&cidr("10.0.0.5 in 10.0.0.0/24").unwrap(), "Answer"),
            "yes, it is inside"
        );
        assert_eq!(
            value(&cidr("10.0.1.5 in 10.0.0.0/24").unwrap(), "Answer"),
            "no, it is outside"
        );
        for bad in [
            "",
            "10.0.0/24",
            "10.0.0.256/24",
            "10.0.0.0/33",
            "10.0.0.0/x",
            "::1",
            "a.b.c.d",
            "10.0.0.0 in",
        ] {
            assert!(cidr(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn colours() {
        let o = color("#ff8800").unwrap();
        assert_eq!(value(&o, "RGB"), "rgb(255, 136, 0)");
        assert_eq!(value(&o, "HSL"), "hsl(32, 100%, 50%)");
        assert_eq!(value(&color("rgb(255,136,0)").unwrap(), "Hex"), "#ff8800");
        assert_eq!(
            value(&color("hsl(32, 100%, 50%)").unwrap(), "Hex"),
            "#ff8800"
        );
        assert_eq!(value(&color("#f80").unwrap(), "Hex"), "#ff8800");
        assert_eq!(value(&color("white").unwrap(), "Hex"), "#ffffff");
        assert_eq!(value(&color("000").unwrap(), "RGB"), "rgb(0, 0, 0)");
        for bad in [
            "",
            "#12",
            "#gggggg",
            "rgb(300,0,0)",
            "hsl(1, 200%, 5%)",
            "notacolour",
            "rgb(1,2)",
        ] {
            assert!(color(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn timestamps() {
        let now = 1_700_000_000;
        let o = timestamp("1700003600", now).unwrap();
        assert_eq!(value(&o, "ISO 8601 (UTC)"), "2023-11-14T23:13:20Z");
        assert_eq!(value(&o, "Compared with now"), "in 1 h");
        assert!(value(&o, "Readable (UTC)").starts_with("Tuesday, 2023-11-14"));
        assert_eq!(
            value(&timestamp("1700003600000", now).unwrap(), "Unix seconds"),
            "1700003600",
            "13 digits are milliseconds"
        );
        assert_eq!(
            value(
                &timestamp("2023-11-14T23:13:20Z", now).unwrap(),
                "Unix seconds"
            ),
            "1700003600"
        );
        assert_eq!(
            value(
                &timestamp("2023-11-14 23:13:20", now).unwrap(),
                "Unix seconds"
            ),
            "1700003600"
        );
        assert_eq!(
            value(
                &timestamp("2023-11-15T01:13:20+02:00", now).unwrap(),
                "Unix seconds"
            ),
            "1700003600"
        );
        assert_eq!(
            value(&timestamp("2023-11-14", now).unwrap(), "Unix seconds"),
            "1699920000"
        );
        assert_eq!(
            value(&timestamp("+3d", now).unwrap(), "Unix seconds"),
            (now + 259_200).to_string()
        );
        assert_eq!(
            value(&timestamp("-2h", now).unwrap(), "Compared with now"),
            "2 h ago"
        );
        assert_eq!(
            value(&timestamp("", now).unwrap(), "Unix seconds"),
            now.to_string()
        );
        assert_eq!(
            value(&timestamp("0", now).unwrap(), "ISO 8601 (UTC)"),
            "1970-01-01T00:00:00Z"
        );
        for bad in [
            "yesterday",
            "2023-13-01",
            "2023-11-14T25:00Z",
            "+3x",
            "99999999999999999999",
        ] {
            assert!(timestamp(bad, now).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn cron_expressions_are_explained_and_their_next_runs_found() {
        // 2023-11-14 22:13:20 UTC is a Tuesday
        let now = 1_700_000_000;
        let o = cron("*/15 9-17 * * 1-5", now).unwrap();
        assert_eq!(value(&o, "Minute"), "every 15 minutes");
        assert_eq!(value(&o, "Hour"), "hour 9 to 17");
        assert_eq!(value(&o, "Day of week"), "day of week 1 to 5");
        assert_eq!(value(&o, "Day of month"), "every day of month");
        let runs = value(&o, "Next runs (UTC)");
        assert_eq!(
            runs.lines().next().unwrap(),
            "2023-11-15 09:00:00 UTC",
            "{runs}"
        );
        assert_eq!(runs.lines().count(), 5);
        assert!(runs
            .lines()
            .all(|l| &l[11..13] >= "09" && &l[11..13] <= "17"));
        // every minute: the very next minute
        assert_eq!(
            value(&cron("* * * * *", now).unwrap(), "Next runs (UTC)")
                .lines()
                .next()
                .unwrap(),
            "2023-11-14 22:14:00 UTC"
        );
        // the 1st of January, midnight
        assert_eq!(
            value(&cron("0 0 1 jan *", now).unwrap(), "Next runs (UTC)")
                .lines()
                .next()
                .unwrap(),
            "2024-01-01 00:00:00 UTC"
        );
        // a day of month and a day of week: either may match
        let both = value(&cron("0 0 13 * 5", now).unwrap(), "Next runs (UTC)");
        assert_eq!(
            both.lines().next().unwrap(),
            "2023-11-17 00:00:00 UTC",
            "{both}"
        );
        // never
        assert!(cron("0 0 31 2 *", now).unwrap().notes[0].contains("does not run"));
        for bad in [
            "",
            "* * * *",
            "61 * * * *",
            "* 24 * * *",
            "* * 0 * *",
            "* * * 13 *",
            "*/0 * * * *",
            "5-2 * * * *",
            "a b c d e",
            "* * * * * *",
        ] {
            assert!(cron(bad, now).is_err(), "{bad:?}");
        }
    }
}
