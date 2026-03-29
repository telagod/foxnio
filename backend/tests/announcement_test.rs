#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::all)]
//! Tests for announcement service

#[cfg(test)]
mod tests {
    use foxnio::entity::announcements::AnnouncementStatus;

    #[test]
    fn test_announcement_status() {
        assert_eq!(AnnouncementStatus::Draft.as_str(), "draft");
        assert_eq!(AnnouncementStatus::Active.as_str(), "active");
        assert_eq!(AnnouncementStatus::Archived.as_str(), "archived");

        assert_eq!(
            AnnouncementStatus::parse("draft"),
            AnnouncementStatus::Draft
        );
        assert_eq!(
            AnnouncementStatus::parse("active"),
            AnnouncementStatus::Active
        );
        assert_eq!(
            AnnouncementStatus::parse("archived"),
            AnnouncementStatus::Archived
        );
        assert_eq!(
            AnnouncementStatus::parse("unknown"),
            AnnouncementStatus::Draft
        );
    }
}
