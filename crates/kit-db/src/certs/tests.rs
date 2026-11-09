use super::*;

#[test]
fn pem_lines_are_64_wide_and_framed() {
    let mut out = String::new();
    write_pem_cert(&mut out, &[0u8; 100]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.first(), Some(&"-----BEGIN CERTIFICATE-----"));
    assert_eq!(lines.last(), Some(&"-----END CERTIFICATE-----"));
    let body = &lines[1..lines.len() - 1];
    assert_eq!(body.len(), 3, "100 bytes → 136 base64 chars → 64+64+8");
    assert!(body[..2].iter().all(|line| line.len() == 64));
    assert_eq!(
        body.concat(),
        base64::engine::general_purpose::STANDARD.encode([0u8; 100])
    );
}

#[test]
fn writes_a_non_empty_bundle_without_touching_the_env() {
    let before = std::env::var_os(SSL_CERT_FILE_ENV);
    let dir = tempfile::tempdir().unwrap();
    let path = write_mozilla_ca_file(&dir.path().join("nested")).unwrap();
    assert_eq!(path.file_name().unwrap(), CA_FILE_NAME);
    let pem = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        pem.matches("-----BEGIN CERTIFICATE-----").count(),
        webpki_root_certs::TLS_SERVER_ROOT_CERTS.len()
    );
    assert!(webpki_root_certs::TLS_SERVER_ROOT_CERTS.len() > 50);
    assert_eq!(std::env::var_os(SSL_CERT_FILE_ENV), before);
}
