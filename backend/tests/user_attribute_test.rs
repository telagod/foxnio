//! Tests for user attribute service

#[cfg(test)]
mod tests {
    use foxnio::entity::user_attribute_definitions::AttributeType;

    #[test]
    fn test_attribute_type() {
        assert_eq!(AttributeType::Text.as_str(), "text");
        assert_eq!(AttributeType::Textarea.as_str(), "textarea");
        assert_eq!(AttributeType::Number.as_str(), "number");
        assert_eq!(AttributeType::Email.as_str(), "email");
        assert_eq!(AttributeType::Url.as_str(), "url");
        assert_eq!(AttributeType::Date.as_str(), "date");
        assert_eq!(AttributeType::Select.as_str(), "select");
        assert_eq!(AttributeType::MultiSelect.as_str(), "multi_select");

        assert_eq!(AttributeType::parse("text"), AttributeType::Text);
        assert_eq!(AttributeType::parse("textarea"), AttributeType::Textarea);
        assert_eq!(AttributeType::parse("number"), AttributeType::Number);
        assert_eq!(AttributeType::parse("email"), AttributeType::Email);
        assert_eq!(AttributeType::parse("url"), AttributeType::Url);
        assert_eq!(AttributeType::parse("date"), AttributeType::Date);
        assert_eq!(AttributeType::parse("select"), AttributeType::Select);
        assert_eq!(
            AttributeType::parse("multi_select"),
            AttributeType::MultiSelect
        );
        assert_eq!(AttributeType::parse("unknown"), AttributeType::Text);
    }
}
