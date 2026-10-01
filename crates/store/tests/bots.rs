use super::*;

fn profile(name: &str) -> Profile {
  Profile {
    name: name.into(),
    label: "Talent Scout".into(),
    ..Default::default()
  }
}

#[tokio::test]
async fn create_pin_hide_order() {
  let pool = crate::memory().await.unwrap();
  let a = create(&pool, &profile("Ada")).await.unwrap();
  let b = create(&pool, &profile("Bo")).await.unwrap();
  pin(&pool, &b.id, true).await.unwrap();
  hide(&pool, &a.id, true).await.unwrap();
  let all = list(&pool).await.unwrap();
  assert_eq!(all[0].id, b.id);
  assert!(all[1].hidden);
}

#[tokio::test]
async fn one_primary() {
  let pool = crate::memory().await.unwrap();
  let a = create(&pool, &profile("A")).await.unwrap();
  let b = create(&pool, &profile("B")).await.unwrap();
  set_primary(&pool, &a.id).await.unwrap();
  set_primary(&pool, &b.id).await.unwrap();
  assert_eq!(primary(&pool).await.unwrap().unwrap().id, b.id);
  assert!(!get(&pool, &a.id).await.unwrap().primary_bot);
}

#[tokio::test]
async fn find_by_mention() {
  let pool = crate::memory().await.unwrap();
  create(&pool, &profile("Scout")).await.unwrap();
  assert!(find(&pool, "@scout").await.unwrap().is_some());
}

#[test]
fn next_name_counts_up() {
  let taken = vec!["scout".to_string(), "scout 2".to_string()];
  assert_eq!(next_name("Scout", &taken), "Scout 3");
  assert_eq!(next_name("New", &taken), "New");
}
