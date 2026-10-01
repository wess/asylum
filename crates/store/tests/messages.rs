use super::*;
use crate::bots::{self, Profile};

async fn setup() -> (Pool, String) {
  let pool = crate::memory().await.unwrap();
  let b = bots::create(&pool, &Profile { name: "A".into(), ..Default::default() }).await.unwrap();
  let c = chats::direct(&pool, &b.id).await.unwrap();
  (pool, c.id)
}

fn new<'a>(chat: &'a str, body: &'a str) -> New<'a> {
  New { chat, bot: None, role: USER, body, parts: &[], status: DONE, run: None, thread: None }
}

#[tokio::test]
async fn search_finds_updated_body() {
  let (pool, chat) = setup().await;
  let m = add(&pool, new(&chat, "draft")).await.unwrap();
  update(&pool, &m.id, "pipeline review list", &[], DONE).await.unwrap();
  let hits = search(&pool, "pipel", 10).await.unwrap();
  assert_eq!(hits.len(), 1);
  assert!(search(&pool, "draft", 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn truncate_drops_tail() {
  let (pool, chat) = setup().await;
  add(&pool, new(&chat, "one")).await.unwrap();
  let two = add(&pool, new(&chat, "two")).await.unwrap();
  add(&pool, new(&chat, "three")).await.unwrap();
  truncate_from(&pool, &chat, &two.id).await.unwrap();
  let left: Vec<_> = list(&pool, &chat).await.unwrap().into_iter().map(|m| m.body).collect();
  assert_eq!(left, vec!["one"]);
}

#[test]
fn fts_query_is_quoted() {
  assert_eq!(fts_query("a \"b OR"), "\"a\"* \"b\"* \"OR\"*");
}

#[tokio::test]
async fn threads_and_reactions() {
  let (pool, chat) = setup().await;
  let root = add(&pool, new(&chat, "root")).await.unwrap();
  let mut reply = new(&chat, "reply");
  reply.thread = Some(&root.id);
  add(&pool, reply).await.unwrap();
  assert_eq!(list(&pool, &chat).await.unwrap().len(), 1);
  assert_eq!(thread(&pool, &root.id).await.unwrap()[0].body, "reply");
  assert_eq!(thread_counts(&pool, &chat).await.unwrap(), vec![(root.id.clone(), 1)]);
  react(&pool, &root.id, "👍").await.unwrap();
  assert_eq!(get(&pool, &root.id).await.unwrap().reactions().get("👍"), Some(&1));
  react(&pool, &root.id, "👍").await.unwrap();
  assert!(get(&pool, &root.id).await.unwrap().reactions().is_empty());
}
