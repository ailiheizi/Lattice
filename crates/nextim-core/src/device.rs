//! 多设备管理

use nextim_proto::identity::DeviceInfo;

/// 设备管理器
pub struct DeviceManager {
    /// 当前用户指纹
    user_fingerprint: String,
    /// 已注册设备列表
    devices: Vec<DeviceInfo>,
}

impl DeviceManager {
    pub fn new(user_fingerprint: &str) -> Self {
        Self {
            user_fingerprint: user_fingerprint.to_string(),
            devices: Vec::new(),
        }
    }

    /// 注册新设备
    pub fn register_device(&mut self, device: DeviceInfo) -> Result<(), DeviceError> {
        if device.user_fingerprint != self.user_fingerprint {
            return Err(DeviceError::WrongUser);
        }
        if self.devices.iter().any(|d| d.device_id == device.device_id) {
            return Err(DeviceError::AlreadyRegistered(device.device_id.clone()));
        }
        self.devices.push(device);
        Ok(())
    }

    /// 注销设备
    pub fn unregister_device(&mut self, device_id: &str) -> Result<DeviceInfo, DeviceError> {
        let idx = self
            .devices
            .iter()
            .position(|d| d.device_id == device_id)
            .ok_or_else(|| DeviceError::NotFound(device_id.to_string()))?;
        Ok(self.devices.remove(idx))
    }

    /// 获取所有设备
    pub fn devices(&self) -> &[DeviceInfo] {
        &self.devices
    }

    /// 获取指定设备
    pub fn get_device(&self, device_id: &str) -> Option<&DeviceInfo> {
        self.devices.iter().find(|d| d.device_id == device_id)
    }

    /// 设备数量
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// 从已有设备列表恢复
    pub fn from_devices(user_fingerprint: &str, devices: Vec<DeviceInfo>) -> Self {
        Self {
            user_fingerprint: user_fingerprint.to_string(),
            devices,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("device belongs to a different user")]
    WrongUser,
    #[error("device already registered: {0}")]
    AlreadyRegistered(String),
    #[error("device not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_device(id: &str, user_fp: &str) -> DeviceInfo {
        DeviceInfo {
            device_id: id.to_string(),
            user_fingerprint: user_fp.to_string(),
            device_ed25519_key: vec![1, 2, 3],
            device_curve25519_key: vec![4, 5, 6],
            signature: vec![7, 8, 9],
            device_name: format!("Device {id}"),
            created_at: 1000,
        }
    }

    #[test]
    fn test_register_device() {
        let mut mgr = DeviceManager::new("user-fp");
        let device = make_device("dev-1", "user-fp");
        mgr.register_device(device).unwrap();
        assert_eq!(mgr.device_count(), 1);
    }

    #[test]
    fn test_register_wrong_user() {
        let mut mgr = DeviceManager::new("user-fp");
        let device = make_device("dev-1", "other-user");
        let result = mgr.register_device(device);
        assert!(matches!(result, Err(DeviceError::WrongUser)));
    }

    #[test]
    fn test_register_duplicate() {
        let mut mgr = DeviceManager::new("user-fp");
        mgr.register_device(make_device("dev-1", "user-fp"))
            .unwrap();
        let result = mgr.register_device(make_device("dev-1", "user-fp"));
        assert!(matches!(result, Err(DeviceError::AlreadyRegistered(_))));
    }

    #[test]
    fn test_unregister_device() {
        let mut mgr = DeviceManager::new("user-fp");
        mgr.register_device(make_device("dev-1", "user-fp"))
            .unwrap();
        mgr.register_device(make_device("dev-2", "user-fp"))
            .unwrap();

        let removed = mgr.unregister_device("dev-1").unwrap();
        assert_eq!(removed.device_id, "dev-1");
        assert_eq!(mgr.device_count(), 1);
    }

    #[test]
    fn test_unregister_not_found() {
        let mut mgr = DeviceManager::new("user-fp");
        let result = mgr.unregister_device("nonexistent");
        assert!(matches!(result, Err(DeviceError::NotFound(_))));
    }

    #[test]
    fn test_get_device() {
        let mut mgr = DeviceManager::new("user-fp");
        mgr.register_device(make_device("dev-1", "user-fp"))
            .unwrap();

        assert!(mgr.get_device("dev-1").is_some());
        assert!(mgr.get_device("dev-999").is_none());
    }

    #[test]
    fn test_multiple_devices() {
        let mut mgr = DeviceManager::new("user-fp");
        for i in 0..5 {
            mgr.register_device(make_device(&format!("dev-{i}"), "user-fp"))
                .unwrap();
        }
        assert_eq!(mgr.device_count(), 5);
        assert_eq!(mgr.devices().len(), 5);
    }
}
