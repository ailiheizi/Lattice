use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use nextim_proto::message::Envelope;

/// 对数据计算 SHA-256 哈希
pub fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// 验证消息签名
///
/// 检查 Envelope 中的 signature 是否由 sender 的公钥签发，
/// 同时验证 payload_hash 与实际 payload 的 SHA-256 一致。
pub fn verify_envelope(
    sender_public_key: &[u8],
    envelope: &Envelope,
) -> Result<bool, SignVerifyError> {
    // 1. 提取 payload 字节
    let payload_bytes = extract_payload_bytes(envelope)?;

    // 2. 验证 SHA-256 完整性
    let computed_hash = sha256(&payload_bytes);
    if computed_hash != envelope.payload_hash {
        return Err(SignVerifyError::HashMismatch);
    }

    // 3. 验证 Ed25519 签名（签名对象是 payload_hash）
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
/// 对 payload 计算 SHA-256，然后用私钥签名哈希值。
/// 返回 (signature, payload_hash)。
pub fn sign_envelope(
    signing_key: &ed25519_dalek::SigningKey,
    envelope: &Envelope,
) -> Result<(Vec<u8>, Vec<u8>), SignVerifyError> {
    use ed25519_dalek::Signer;

    let payload_bytes = extract_payload_bytes(envelope)?;
    let payload_hash = sha256(&payload_bytes);
    let signature = signing_key.sign(&payload_hash);

    Ok((signature.to_bytes().to_vec(), payload_hash))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use nextim_proto::message::{
        envelope::Payload, MessageContent, MessageType, PlainPayload,
    };
    use rand::rngs::OsRng;

    fn make_test_envelope() -> Envelope {
        Envelope {
            msg_id: "test-001".to_string(),
            sender_fingerprint: "aabbcc".to_string(),
            recipient_fingerprint: "ddeeff".to_string(),
            timestamp: 1234567890,
            signature: vec![],
            payload_hash: vec![],
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

    #[test]
    fn test_sign_and_verify_envelope() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut envelope = make_test_envelope();

        let (sig, hash) = sign_envelope(&signing_key, &envelope).unwrap();
        envelope.signature = sig;
        envelope.payload_hash = hash;

        let result = verify_envelope(
            signing_key.verifying_key().as_bytes(),
            &envelope,
        );
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_tampered_payload_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let mut envelope = make_test_envelope();

        let (sig, hash) = sign_envelope(&signing_key, &envelope).unwrap();
        envelope.signature = sig;
        envelope.payload_hash = hash;

        // 篡改 payload
        if let Some(Payload::Plain(ref mut p)) = envelope.payload {
            if let Some(ref mut c) = p.content {
                c.text = "tampered".to_string();
            }
        }

        let result = verify_envelope(
            signing_key.verifying_key().as_bytes(),
            &envelope,
        );
        assert!(matches!(result, Err(SignVerifyError::HashMismatch)));
    }

    #[test]
    fn test_wrong_key_fails() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let wrong_key = SigningKey::generate(&mut OsRng);
        let mut envelope = make_test_envelope();

        let (sig, hash) = sign_envelope(&signing_key, &envelope).unwrap();
        envelope.signature = sig;
        envelope.payload_hash = hash;

        let result = verify_envelope(
            wrong_key.verifying_key().as_bytes(),
            &envelope,
        );
        assert!(matches!(
            result,
            Err(SignVerifyError::SignatureVerificationFailed)
        ));
    }

    #[test]
    fn test_sha256() {
        let hash = sha256(b"hello");
        assert_eq!(hash.len(), 32);
        // 确定性
        assert_eq!(hash, sha256(b"hello"));
        // 不同输入不同输出
        assert_ne!(hash, sha256(b"world"));
    }
}
