use super::*;

// Cadence parsing is the part a person touches, and the part that silently does
// the wrong thing if it guesses.

#[test]
fn cadences_are_read_in_the_words_people_type() {
    assert_eq!(cadence("30m"), Some(30));
    assert_eq!(cadence("1h"), Some(60));
    assert_eq!(cadence("2 hours"), Some(120));
    assert_eq!(cadence("1d"), Some(1440));
    assert_eq!(cadence("1 week"), Some(10080));
    assert_eq!(cadence(" 15 MINUTES "), Some(15));
}

#[test]
fn nonsense_is_refused_rather_than_guessed() {
    // Guessing here means a job firing on a cadence nobody chose, unattended.
    assert_eq!(cadence(""), None);
    assert_eq!(cadence("soon"), None);
    assert_eq!(cadence("1"), None);
    assert_eq!(cadence("h"), None);
    assert_eq!(cadence("0h"), None);
    assert_eq!(cadence("-3h"), None);
    assert_eq!(cadence("1 fortnight"), None);
}

#[test]
fn a_cadence_round_trips_through_the_words_it_came_from() {
    for text in ["30m", "1h", "6h", "1d", "2d", "1w"] {
        let minutes = cadence(text).expect(text);
        assert_eq!(human(minutes), text, "{text}");
    }
}

#[test]
fn odd_cadences_still_read_as_minutes() {
    // 90 minutes is not a whole number of hours; saying "1h" would be a lie.
    assert_eq!(human(90), "90m");
}
