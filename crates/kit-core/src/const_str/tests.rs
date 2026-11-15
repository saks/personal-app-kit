use super::char_at;

#[test]
fn decodes_every_width_like_chars() {
    let text = "a\u{e9}\u{20ac}\u{1f600}z";
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut decoded = Vec::new();
    while i < bytes.len() {
        let (ch, width) = char_at(bytes, i);
        assert_eq!(width, ch.len_utf8());
        decoded.push(ch);
        i += width;
    }
    assert_eq!(decoded, text.chars().collect::<Vec<_>>());
}

#[test]
fn bad_input_gives_a_char_instead_of_failing() {
    assert_eq!(
        char_at(&[0xED, 0xA0, 0x80], 0).0,
        char::REPLACEMENT_CHARACTER
    );
    assert_eq!(char_at(b"", 0), ('\0', 1));
}
