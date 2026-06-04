//! 群组/房间管理

use nextim_proto::group::{
    HistoryVisibility, MemberRole, Room, RoomEvent, RoomEventType, RoomMember, RoomType,
};
use sha2::{Digest, Sha256};

/// 权限检查错误
#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("member not found: {0}")]
    MemberNotFound(String),
    #[error("already a member: {0}")]
    AlreadyMember(String),
    #[error("cannot modify owner")]
    CannotModifyOwner,
}

/// 获取成员角色
pub fn get_member_role(room: &Room, fingerprint: &str) -> Option<MemberRole> {
    room.members
        .iter()
        .find(|m| m.user_fingerprint == fingerprint)
        .map(|m| MemberRole::try_from(m.role).unwrap_or(MemberRole::Member))
}

/// 检查用户是否有权限执行操作
fn check_permission(
    room: &Room,
    actor_fingerprint: &str,
    required_role: MemberRole,
) -> Result<(), RoomError> {
    let actor_role = get_member_role(room, actor_fingerprint)
        .ok_or_else(|| RoomError::MemberNotFound(actor_fingerprint.to_string()))?;

    if role_level(actor_role) >= role_level(required_role) {
        Ok(())
    } else {
        Err(RoomError::PermissionDenied(format!(
            "requires {:?}, has {:?}",
            required_role, actor_role
        )))
    }
}

