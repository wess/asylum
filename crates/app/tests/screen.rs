use super::*;

#[test]
fn wallpaper_darkens_at_night() {
  let (day, _) = wallpaper(12);
  let (night, _) = wallpaper(23);
  assert!(day.l > night.l);
  assert_eq!(named("enter"), Some("Enter"));
  assert_eq!(named("q"), None);
}
