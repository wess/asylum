use super::*;
use crate::bots::{self, Profile};

#[tokio::test]
async fn direct_is_reused_and_groups_track_members() {
  let pool = crate::memory().await.unwrap();
  let p = Profile { name: "A".into(), ..Default::default() };
  let a = bots::create(&pool, &p).await.unwrap();
  let b = bots::create(&pool, &Profile { name: "B".into(), ..p }).await.unwrap();
  let c1 = direct(&pool, &a.id).await.unwrap();
  let c2 = direct(&pool, &a.id).await.unwrap();
  assert_eq!(c1.id, c2.id);
  let g = create_group(&pool, "launch", &[a.id.clone(), b.id.clone()]).await.unwrap();
  assert_eq!(members(&pool, &g.id).await.unwrap().len(), 2);
  remove_member(&pool, &g.id, &a.id).await.unwrap();
  assert_eq!(members(&pool, &g.id).await.unwrap(), vec![b.id.clone()]);
  bots::delete(&pool, &b.id).await.unwrap();
  assert!(members(&pool, &g.id).await.unwrap().is_empty());
}