fn role_level(role: MemberRole) -> i32 {
    match role {
        MemberRole::Member => 0,
        MemberRole::Admin => 1,
        MemberRole::Owner => 2,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 创建新房间
pub fn create_room(
    room_id: &str,
    name: &str,
    room_type: RoomType,
    creator_fingerprint: &str,
    encrypted: bool,
    history_visibility: HistoryVisibility,
) -> Room {
    Room {
        room_id: room_id.to_string(),
        name: name.to_string(),
        r#type: room_type as i32,
        creator_fingerprint: creator_fingerprint.to_string(),
        encrypted,
        history_visibility: history_visibility as i32,
        created_at: now_ms(),
        members: vec![RoomMember {
            user_fingerprint: creator_fingerprint.to_string(),
            role: MemberRole::Owner as i32,
            joined_at: now_ms(),
        }],
    }
}

/// 添加成员
pub fn add_member(
    room: &mut Room,
    actor_fingerprint: &str,
    new_member_fingerprint: &str,
) -> Result<RoomEvent, RoomError> {
    // 频道任何人可加入，群聊需要 member 以上权限邀请
    if RoomType::try_from(room.r#type).unwrap_or(RoomType::Group) != RoomType::Channel {
        check_permission(room, actor_fingerprint, MemberRole::Member)?;
    }

    if room.members.iter().any(|m| m.user_fingerprint == new_member_fingerprint) {
        return Err(RoomError::AlreadyMember(new_member_fingerprint.to_string()));
    }

    room.members.push(RoomMember {
        user_fingerprint: new_member_fingerprint.to_string(),
        role: MemberRole::Member as i32,
        joined_at: now_ms(),
    });

    Ok(RoomEvent {
        room_id: room.room_id.clone(),
        actor_fingerprint: actor_fingerprint.to_string(),
        r#type: RoomEventType::MemberJoin as i32,
        target_fingerprint: new_member_fingerprint.to_string(),
        timestamp: now_ms(),
        signature: vec![],
        prev_hashes: Vec::new(),
        msg_hash: Vec::new(),
    })
}

/// 移除成员（踢人）
pub fn kick_member(
    room: &mut Room,
    actor_fingerprint: &str,
    target_fingerprint: &str,
) -> Result<RoomEvent, RoomError> {
    check_permission(room, actor_fingerprint, MemberRole::Admin)?;

    let target_role = get_member_role(room, target_fingerprint)
        .ok_or_else(|| RoomError::MemberNotFound(target_fingerprint.to_string()))?;

    if target_role == MemberRole::Owner {
        return Err(RoomError::CannotModifyOwner);
    }

    // admin 不能踢 admin，只有 owner 可以
    let actor_role = get_member_role(room, actor_fingerprint).unwrap();
    if target_role == MemberRole::Admin && actor_role != MemberRole::Owner {
        return Err(RoomError::PermissionDenied("only owner can kick admin".into()));
    }

    room.members.retain(|m| m.user_fingerprint != target_fingerprint);

    Ok(RoomEvent {
        room_id: room.room_id.clone(),
        actor_fingerprint: actor_fingerprint.to_string(),
        r#type: RoomEventType::MemberKick as i32,
        target_fingerprint: target_fingerprint.to_string(),
        timestamp: now_ms(),
        signature: vec![],
        prev_hashes: Vec::new(),
        msg_hash: Vec::new(),
    })
}

/// 成员主动离开
pub fn leave_room(
    room: &mut Room,
    member_fingerprint: &str,
) -> Result<RoomEvent, RoomError> {
    let role = get_member_role(room, member_fingerprint)
        .ok_or_else(|| RoomError::MemberNotFound(member_fingerprint.to_string()))?;

    if role == MemberRole::Owner {
        return Err(RoomError::PermissionDenied("owner cannot leave, transfer ownership first".into()));
    }

    room.members.retain(|m| m.user_fingerprint != member_fingerprint);

    Ok(RoomEvent {
        room_id: room.room_id.clone(),
        actor_fingerprint: member_fingerprint.to_string(),
        r#type: RoomEventType::MemberLeave as i32,
        target_fingerprint: member_fingerprint.to_string(),
        timestamp: now_ms(),
        signature: vec![],
        prev_hashes: Vec::new(),
        msg_hash: Vec::new(),
    })
}

/// 修改成员角色
pub fn change_role(
    room: &mut Room,
    actor_fingerprint: &str,
    target_fingerprint: &str,
    new_role: MemberRole,
) -> Result<RoomEvent, RoomError> {
    check_permission(room, actor_fingerprint, MemberRole::Owner)?;

    if target_fingerprint == room.creator_fingerprint && new_role != MemberRole::Owner {
        return Err(RoomError::CannotModifyOwner);
    }

    let member = room
        .members
        .iter_mut()
        .find(|m| m.user_fingerprint == target_fingerprint)
        .ok_or_else(|| RoomError::MemberNotFound(target_fingerprint.to_string()))?;

    member.role = new_role as i32;

    Ok(RoomEvent {
        room_id: room.room_id.clone(),
        actor_fingerprint: actor_fingerprint.to_string(),
        r#type: RoomEventType::RoleChange as i32,
        target_fingerprint: target_fingerprint.to_string(),
        timestamp: now_ms(),
        signature: vec![],
        prev_hashes: Vec::new(),
        msg_hash: Vec::new(),
    })
}

pub fn compute_event_hash(event: &RoomEvent) -> Vec<u8> {
    let mut prev_hashes = event.prev_hashes.clone();
    prev_hashes.sort();

    let mut encoded = Vec::new();
    append_length_prefixed(&mut encoded, event.room_id.as_bytes());
    append_length_prefixed(&mut encoded, event.actor_fingerprint.as_bytes());
    encoded.extend_from_slice(&event.r#type.to_be_bytes());
    append_length_prefixed(&mut encoded, event.target_fingerprint.as_bytes());
    encoded.extend_from_slice(&event.timestamp.to_be_bytes());
    for prev_hash in prev_hashes {
        append_length_prefixed(&mut encoded, &prev_hash);
    }

    let mut hasher = Sha256::new();
    hasher.update(encoded);
    hasher.finalize().to_vec()
}

pub fn history_cutoff(room: &Room, member_fingerprint: &str) -> Option<u64> {
    let visibility = HistoryVisibility::try_from(room.history_visibility)
        .unwrap_or(HistoryVisibility::JoinOnly);

    match visibility {
        HistoryVisibility::Full => Some(0),
        HistoryVisibility::JoinOnly => room
            .members
            .iter()
            .find(|m| m.user_fingerprint == member_fingerprint)
            .map(|m| m.joined_at),
    }
}

fn append_length_prefixed(buf: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(bytes);
}

/// 判断用户是否可以查看历史消息
pub fn can_view_history(room: &Room, member_fingerprint: &str) -> bool {
    history_cutoff(room, member_fingerprint).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_room() -> Room {
        create_room(
            "room-1",
            "Test Group",
            RoomType::Group,
            "owner-fp",
            false,
            HistoryVisibility::Full,
        )
    }

    #[test]
    fn test_create_room() {
        let room = make_room();
        assert_eq!(room.members.len(), 1);
        assert_eq!(room.members[0].user_fingerprint, "owner-fp");
        assert_eq!(room.members[0].role, MemberRole::Owner as i32);
    }

    #[test]
    fn test_add_member() {
        let mut room = make_room();
        let event = add_member(&mut room, "owner-fp", "alice-fp").unwrap();
        assert_eq!(room.members.len(), 2);
        assert_eq!(event.r#type, RoomEventType::MemberJoin as i32);
    }

    #[test]
    fn test_add_duplicate_member() {
        let mut room = make_room();
        add_member(&mut room, "owner-fp", "alice-fp").unwrap();
        let result = add_member(&mut room, "owner-fp", "alice-fp");
        assert!(matches!(result, Err(RoomError::AlreadyMember(_))));
    }

    #[test]
    fn test_kick_member() {
        let mut room = make_room();
        add_member(&mut room, "owner-fp", "alice-fp").unwrap();
        assert_eq!(room.members.len(), 2);

        let event = kick_member(&mut room, "owner-fp", "alice-fp").unwrap();
        assert_eq!(room.members.len(), 1);
        assert_eq!(event.r#type, RoomEventType::MemberKick as i32);
    }

    #[test]
    fn test_cannot_kick_owner() {
        let mut room = make_room();
        add_member(&mut room, "owner-fp", "alice-fp").unwrap();
        change_role(&mut room, "owner-fp", "alice-fp", MemberRole::Admin).unwrap();

        let result = kick_member(&mut room, "alice-fp", "owner-fp");
        assert!(matches!(result, Err(RoomError::CannotModifyOwner)));
    }

    #[test]
    fn test_member_cannot_kick() {
        let mut room = make_room();
        add_member(&mut room, "owner-fp", "alice-fp").unwrap();
        add_member(&mut room, "owner-fp", "bob-fp").unwrap();

        let result = kick_member(&mut room, "alice-fp", "bob-fp");
        assert!(matches!(result, Err(RoomError::PermissionDenied(_))));
    }

    #[test]
    fn test_leave_room() {
        let mut room = make_room();
        add_member(&mut room, "owner-fp", "alice-fp").unwrap();

        let event = leave_room(&mut room, "alice-fp").unwrap();
        assert_eq!(room.members.len(), 1);
        assert_eq!(event.r#type, RoomEventType::MemberLeave as i32);
    }

    #[test]
    fn test_owner_cannot_leave() {
        let mut room = make_room();
        let result = leave_room(&mut room, "owner-fp");
        assert!(matches!(result, Err(RoomError::PermissionDenied(_))));
    }

    #[test]
    fn test_change_role() {
        let mut room = make_room();
        add_member(&mut room, "owner-fp", "alice-fp").unwrap();

        change_role(&mut room, "owner-fp", "alice-fp", MemberRole::Admin).unwrap();
        let role = get_member_role(&room, "alice-fp").unwrap();
        assert_eq!(role, MemberRole::Admin);
    }

    #[test]
    fn test_only_owner_can_change_role() {
        let mut room = make_room();
        add_member(&mut room, "owner-fp", "alice-fp").unwrap();
        add_member(&mut room, "owner-fp", "bob-fp").unwrap();
        change_role(&mut room, "owner-fp", "alice-fp", MemberRole::Admin).unwrap();

        let result = change_role(&mut room, "alice-fp", "bob-fp", MemberRole::Admin);
        assert!(matches!(result, Err(RoomError::PermissionDenied(_))));
    }

    #[test]
    fn test_history_visibility() {
        let room = make_room();
        assert!(can_view_history(&room, "anyone"));

        let join_only_room = create_room(
            "room-2",
            "Private",
            RoomType::Group,
            "owner-fp",
            false,
            HistoryVisibility::JoinOnly,
        );
        assert!(can_view_history(&join_only_room, "owner-fp"));
        assert!(!can_view_history(&join_only_room, "outsider"));
    }

    #[test]
    fn test_compute_event_hash_changes_with_parents() {
        let mut event = RoomEvent {
            room_id: "room-1".to_string(),
            actor_fingerprint: "owner-fp".to_string(),
            r#type: RoomEventType::MemberJoin as i32,
            target_fingerprint: "alice-fp".to_string(),
            timestamp: 1234,
            signature: vec![],
            prev_hashes: vec![b"b".to_vec(), b"a".to_vec()],
            msg_hash: Vec::new(),
        };

        let first = compute_event_hash(&event);
        event.prev_hashes.push(b"c".to_vec());
        let second = compute_event_hash(&event);

        assert_ne!(first, second);
    }

    #[test]
    fn test_history_cutoff_for_join_only_member() {
        let room = create_room(
            "room-3",
            "Private",
            RoomType::Group,
            "owner-fp",
            false,
            HistoryVisibility::JoinOnly,
        );

        let joined_at = history_cutoff(&room, "owner-fp").expect("member cutoff");
        assert!(joined_at > 0);
        assert!(history_cutoff(&room, "outsider").is_none());
    }
}
