use super::*;

#[test]
fn every_class_is_sixteen_square_with_colors() {
  for c in &CLASSES {
    let s = of_class(c.id);
    assert_eq!(s.class, c.id);
    for v in [&s.primary, &s.accent, &s.hair, &s.skin] {
      assert!(hex(v).is_some(), "{}: {v}", c.id);
    }
    let px = pixels(&s);
    assert_eq!(px.len(), SIZE, "{}", c.id);
    assert!(px.iter().all(|r| r.len() == SIZE), "{}", c.id);
    assert!(px.iter().flatten().filter(|p| p.is_some()).count() > 100, "{}", c.id);
  }
}

#[test]
fn spec_round_trip_and_fallbacks() {
  let s = Spec { class: "thief".into(), primary: "#2f5fb3".into(), accent: "#c2185b".into(), hair: "#2b2b2b".into(), skin: "#a8704a".into() };
  assert_eq!(parse(format(&s).trim_start_matches("sprite:")), s);
  // Unknown class: the first one. Bad colors: the class's own.
  assert_eq!(parse("dragon").class, "warrior");
  assert_eq!(parse("wizard;primary=blue").primary, of_class("wizard").primary);
}

#[test]
fn colors_paint_their_letters() {
  let mut s = of_class("warrior");
  s.primary = "#010203".into();
  assert!(pixels(&s).iter().flatten().any(|p| *p == Some(0x010203)));
}

#[test]
fn runs_merge_equal_neighbours() {
  let row = [None, Some(1), Some(1), Some(2), None];
  assert_eq!(runs(&row), vec![(0, 1, None), (1, 2, Some(1)), (3, 1, Some(2)), (4, 1, None)]);
}

#[test]
fn auto_is_stable_and_varied() {
  assert_eq!(auto("agent-1"), auto("agent-1"));
  let classes: std::collections::HashSet<String> = (0..64).map(|i| auto(&format!("agent-{i}")).class).collect();
  assert!(classes.len() >= 6, "{classes:?}");
}

#[test]
fn class_colors_are_offered_swatches() {
  for c in &CLASSES {
    let s = of_class(c.id);
    assert!(CLOTH.contains(&s.primary.as_str()), "{} primary {}", c.id, s.primary);
    assert!(CLOTH.contains(&s.accent.as_str()), "{} accent {}", c.id, s.accent);
    assert!(HAIR.contains(&s.hair.as_str()), "{} hair {}", c.id, s.hair);
    assert!(SKIN.contains(&s.skin.as_str()), "{} skin {}", c.id, s.skin);
  }
}

#[test]
fn small_sizes_show_a_portrait() {
  assert_eq!(frame(96.0), (0..16, 0..16));
  let (rows, cols) = frame(24.0);
  assert_eq!((rows.len(), cols.len()), (12, 12));
}
