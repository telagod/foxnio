//! 公告系统测试

#[cfg(test)]
mod tests {
    use crate::service::announcement::{
        Announcement, AnnouncementType, AnnouncementUpdates, AnnouncementStats
    };
    
    #[test]
    fn test_announcement_types() {
        let types = vec![
            AnnouncementType::Info,
            AnnouncementType::Warning,
            AnnouncementType::Maintenance,
            AnnouncementType::Update,
            AnnouncementType::Promotion,
        ];
        
        assert_eq!(types.len(), 5);
    }
    
    #[test]
    fn test_announcement_creation() {
        let announcement = Announcement {
            id: uuid::Uuid::new_v4(),
            title: "系统维护通知".to_string(),
            content: "系统将于今晚 22:00-24:00 进行维护".to_string(),
            announcement_type: AnnouncementType::Maintenance,
            priority: 10,
            start_time: None,
            end_time: None,
            is_active: true,
            is_pinned: false,
            created_by: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        assert_eq!(announcement.title, "系统维护通知");
        assert_eq!(announcement.announcement_type, AnnouncementType::Maintenance);
        assert!(announcement.is_active);
    }
    
    #[test]
    fn test_announcement_priority_sorting() {
        let mut announcements = vec![
            (1, "普通通知"),
            (10, "紧急通知"),
            (5, "重要通知"),
            (3, "一般通知"),
        ];
        
        // 按优先级降序排序
        announcements.sort_by(|a, b| b.0.cmp(&a.0));
        
        assert_eq!(announcements[0].1, "紧急通知");
        assert_eq!(announcements[1].1, "重要通知");
        assert_eq!(announcements[2].1, "一般通知");
        assert_eq!(announcements[3].1, "普通通知");
    }
    
    #[test]
    fn test_announcement_updates() {
        let updates = AnnouncementUpdates {
            title: Some("新标题".to_string()),
            content: Some("新内容".to_string()),
            announcement_type: Some(AnnouncementType::Warning),
            priority: Some(8),
            start_time: None,
            end_time: None,
            is_active: None,
            is_pinned: Some(true),
        };
        
        assert!(updates.title.is_some());
        assert!(updates.is_pinned.unwrap());
    }
    
    #[test]
    fn test_announcement_stats() {
        let stats = AnnouncementStats {
            total_reads: 100,
            unique_readers: 75,
        };
        
        assert_eq!(stats.total_reads, 100);
        assert!(stats.unique_readers <= stats.total_reads);
    }
    
    #[test]
    fn test_announcement_time_range() {
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::hours(1);
        let end = now + chrono::Duration::hours(24);
        
        // 当前时间应该在时间范围内
        assert!(now >= start);
        assert!(now <= end);
    }
    
    #[test]
    fn test_pinned_announcement() {
        let mut announcements = vec![
            (false, 5, "普通公告1"),
            (true, 3, "置顶公告"),
            (false, 10, "普通公告2"),
        ];
        
        // 置顶优先，然后按优先级排序
        announcements.sort_by(|a, b| {
            match (a.0, b.0) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.1.cmp(&a.1),
            }
        });
        
        assert_eq!(announcements[0].2, "置顶公告");
    }
    
    #[test]
    fn test_announcement_type_messages() {
        let messages = vec![
            (AnnouncementType::Info, "信息通知"),
            (AnnouncementType::Warning, "警告通知"),
            (AnnouncementType::Maintenance, "维护通知"),
            (AnnouncementType::Update, "更新通知"),
            (AnnouncementType::Promotion, "推广通知"),
        ];
        
        for (typ, msg) in messages {
            let announcement = Announcement {
                id: uuid::Uuid::new_v4(),
                title: msg.to_string(),
                content: "内容".to_string(),
                announcement_type: typ.clone(),
                priority: 5,
                start_time: None,
                end_time: None,
                is_active: true,
                is_pinned: false,
                created_by: uuid::Uuid::new_v4(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            
            assert_eq!(announcement.announcement_type, typ);
        }
    }
}
