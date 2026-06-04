use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use nextim_proto::message::Envelope;

/// 对数据计算 SHA-256 哈希
pub fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// 计算 Envelope 的消息哈希。
///
/// 字节布局为：
/// - `u32::to_be_bytes(msg_id.len())` + `msg_id.as_bytes()`
/// - `u32::to_be_bytes(sender_fingerprint.len())` + `sender_fingerprint.as_bytes()`
/// - `u32::to_be_bytes(recipient_fingerprint.len())` + `recipient_fingerprint.as_bytes()`
/// - `u32::to_be_bytes(payload_bytes.len())` + `payload_bytes`
/// - `sort(prev_hashes)` 后，对每个 hash 追加 `u32::to_be_bytes(hash.len())` + `hash`
pub fn compute_msg_hash(envelope: &Envelope) -> Result<Vec<u8>, SignVerifyError> {
    let payload_bytes = extract_payload_bytes(envelope)?;
    let mut prev_hashes = envelope.prev_hashes.clone();
    prev_hashes.sort();
    let mut encoded = Vec::with_capacity(
        envelope.msg_id.len()
            + envelope.sender_fingerprint.len()
            + envelope.recipient_fingerprint.len()
            + payload_bytes.len()
            + prev_hashes.iter().map(|hash| hash.len() + 4).sum::<usize>()
            + 16,
    );

    append_length_prefixed(&mut encoded, envelope.msg_id.as_bytes())?;
    append_length_prefixed(&mut encoded, envelope.sender_fingerprint.as_bytes())?;
    append_length_prefixed(&mut encoded, envelope.recipient_fingerprint.as_bytes())?;
    append_length_prefixed(&mut encoded, &payload_bytes)?;
    for prev_hash in &prev_hashes {
        append_length_prefixed(&mut encoded, prev_hash)?;
    }

    Ok(sha256(&encoded))
}

fn append_length_prefixed(buf: &mut Vec<u8>, bytes: &[u8]) -> Result<(), SignVerifyError> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| SignVerifyError::FieldTooLarge("message field exceeds u32 length".into()))?;
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(bytes);
    Ok(())
}

/// 验证消息签名
///
/// 检查 Envelope 中的 signature 是否由 sender 的公钥签发，
/// 同时验证 payload_hash 与按消息关键字段计算的 msg_hash 一致。
pub fn verify_envelope(
    sender_public_key: &[u8],
    envelope: &Envelope,
) -> Result<bool, SignVerifyError> {
    let computed_hash = compute_msg_hash(envelope)?;
    if computed_hash != envelope.payload_hash {
        return Err(SignVerifyError::HashMismatch);
    }

    let verifying_key = VerifyingKey::from_bytes(
        sender_public_key
            .try_into()
            .map_err(|_| SignVerifyError::InvalidPublicKey)?,
    )
    .map_err(|_| SignVerifyError::InvalidPublicKey)?;

    let sig_bytes: [u8; 64] = envelope
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| SignVerifyError::InvalidSignature)?;
    let signature = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify(&envelope.payload_hash, &signature)
        .map_err(|_| SignVerifyError::SignatureVerificationFailed)?;

    Ok(true)
}

/// 为 Envelope 生成签名和哈希
///
/// 对消息关键字段计算 msg_hash，然后用私钥签名。
/// 返回 (signature, payload_hash)，其中 payload_hash 的语义已升级为 msg_hash。
pub fn sign_envelope(
    signing_key: &ed25519_dalek::SigningKey,
    envelope: &Envelope,
) -> Result<(Vec<u8>, Vec<u8>), SignVerifyError> {
    use ed25519_dalek::Signer;

    let msg_hash = compute_msg_hash(envelope)?;
    let signature = signing_key.sign(&msg_hash);

    Ok((signature.to_bytes().to_vec(), msg_hash))
}

