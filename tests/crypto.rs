#![cfg(feature = "crypto")]
use rustwire_mc::{
    connection::Connection,
    crypto::{encryption_response, offline_uuid, server_hash, signed_hex, Cipher},
    frame::RawPacket,
    Limits, Version,
};
fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
#[test]
fn signed_sha1_java_examples() {
    assert_eq!(
        server_hash("Notch", &[], &[]),
        "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48"
    );
    assert_eq!(
        server_hash("jeb_", &[], &[]),
        "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1"
    );
    assert_eq!(
        server_hash("simon", &[], &[]),
        "88e16a1019277b15d58faf0541e11910eb756f6"
    );
    assert_eq!(signed_hex(&[0]), "0");
    assert_eq!(signed_hex(&[255]), "-1");
}
#[test]
fn cipher_stream_chunking_is_continuous() {
    let secret = [0x42; 16];
    let plain: Vec<u8> = (0..=255).cycle().take(4096).collect();
    let mut whole = plain.clone();
    Cipher::new(&secret).encrypt(&mut whole);
    for size in [1, 2, 15, 16, 17, 255, 256, 1024] {
        let mut data = plain.clone();
        let mut c = Cipher::new(&secret);
        for chunk in data.chunks_mut(size) {
            c.encrypt(chunk);
        }
        assert_eq!(data, whole);
        let mut c = Cipher::new(&secret);
        for chunk in data.chunks_mut(size) {
            c.decrypt(chunk);
        }
        assert_eq!(data, plain);
    }
}
#[test]
fn cfb8_independent_openssl_fixture() {
    // AES-128-CFB8 with IV equal to key, as required by Minecraft.
    let key: [u8; 16] = hex("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
    let mut plaintext = hex("00112233445566778899aabbccddeeff");
    Cipher::new(&key).encrypt(&mut plaintext);
    assert_eq!(plaintext, hex("0a3229c3909dfb17b1f4d635a00176a3"));
}
#[test]
fn offline_uuid_matches_java_name_uuid() {
    assert_eq!(
        offline_uuid("Notch"),
        hex("b50ad385829d3141a2167e7d7539ba7f").as_slice()
    );
}
#[test]
fn rsa_response_decrypts_and_hides_secrets() {
    use rsa::{pkcs8::EncodePublicKey, Pkcs1v15Encrypt, RsaPrivateKey};
    let private = RsaPrivateKey::new(&mut rand::rngs::OsRng, 1024).unwrap();
    let der = private.to_public_key().to_public_key_der().unwrap();
    let response = encryption_response(der.as_bytes(), b"verify").unwrap();
    assert_eq!(
        private
            .decrypt(Pkcs1v15Encrypt, &response.encrypted_secret)
            .unwrap(),
        response.shared_secret
    );
    assert_eq!(
        private
            .decrypt(Pkcs1v15Encrypt, &response.encrypted_verify_token)
            .unwrap(),
        b"verify"
    );
    assert!(format!("{response:?}").contains("REDACTED"));
    assert!(encryption_response(b"invalid", b"verify").is_err());
}
#[test]
fn encrypted_frames_cross_boundaries() {
    use std::{net::TcpListener, time::Duration};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut c = Connection::new(stream, Version::V1_21, Limits::default());
        c.enable_encryption(&[7; 16]).unwrap();
        for i in 0..10 {
            let p = c.receive().unwrap();
            assert_eq!(p.id, i);
            c.send(&p).unwrap();
        }
    });
    let mut c = Connection::connect(
        address,
        Version::V1_21,
        Duration::from_secs(3),
        Limits::default(),
    )
    .unwrap();
    c.enable_encryption(&[7; 16]).unwrap();
    assert!(c.enable_encryption(&[7; 16]).is_err());
    for i in 0..10 {
        let p = RawPacket::new(i, vec![i as u8; 100 + i as usize]);
        c.send(&p).unwrap();
        assert_eq!(c.receive().unwrap(), p);
    }
    thread.join().unwrap();
}

#[test]
fn server_hash_uses_java_latin1_not_utf8() {
    // Independently generated with Java's ISO_8859_1 encoder, MessageDigest
    // SHA-1 and signed BigInteger. One supplementary scalar becomes one '?'.
    let secret = [0, 1, 2, 3];
    let public_key = [48, 1, 2, 255];
    for (id, expected) in [
        ("café", "-520fd4ed20b98c661bbb90ede1e7771c324ae76e"),
        ("a🌏b", "-4826efe99a064276b5bf39b769404d53e6ddf317"),
        ("snowman☃", "-62b8fb228872141733d03d20749446023d4ec84"),
    ] {
        assert_eq!(server_hash(id, &secret, &public_key), expected);
    }
    assert_eq!(
        server_hash("a🌏b", &secret, &public_key),
        server_hash("a?b", &secret, &public_key)
    );
}
