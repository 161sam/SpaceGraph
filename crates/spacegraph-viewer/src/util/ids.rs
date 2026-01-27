use std::hash::{Hash, Hasher};

pub fn stable_u32(s: &str) -> u32 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    (h.finish() & 0xFFFF_FFFF) as u32
}

// viewer-side "pretty path" (display only)
pub fn normalize_display_path(p: &str) -> String {
    let mut s = p.replace("/./", "/");
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    let mut parts = Vec::new();
    for part in s.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x),
        }
    }
    let out = format!("/{}", parts.join("/"));
    if out == "/" {
        "/".into()
    } else {
        out
    }
}