/// 从 Envelope 中提取 payload 字节用于哈希/签名
fn extract_payload_bytes(envelope: &Envelope) -> Result<Vec<u8>, SignVerifyError> {
    use prost::Message;

    match &envelope.payload {
        Some(nextim_proto::message::envelope::Payload::Plain(p)) => {
            let mut buf = Vec::new();
            p.encode(&mut buf)
                .map_err(|e| SignVerifyError::SerializationError(e.to_string()))?;
            Ok(buf)
        }
        Some(nextim_proto::message::envelope::Payload::Encrypted(e)) => {
            let mut buf = Vec::new();
            e.encode(&mut buf)
                .map_err(|e| SignVerifyError::SerializationError(e.to_string()))?;
            Ok(buf)
        }
        None => Err(SignVerifyError::EmptyPayload),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignVerifyError {
    #[error("invalid public key")]
    InvalidPublicKey,

    #[error("invalid signature format")]
    InvalidSignature,

    #[error("signature verification failed")]
    SignatureVerificationFailed,

    #[error("payload hash mismatch")]
    HashMismatch,

    #[error("empty payload")]
    EmptyPayload,

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("field too large: {0}")]
    FieldTooLarge(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use nextim_proto::message::{envelope::Payload, MessageContent, MessageType, PlainPayload};
    use rand::rngs::OsRng;

    fn make_test_envelope() -> Envelope {
        Envelope {
            msg_id: "test-001".to_string(),
            sender_fingerprint: "aabbcc".to_string(),
            recipient_fingerprint: "ddeeff".to_string(),
            timestamp: 1234567890,
            signature: vec![],
            payload_hash: vec![],
            prev_hashes: Vec::new(),
            payload: Some(Payload::Plain(PlainPayload {
                content: Some(MessageContent {
                    r#type: MessageType::Text as i32,
                    text: "hello world".to_string(),
                    media: vec![],
                    media_type: String::new(),
                    reply_to: String::new(),
                }),
            })),
        }
    }

    fn sign_test_envelope(signing_key: &SigningKey) -> Envelope {
        let mut envelope = make_test_envelope();
        let (sig, hash) = sign_envelope(signing_key, &envelope).unwrap();
        envelope.signature = sig;
        envelope.payload_hash = hash;
        envelope
    }

    #[test]
    fn test_sign_and_verify_envelope() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let envelope = sign_test_envelope(&signing_key);

        let result = verify_envelope(signing_key.verifying_key().as_bytes(), &envelope);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_tampered_msg_id_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut envelope = sign_test_envelope(&signing_key);
        envelope.msg_id.push('x');

        let result = verify_envelope(signing_key.verifying_key().as_bytes(), &envelope);
        assert!(matches!(result, Err(SignVerifyError::HashMismatch)));
    }

    #[test]
    fn test_tampered_sender_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut envelope = sign_test_envelope(&signing_key);
        envelope.sender_fingerprint.push('x');

        let result = verify_envelope(signing_key.verifying_key().as_bytes(), &envelope);
        assert!(matches!(result, Err(SignVerifyError::HashMismatch)));
    }

    #[test]
    fn test_tampered_recipient_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut envelope = sign_test_envelope(&signing_key);
        envelope.recipient_fingerprint.push('x');

        let result = verify_envelope(signing_key.verifying_key().as_bytes(), &envelope);
        assert!(matches!(result, Err(SignVerifyError::HashMismatch)));
    }

    #[test]
    fn test_tampered_payload_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut envelope = sign_test_envelope(&signing_key);

        if let Some(Payload::Plain(ref mut p)) = envelope.payload {
            if let Some(ref mut c) = p.content {
                c.text = "tampered".to_string();
            }
        }

        let result = verify_envelope(signing_key.verifying_key().as_bytes(), &envelope);
        assert!(matches!(result, Err(SignVerifyError::HashMismatch)));
    }

    #[test]
    fn test_wrong_key_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let wrong_key = SigningKey::generate(&mut OsRng);
        let envelope = sign_test_envelope(&signing_key);

        let result = verify_envelope(wrong_key.verifying_key().as_bytes(), &envelope);
        assert!(matches!(
            result,
            Err(SignVerifyError::SignatureVerificationFailed)
        ));
    }

    #[test]
    fn test_empty_prev_hashes_preserve_legacy_layout() {
        let envelope = make_test_envelope();
        let payload_bytes = extract_payload_bytes(&envelope).unwrap();
        let mut legacy = Vec::new();
        append_length_prefixed(&mut legacy, envelope.msg_id.as_bytes()).unwrap();
        append_length_prefixed(&mut legacy, envelope.sender_fingerprint.as_bytes()).unwrap();
        append_length_prefixed(&mut legacy, envelope.recipient_fingerprint.as_bytes()).unwrap();
        append_length_prefixed(&mut legacy, &payload_bytes).unwrap();

        assert_eq!(compute_msg_hash(&envelope).unwrap(), sha256(&legacy));
    }

    #[test]
    fn test_prev_hashes_are_signed_deterministically() {
        let mut envelope_a = make_test_envelope();
        envelope_a.prev_hashes = vec![vec![0xBB], vec![0xAA, 0x01]];

        let mut envelope_b = make_test_envelope();
        envelope_b.prev_hashes = vec![vec![0xAA, 0x01], vec![0xBB]];

        let mut envelope_tampered = envelope_b.clone();
        envelope_tampered.prev_hashes.push(vec![0xCC]);

        assert_eq!(compute_msg_hash(&envelope_a).unwrap(), compute_msg_hash(&envelope_b).unwrap());
        assert_ne!(
            compute_msg_hash(&envelope_a).unwrap(),
            compute_msg_hash(&envelope_tampered).unwrap()
        );
    }

    #[test]
    fn test_sha256() {
        let hash = sha256(b"hello");
        assert_eq!(hash.len(), 32);
        assert_eq!(hash, sha256(b"hello"));
        assert_ne!(hash, sha256(b"world"));
    }
}
