use super::super::super::hosts;

#[test]
fn alias_matching_is_case_and_punctuation_insensitive() {
    fn matches(alias: &str, query: &str) -> bool {
        let a = hosts::normalize(alias);
        let q = hosts::normalize(query);
        !q.is_empty() && (a == q || a.contains(&q))
    }
    assert!(matches("Living Room Right Lamp", "living room"));
    assert!(matches("Coat-Rack Lights", "coat rack"));
    assert!(matches("Kitchen Wax Melter", "KITCHEN"));
    assert!(!matches("Back Porch Reading Lamp", "coat rack"));
}
