use super::*;

#[test]
fn trailing_tokens() {
  assert_eq!(token("hey @sco"), Some(('@', "@sco".into())));
  assert_eq!(token("/week"), Some(('/', "/week".into())));
  assert_eq!(token("hey there"), None);
  assert_eq!(token("mail me@x"), None);
}
