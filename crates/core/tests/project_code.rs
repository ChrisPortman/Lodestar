use std::collections::HashSet;

use lodestar_core::project::{ProjectCode, ProjectCodeError, suggest_code};

fn suggest(name: &str, taken: &[&str]) -> Option<String> {
    let taken: HashSet<String> = taken.iter().map(|s| (*s).to_owned()).collect();
    suggest_code(name, |code| taken.contains(code)).map(|c| c.as_str().to_owned())
}

#[test]
fn suggestions_follow_fr16() {
    let cases: &[(&str, &[&str], Option<&str>)] = &[
        // The FR-16 examples.
        ("Cake Eater", &[], Some("CAEA")),
        ("Big Red Dog", &[], Some("BIRD")),
        ("Website", &[], Some("WEBS")),
        // Four or more words: first letter of the first four.
        ("Very Big New Website Rebuild", &[], Some("VBNW")),
        ("Alpha Beta Gamma Delta", &[], Some("ABGD")),
        // Filler words and non-letters are ignored; case does not matter.
        ("The Website of the Future", &[], Some("WEFU")),
        ("a plan for 2027!", &[], Some("PLAN")),
        ("Web-site Rebuild", &[], Some("WERE")),
        ("Q3 Budget", &[], Some("QBUD")),
        // A word that runs out of letters hands the rest to the next word.
        ("X Website", &[], Some("XWEB")),
        ("Go Up", &[], Some("GOUP")),
        // Too few letters for four: as many as there are, if at least two.
        ("Go", &[], Some("GO")),
        ("Ab C", &[], Some("ABC")),
        // Nothing usable.
        ("X", &[], None),
        ("1234", &[], None),
        ("", &[], None),
        // Taken: a number is added.
        ("Web Rebuild", &["WERE"], Some("WERE1")),
        ("Web Rebuild", &["WERE", "WERE1", "WERE2"], Some("WERE3")),
        ("Website", &["WEBS1"], Some("WEBS")),
    ];
    for (name, taken, expected) in cases {
        assert_eq!(
            suggest(name, taken).as_deref(),
            *expected,
            "{name:?} with {taken:?} taken"
        );
    }
}

#[test]
fn only_filler_words_are_used_when_nothing_else_remains() {
    assert_eq!(suggest("The End", &[]).as_deref(), Some("END"));
    assert_eq!(suggest("Of The", &[]).as_deref(), Some("OFTH"));
}

#[test]
fn codes_are_validated_per_fr17() {
    for code in ["WEBR", "AB", "A123456789", "WEBR1", "X9"] {
        assert_eq!(ProjectCode::new(code).unwrap().as_str(), code, "{code}");
    }
    for code in [
        "",
        "A",
        "webr",
        "1ABC",
        "AB-C",
        "A1234567890",
        " WEBR",
        "ÉCOLE",
    ] {
        assert_eq!(
            ProjectCode::new(code).unwrap_err(),
            ProjectCodeError::Invalid,
            "{code:?}"
        );
    }
}
