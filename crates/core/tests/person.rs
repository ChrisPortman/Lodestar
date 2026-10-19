use lodestar_core::person::{DisplayNameError, Person};

#[test]
fn a_new_person_has_a_trimmed_display_name_and_version_one() {
    let person = Person::create("  Ada Lovelace ").unwrap();
    assert_eq!(person.display_name().as_str(), "Ada Lovelace");
    assert_eq!(person.version(), 1);
}

#[test]
fn display_name_is_required() {
    assert_eq!(Person::create("   ").unwrap_err(), DisplayNameError::Empty);
}

#[test]
fn display_name_is_at_most_100_characters() {
    assert!(Person::create("é".repeat(100)).is_ok());
    assert_eq!(
        Person::create("a".repeat(101)).unwrap_err(),
        DisplayNameError::TooLong
    );
}

#[test]
fn renaming_produces_the_next_version_with_the_same_id() {
    let before = Person::create("Ada").unwrap();
    let after = before.renamed(" Ada King ").unwrap();
    assert_eq!(after.id(), before.id());
    assert_eq!(after.display_name().as_str(), "Ada King");
    assert_eq!(after.version(), 2);
}

#[test]
fn person_ids_are_uuid_v7() {
    let person = Person::create("Ada").unwrap();
    assert_eq!(person.id().as_uuid().get_version_num(), 7);
}
