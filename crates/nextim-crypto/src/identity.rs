use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use nextim_proto::identity::{DeviceInfo, Identity, IdentityCard};

/// 用户主密钥对
pub struct MasterKeyPair {
    /// Ed25519 签名私钥
    signing_key: SigningKey,
    /// Curve25519 加密私钥
    encryption_key: StaticSecret,
}

impl MasterKeyPair {
    /// 生成新的主密钥对
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let encryption_key = StaticSecret::random_from_rng(OsRng);
        Self {
            signing_key,
            encryption_key,
        }
    }

    /// 从已有私钥恢复
    pub fn from_bytes(signing_bytes: &[u8; 32], encryption_bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(signing_bytes);
        let encryption_key = StaticSecret::from(*encryption_bytes);
        Self {
            signing_key,
            encryption_key,
        }
    }

    /// 获取 Ed25519 签名公钥
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// 获取 Curve25519 加密公钥
    pub fn encryption_public_key(&self) -> X25519PublicKey {
        X25519PublicKey::from(&self.encryption_key)
    }

    /// 计算公钥指纹: SHA-256(ed25519_public_key) 的十六进制
    pub fn fingerprint(&self) -> String {
        compute_fingerprint(self.verifying_key().as_bytes())
    }

    /// 对数据签名
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        let sig = self.signing_key.sign(data);
        sig.to_bytes().to_vec()
    }

    /// 导出签名私钥字节（敏感操作）
    pub fn signing_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// 导出加密私钥字节（敏感操作）
    pub fn encryption_key_bytes(&self) -> [u8; 32] {
        self.encryption_key.to_bytes()
    }

    /// 生成 Identity protobuf 消息
    pub fn to_identity(&self, display_name: &str) -> Identity {
        Identity {
            ed25519_public_key: self.verifying_key().as_bytes().to_vec(),
            curve25519_public_key: self.encryption_public_key().as_bytes().to_vec(),
            fingerprint: self.fingerprint(),
            display_name: display_name.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// 生成身份卡片
    pub fn to_identity_card(
        &self,
        display_name: &str,
        store_address: &str,
        proxy_store_address: Option<&str>,
    ) -> IdentityCard {
        IdentityCard {
            identity: Some(self.to_identity(display_name)),
            store_address: store_address.to_string(),
            proxy_store_address: proxy_store_address.unwrap_or_default().to_string(),
        }
    }

    /// 为设备密钥签名，生成 DeviceInfo
    pub fn sign_device(
        &self,
        device_id: &str,
        device_name: &str,
        device_signing_key: &VerifyingKey,
        device_encryption_key: &X25519PublicKey,
    ) -> DeviceInfo {
        // 签名内容 = device_ed25519_key || device_curve25519_key
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(device_signing_key.as_bytes());
        sign_data.extend_from_slice(device_encryption_key.as_bytes());
        let signature = self.sign(&sign_data);

        DeviceInfo {
            device_id: device_id.to_string(),
            user_fingerprint: self.fingerprint(),
            device_ed25519_key: device_signing_key.as_bytes().to_vec(),
            device_curve25519_key: device_encryption_key.as_bytes().to_vec(),
            signature,
            device_name: device_name.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }
}

impl Drop for MasterKeyPair {
    fn drop(&mut self) {
        // SigningKey 内部实现了 Zeroize
        // StaticSecret 也实现了 Zeroize
    }
}

/// 设备密钥对（每个设备独立生成）
pub struct DeviceKeyPair {
    signing_key: SigningKey,
    encryption_key: StaticSecret,
}

impl DeviceKeyPair {
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut OsRng),
            encryption_key: StaticSecret::random_from_rng(OsRng),
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn encryption_public_key(&self) -> X25519PublicKey {
        X25519PublicKey::from(&self.encryption_key)
    }

    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.signing_key.sign(data).to_bytes().to_vec()
    }
}

/// 计算公钥指纹: SHA-256 十六进制
pub fn compute_fingerprint(public_key_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key_bytes);
    hex::encode(hasher.finalize())
}

/// 验证设备签名是否由指定主密钥签发
pub fn verify_device_signature(
    master_public_key: &[u8],
    device: &DeviceInfo,
) -> Result<bool, ed25519_dalek::SignatureError> {
    let verifying_key = VerifyingKey::from_bytes(
        master_public_key
            .try_into()
            .map_err(|_| ed25519_dalek::SignatureError::new())?,
    )?;

    let mut sign_data = Vec::new();
    sign_data.extend_from_slice(&device.device_ed25519_key);
    sign_data.extend_from_slice(&device.device_curve25519_key);

    let sig_bytes: [u8; 64] = device
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| ed25519_dalek::SignatureError::new())?;
    let signature = Signature::from_bytes(&sig_bytes);

    verifying_key.verify(&sign_data, &signature)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_master_keypair() {
        let kp = MasterKeyPair::generate();
        let fingerprint = kp.fingerprint();
        assert_eq!(fingerprint.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_sign_and_verify() {
        let kp = MasterKeyPair::generate();
        let data = b"hello nextim";
        let sig = kp.sign(data);

        let verifying_key = kp.verifying_key();
        let sig_bytes: [u8; 64] = sig.as_slice().try_into().unwrap();
        let signature = Signature::from_bytes(&sig_bytes);
        assert!(verifying_key.verify(data, &signature).is_ok());
    }

    #[test]
    fn test_fingerprint_deterministic() {
        let kp = MasterKeyPair::generate();
        assert_eq!(kp.fingerprint(), kp.fingerprint());
    }

    #[test]
    fn test_from_bytes_roundtrip() {
        let kp = MasterKeyPair::generate();
        let signing_bytes = kp.signing_key_bytes();
        let encryption_bytes = kp.encryption_key_bytes();

        let kp2 = MasterKeyPair::from_bytes(&signing_bytes, &encryption_bytes);
        assert_eq!(kp.fingerprint(), kp2.fingerprint());
    }

    #[test]
    fn test_device_signing() {
        let master = MasterKeyPair::generate();
        let device = DeviceKeyPair::generate();

        let device_info = master.sign_device(
            "device-001",
            "Test Phone",
            &device.verifying_key(),
            &device.encryption_public_key(),
        );

        let result = verify_device_signature(master.verifying_key().as_bytes(), &device_info);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_identity_card() {
        let kp = MasterKeyPair::generate();
        let card = kp.to_identity_card("Alice", "ws://127.0.0.1:8080", None);
        let identity = card.identity.unwrap();
        assert_eq!(identity.display_name, "Alice");
        assert_eq!(identity.fingerprint, kp.fingerprint());
        assert_eq!(card.store_address, "ws://127.0.0.1:8080");
    }
}
