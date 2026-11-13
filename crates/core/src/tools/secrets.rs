//! Hashing and password generation: pure functions like every tool (see `tools.rs`). The hash functions are written out here (MD5, SHA-1,
//! SHA-256, SHA-384 and SHA-512) so the project needs no crate for them; each is checked against the published test vectors.

use super::Output;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Pad a message the way MD5, SHA-1 and SHA-2 all do: a 1 bit, zeros, then the length in bits (`len_bytes` wide, big or little endian).
fn pad(data: &[u8], block: usize, len_bytes: usize, big_endian: bool) -> Vec<u8> {
    let mut m = data.to_vec();
    m.push(0x80);
    while m.len() % block != block - len_bytes {
        m.push(0);
    }
    let bits = (data.len() as u128) * 8;
    let bytes = bits.to_be_bytes();
    let tail = &bytes[16 - len_bytes..];
    if big_endian {
        m.extend_from_slice(tail);
    } else {
        m.extend(tail.iter().rev());
    }
    m
}

pub fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64)
        .map(|i| ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32)
        .collect();
    let (mut a0, mut b0, mut c0, mut d0) =
        (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    for chunk in pad(data, 64, 8, false).chunks(64) {
        let m: Vec<u32> = chunk
            .chunks(4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, v) in [a0, b0, c0, d0].iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    for chunk in pad(data, 64, 8, true).chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i / 20 {
                0 => ((b & c) | (!b & d), 0x5A827999),
                1 => (b ^ c ^ d, 0x6ED9EBA1),
                2 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in pad(data, 64, 8, true).chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

const K512: [u64; 80] = [
    0x428a2f98d728ae22,
    0x7137449123ef65cd,
    0xb5c0fbcfec4d3b2f,
    0xe9b5dba58189dbbc,
    0x3956c25bf348b538,
    0x59f111f1b605d019,
    0x923f82a4af194f9b,
    0xab1c5ed5da6d8118,
    0xd807aa98a3030242,
    0x12835b0145706fbe,
    0x243185be4ee4b28c,
    0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f,
    0x80deb1fe3b1696b1,
    0x9bdc06a725c71235,
    0xc19bf174cf692694,
    0xe49b69c19ef14ad2,
    0xefbe4786384f25e3,
    0x0fc19dc68b8cd5b5,
    0x240ca1cc77ac9c65,
    0x2de92c6f592b0275,
    0x4a7484aa6ea6e483,
    0x5cb0a9dcbd41fbd4,
    0x76f988da831153b5,
    0x983e5152ee66dfab,
    0xa831c66d2db43210,
    0xb00327c898fb213f,
    0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2,
    0xd5a79147930aa725,
    0x06ca6351e003826f,
    0x142929670a0e6e70,
    0x27b70a8546d22ffc,
    0x2e1b21385c26c926,
    0x4d2c6dfc5ac42aed,
    0x53380d139d95b3df,
    0x650a73548baf63de,
    0x766a0abb3c77b2a8,
    0x81c2c92e47edaee6,
    0x92722c851482353b,
    0xa2bfe8a14cf10364,
    0xa81a664bbc423001,
    0xc24b8b70d0f89791,
    0xc76c51a30654be30,
    0xd192e819d6ef5218,
    0xd69906245565a910,
    0xf40e35855771202a,
    0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8,
    0x1e376c085141ab53,
    0x2748774cdf8eeb99,
    0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63,
    0x4ed8aa4ae3418acb,
    0x5b9cca4f7763e373,
    0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc,
    0x78a5636f43172f60,
    0x84c87814a1f0ab72,
    0x8cc702081a6439ec,
    0x90befffa23631e28,
    0xa4506cebde82bde9,
    0xbef9a3f7b2c67915,
    0xc67178f2e372532b,
    0xca273eceea26619c,
    0xd186b8c721c0c207,
    0xeada7dd6cde0eb1e,
    0xf57d4f7fee6ed178,
    0x06f067aa72176fba,
    0x0a637dc5a2c898a6,
    0x113f9804bef90dae,
    0x1b710b35131c471b,
    0x28db77f523047d84,
    0x32caab7b40c72493,
    0x3c9ebe0a15c9bebc,
    0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6,
    0x597f299cfc657e2a,
    0x5fcb6fab3ad6faec,
    0x6c44198c4a475817,
];

fn sha512_core(data: &[u8], mut h: [u64; 8]) -> [u64; 8] {
    for chunk in pad(data, 128, 16, true).chunks(128) {
        let mut w = [0u64; 80];
        for i in 0..16 {
            w[i] = u64::from_be_bytes(chunk[i * 8..i * 8 + 8].try_into().unwrap_or([0; 8]));
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..80 {
            let s1 = v[4].rotate_right(14) ^ v[4].rotate_right(18) ^ v[4].rotate_right(41);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K512[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(28) ^ v[0].rotate_right(34) ^ v[0].rotate_right(39);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    h
}

pub fn sha512(data: &[u8]) -> Vec<u8> {
    sha512_core(
        data,
        [
            0x6a09e667f3bcc908,
            0xbb67ae8584caa73b,
            0x3c6ef372fe94f82b,
            0xa54ff53a5f1d36f1,
            0x510e527fade682d1,
            0x9b05688c2b3e6c1f,
            0x1f83d9abfb41bd6b,
            0x5be0cd19137e2179,
        ],
    )
    .iter()
    .flat_map(|v| v.to_be_bytes())
    .collect()
}

pub fn sha384(data: &[u8]) -> Vec<u8> {
    sha512_core(
        data,
        [
            0xcbbb9d5dc1059ed8,
            0x629a292a367cd507,
            0x9159015a3070dd17,
            0x152fecd8f70e5939,
            0x67332667ffc00b31,
            0x8eb44a8768581511,
            0xdb0c2e0d64f98fa7,
            0x47b5481dbefa4fa4,
        ],
    )
    .iter()
    .take(6)
    .flat_map(|v| v.to_be_bytes())
    .collect()
}

pub fn hash(input: &str) -> Result<Output, String> {
    if input.is_empty() {
        return Err("type some text to hash".into());
    }
    let d = input.as_bytes();
    let mut o = Output::default();
    o.field("MD5", hex(&md5(d)));
    o.field("SHA-1", hex(&sha1(d)));
    o.field("SHA-256", hex(&sha256(d)));
    o.field("SHA-384", hex(&sha384(d)));
    o.field("SHA-512", hex(&sha512(d)));
    o.note("MD5 and SHA-1 are broken for security; use SHA-256 or better. The text is hashed exactly as typed, including line breaks.");
    Ok(o)
}

// ---- password ----

/// Random bytes from the standard library's seeded hashing (seeded by the operating system) mixed with the time. Fine for a throw-away value;
/// for a long-lived secret use a password manager.
fn random_bytes(n: usize) -> Vec<u8> {
    use std::hash::{BuildHasher, Hasher};
    let mut out = Vec::with_capacity(n + 8);
    let mut counter = 0u64;
    while out.len() < n {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(counter);
        if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            h.write_u128(d.as_nanos());
        }
        out.extend_from_slice(&h.finish().to_le_bytes());
        counter += 1;
    }
    out.truncate(n);
    out
}

/// A uniform pick from `alphabet` (rejecting bytes that would bias it).
fn pick(alphabet: &[char], count: usize) -> String {
    let limit = 256 - (256 % alphabet.len());
    let mut out = String::new();
    while out.chars().count() < count {
        for b in random_bytes(count * 2 + 8) {
            if (b as usize) < limit && out.chars().count() < count {
                out.push(alphabet[b as usize % alphabet.len()]);
            }
        }
    }
    out
}

pub fn password(input: &str) -> Result<Output, String> {
    let t = input.trim().to_lowercase();
    let mut length: Option<usize> = None;
    let (mut upper, mut lower, mut digits, mut symbols, mut hex_only, mut pin) =
        (true, true, true, true, false, false);
    for word in t.split_whitespace() {
        match word {
            "nosymbols" => symbols = false,
            "nodigits" => digits = false,
            "noupper" => upper = false,
            "nolower" => lower = false,
            "hex" => hex_only = true,
            "pin" => pin = true,
            w => length = Some(w.parse::<usize>().map_err(|_| format!("{w} is not a length or a flag (nosymbols, nodigits, noupper, nolower, hex, pin)"))?),
        }
    }
    let alphabet: Vec<char> = if pin {
        ('0'..='9').collect()
    } else if hex_only {
        "0123456789abcdef".chars().collect()
    } else {
        let mut a = vec![];
        if lower {
            a.extend('a'..='z');
        }
        if upper {
            a.extend('A'..='Z');
        }
        if digits {
            a.extend('0'..='9');
        }
        if symbols {
            a.extend("!@#$%^&*()-_=+[]{};:,.?".chars());
        }
        a
    };
    if alphabet.is_empty() {
        return Err("at least one kind of character must be allowed".into());
    }
    let length = length.unwrap_or(if pin { 6 } else { 20 });
    if !(4..=128).contains(&length) {
        return Err("the length must be from 4 to 128".into());
    }
    let bits = length as f64 * (alphabet.len() as f64).log2();
    let mut o = Output::default();
    o.field("Password", pick(&alphabet, length));
    o.field("Another", pick(&alphabet, length));
    o.field(
        "Strength",
        format!(
            "about {} bits ({} possible characters, {length} long)",
            bits.round(),
            alphabet.len()
        ),
    );
    o.note("Not stored or logged. Generated from the system's random seed: for something you will keep for years, use a password manager.");
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_published_test_vectors() {
        assert_eq!(hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hex(&md5(b"The quick brown fox jumps over the lazy dog")),
            "9e107d9d372bb6826bd81d3542a419d6"
        );
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(hex(&sha384(b"abc")), "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7");
        assert_eq!(hex(&sha512(b"abc")), "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f");
    }

    #[test]
    fn hashes_cross_block_boundaries() {
        // lengths around the padding edges (55, 56, 63, 64, 111, 112, 127, 128) must all work and differ
        let mut seen = std::collections::HashSet::new();
        for n in [0usize, 1, 55, 56, 63, 64, 65, 111, 112, 127, 128, 129, 1000] {
            let data = vec![b'a'; n];
            for h in [
                hex(&md5(&data)),
                hex(&sha1(&data)),
                hex(&sha256(&data)),
                hex(&sha384(&data)),
                hex(&sha512(&data)),
            ] {
                assert!(seen.insert(h), "a repeated hash at length {n}");
            }
        }
        // a million 'a's: the other published SHA-256 vector
        assert_eq!(
            hex(&sha256(&vec![b'a'; 1_000_000])),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn the_hash_tool_lists_every_algorithm() {
        let o = hash("abc").unwrap();
        assert_eq!(
            o.fields
                .iter()
                .map(|f| f.label.as_str())
                .collect::<Vec<_>>(),
            vec!["MD5", "SHA-1", "SHA-256", "SHA-384", "SHA-512"]
        );
        assert!(hash("").is_err());
    }

    #[test]
    fn passwords_follow_the_flags_and_differ() {
        let p = |s: &str| password(s).unwrap().fields[0].value.clone();
        assert_eq!(p("").chars().count(), 20);
        assert_eq!(p("32").chars().count(), 32);
        assert!(p("40 nosymbols nodigits")
            .chars()
            .all(|c| c.is_ascii_alphabetic()));
        assert!(p("6 pin").chars().all(|c| c.is_ascii_digit()));
        assert_eq!(p("pin").chars().count(), 6);
        assert!(p("64 hex")
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert!(p("30 noupper nolower nosymbols")
            .chars()
            .all(|c| c.is_ascii_digit()));
        assert_ne!(p("24"), p("24"), "two passwords are not the same");
        let o = password("20").unwrap();
        assert_ne!(o.fields[0].value, o.fields[1].value);
        assert!(
            o.fields[2].value.starts_with("about 128"),
            "{}",
            o.fields[2].value
        );
        for bad in [
            "3",
            "129",
            "x",
            "nosymbols nodigits noupper nolower",
            "10 nolowr",
        ] {
            assert!(password(bad).is_err(), "{bad:?}");
        }
    }
}
