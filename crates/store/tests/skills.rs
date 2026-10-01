use super::*;
use crate::bots::{self, Profile};

#[tokio::test]
async fn library_with_per_bot_enablement() {
  let pool = crate::memory().await.unwrap();
  let a = bots::create(&pool, &Profile { name: "A".into(), ..Default::default() }).await.unwrap();
  let s = save(&pool, "Expense report", "d", "1. open", WRITTEN).await.unwrap();
  assert!(enabled(&pool, &a.id).await.unwrap().is_empty());
  enable(&pool, &a.id, &s.id, true).await.unwrap();
  assert_eq!(enabled(&pool, &a.id).await.unwrap().len(), 1);
  let again = save(&pool, "expense REPORT", "d2", "2. file", LEARNED).await.unwrap();
  assert_eq!(again.id, s.id);
  assert_eq!(find(&pool, "/expense report").await.unwrap().unwrap().instructions, "2. file");
}
