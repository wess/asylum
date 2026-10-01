use super::*;
use crate::bots::{self, Profile};

#[tokio::test]
async fn due_and_webhook_key() {
  let pool = crate::memory().await.unwrap();
  let b = bots::create(&pool, &Profile { name: "A".into(), ..Default::default() }).await.unwrap();
  let r = create(&pool, New {
    bot: b.id.clone(),
    name: "digest".into(),
    instruction: "summarize".into(),
    trigger: SCHEDULE.into(),
    schedule: "0 8 * * 1-5".into(),
    next_run: Some(100),
    ..Default::default()
  })
  .await
  .unwrap();
  assert_eq!(due(&pool, 99).await.unwrap().len(), 0);
  assert_eq!(due(&pool, 100).await.unwrap().len(), 1);
  set_active(&pool, &r.id, false, None).await.unwrap();
  assert!(due(&pool, 1000).await.unwrap().is_empty());
  assert!(by_key(&pool, &r.id, &r.webhook_key).await.unwrap().is_some());
  assert!(by_key(&pool, &r.id, "nope").await.unwrap().is_none());
}

#[tokio::test]
async fn rejects_unknown_trigger() {
  let pool = crate::memory().await.unwrap();
  let b = bots::create(&pool, &Profile { name: "A".into(), ..Default::default() }).await.unwrap();
  let bad = New { bot: b.id, trigger: "carrier-pigeon".into(), ..Default::default() };
  assert!(create(&pool, bad).await.is_err());
}
