//! `const` string helpers for compile-time checks of literals (`chars()` is
//! not `const`). Shared by kit-core's identity rules and
//! tauri-plugin-kit-android's `LibName`.

/// The `char` starting at byte `i` of valid UTF-8 (`i` on a char boundary),
/// and its width in bytes.
///
/// Meant for error reports on `&str` input: on bytes that are not valid
/// UTF-8, or `i` past the end, it returns some `char` (possibly
/// [`char::REPLACEMENT_CHARACTER`]) rather than failing.
///
/// ```
/// const AT: (char, usize) = kit_core::const_str::char_at("aé".as_bytes(), 1);
/// assert_eq!(AT, ('é', 2));
/// ```
pub const fn char_at(bytes: &[u8], i: usize) -> (char, usize) {
    let lead = byte_at(bytes, i);
    let (code, width) = if lead < 0x80 {
        (lead, 1)
    } else if lead < 0xE0 {
        ((lead & 0x1F) << 6 | (byte_at(bytes, i + 1) & 0x3F), 2)
    } else if lead < 0xF0 {
        (
            (lead & 0x0F) << 12
                | (byte_at(bytes, i + 1) & 0x3F) << 6
                | (byte_at(bytes, i + 2) & 0x3F),
            3,
        )
    } else {
        (
            (lead & 0x07) << 18
                | (byte_at(bytes, i + 1) & 0x3F) << 12
                | (byte_at(bytes, i + 2) & 0x3F) << 6
                | (byte_at(bytes, i + 3) & 0x3F),
            4,
        )
    };
    match char::from_u32(code) {
        Some(ch) => (ch, width),
        None => (char::REPLACEMENT_CHARACTER, width),
    }
}

const fn byte_at(bytes: &[u8], i: usize) -> u32 {
    if i < bytes.len() { bytes[i] as u32 } else { 0 }
}

#[cfg(test)]
mod tests;
