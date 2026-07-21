use super::*;
use std::time::Instant;

fn db() -> Db {
    Db::memory().unwrap()
}

fn project(db: &Db, path: &str) -> i64 {
    db.create_project("Repo", path, "main", 1).unwrap().id
}

/// Sorted ids of one kind in a result set.
fn ids(records: &[SearchRecord], kind: SearchKind) -> Vec<i64> {
    let mut out: Vec<i64> = records
        .iter()
        .filter(|r| r.kind == kind)
        .map(|r| r.id)
        .collect();
    out.sort_unstable();
    out
}

#[test]
fn fts5_compiled_into_bundled_sqlite() {
    // The whole design rests on FTS5 being present in the bundled SQLite. Assert
    // the compile-time flag and prove a virtual table matches end to end, so a
    // future toolchain/feature change that drops FTS5 fails loudly here.
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let flag: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_compile_options \
             WHERE compile_options = 'ENABLE_FTS5'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(flag, 1, "bundled SQLite must be compiled with ENABLE_FTS5");

    conn.execute_batch(
        "CREATE VIRTUAL TABLE probe USING fts5(body);
         INSERT INTO probe(body) VALUES ('the quick brown fox');",
    )
    .unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT count(*) FROM probe WHERE probe MATCH 'brown'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "FTS5 MATCH must work at runtime");
}

#[test]
fn finds_task_prompt_and_run_transcript() {
    let db = db();
    let p = project(&db, "/tmp/s1");
    let t = db
        .create_task(p, "Deploy", "Wire the deployment pipeline", 10)
        .unwrap();
    let r = db
        .create_run(t.id, "claude-code", "/wt/1", "task-deploy")
        .unwrap();
    db.save_run_output(r.id, "compiling the kubernetes manifests")
        .unwrap();

    // A word from the task prompt matches the task.
    let hits = db.search_project(p, "pipeline", 50).unwrap();
    assert_eq!(ids(&hits, SearchKind::Task), vec![t.id]);

    // A word from the saved transcript matches the run.
    let hits = db.search_project(p, "kubernetes", 50).unwrap();
    assert_eq!(ids(&hits, SearchKind::Run), vec![r.id]);

    // A word in neither matches nothing.
    assert!(db.search_project(p, "nonexistent", 50).unwrap().is_empty());
}

#[test]
fn prefix_and_and_semantics() {
    let db = db();
    let p = project(&db, "/tmp/s2");
    let t = db
        .create_task(p, "Deploy", "deployment pipeline rollout", 10)
        .unwrap();

    // Typing part of a word matches via the prefix term.
    assert_eq!(
        ids(
            &db.search_project(p, "deplo", 50).unwrap(),
            SearchKind::Task
        ),
        vec![t.id]
    );
    // Multiple tokens AND together, each a prefix.
    assert_eq!(
        ids(
            &db.search_project(p, "deploy pipe", 50).unwrap(),
            SearchKind::Task
        ),
        vec![t.id]
    );
    // A token present in no row excludes the match (AND, not OR).
    assert!(db
        .search_project(p, "deploy zzzznope", 50)
        .unwrap()
        .is_empty());
}

#[test]
fn triggers_keep_index_in_sync() {
    let db = db();
    let p = project(&db, "/tmp/s3");
    let t = db.create_task(p, "T", "prompt body", 1).unwrap();
    let r = db
        .create_run(t.id, "codex", "/wt/1", "branch-alpha")
        .unwrap();
    db.save_run_output(r.id, "initial gamma output").unwrap();
    assert_eq!(
        ids(&db.search_project(p, "gamma", 50).unwrap(), SearchKind::Run),
        vec![r.id]
    );
    // The branch is indexed too.
    assert_eq!(
        ids(&db.search_project(p, "alpha", 50).unwrap(), SearchKind::Run),
        vec![r.id]
    );

    // Re-snapshotting the transcript replaces the indexed text.
    db.save_run_output(r.id, "revised delta output").unwrap();
    assert!(
        db.search_project(p, "gamma", 50).unwrap().is_empty(),
        "stale transcript term must leave the index"
    );
    assert_eq!(
        ids(&db.search_project(p, "delta", 50).unwrap(), SearchKind::Run),
        vec![r.id]
    );

    // Finishing with a final transcript keeps the index current.
    db.finish_run_with_output(r.id, 0, "epsilon final", 5)
        .unwrap();
    assert_eq!(
        ids(
            &db.search_project(p, "epsilon", 50).unwrap(),
            SearchKind::Run
        ),
        vec![r.id]
    );
    assert!(db.search_project(p, "delta", 50).unwrap().is_empty());

    // Deleting the task cascades to the run; both leave the index.
    db.delete_task(t.id).unwrap();
    assert!(db.search_project(p, "epsilon", 50).unwrap().is_empty());
    assert!(db.search_project(p, "prompt", 50).unwrap().is_empty());
}

#[test]
fn empty_query_lists_recent() {
    let db = db();
    let p = project(&db, "/tmp/s4");
    let a = db.create_task(p, "A", "alpha", 10).unwrap();
    let b = db.create_task(p, "B", "beta", 20).unwrap();
    let hits = db.search_project(p, "", 50).unwrap();
    // Both tasks come back, newest (higher updated_at) first.
    let task_ids = ids(&hits, SearchKind::Task);
    assert!(task_ids.contains(&a.id) && task_ids.contains(&b.id));
    let ordered: Vec<i64> = hits
        .iter()
        .filter(|r| r.kind == SearchKind::Task)
        .map(|r| r.id)
        .collect();
    assert_eq!(ordered, vec![b.id, a.id], "recent list is newest-first");
}

#[test]
fn like_fallback_matches_punctuated_substring() {
    let db = db();
    let p = project(&db, "/tmp/s5");
    let t = db
        .create_task(p, "Refactor", "replace foo::bar with foo::baz", 10)
        .unwrap();

    // `::` is FTS-hostile, so this takes the LIKE path and matches the literal
    // substring - including a mid-token match FTS could not do.
    assert!(fts_query("foo::bar").is_none());
    assert_eq!(
        ids(
            &db.search_project(p, "foo::bar", 50).unwrap(),
            SearchKind::Task
        ),
        vec![t.id]
    );
    // A path-like needle also routes to LIKE and matches literally.
    let r = db.create_run(t.id, "aider", "/wt/1", "b").unwrap();
    db.save_run_output(r.id, "edited src/main.rs and src/lib.rs")
        .unwrap();
    assert_eq!(
        ids(
            &db.search_project(p, "src/main.rs", 50).unwrap(),
            SearchKind::Run
        ),
        vec![r.id]
    );
}

#[test]
fn scoped_to_one_project() {
    let db = db();
    let p1 = project(&db, "/tmp/s6a");
    let p2 = project(&db, "/tmp/s6b");
    let t1 = db.create_task(p1, "T1", "shared keyword here", 1).unwrap();
    let _t2 = db.create_task(p2, "T2", "shared keyword here", 1).unwrap();
    // The same word exists in both projects; a search sees only its project.
    assert_eq!(
        ids(
            &db.search_project(p1, "keyword", 50).unwrap(),
            SearchKind::Task
        ),
        vec![t1.id]
    );
}

#[test]
fn fts_query_builder_rules() {
    // Word queries become AND-ed lowercased prefix terms.
    assert_eq!(fts_query("Deploy Pipe").as_deref(), Some("deploy* pipe*"));
    assert_eq!(
        fts_query("  spaced  out  ").as_deref(),
        Some("spaced* out*")
    );
    // Punctuation routes to the substring fallback.
    assert!(fts_query("foo::bar").is_none());
    assert!(fts_query("a-b").is_none());
    assert!(fts_query("\"quoted\"").is_none());
    // Nothing indexable -> fallback (an empty query is handled earlier).
    assert!(fts_query("").is_none());
}

#[test]
fn input_caps_bound_pathological_queries() {
    let db = db();
    let p = project(&db, "/tmp/s7");
    db.create_task(p, "T", "ordinary prompt", 1).unwrap();

    // A very long single token must not panic and stays a bounded prefix term.
    let long = "a".repeat(5_000);
    let built = fts_query(&long).unwrap();
    assert_eq!(
        built.len(),
        MAX_TERM_CHARS + 1,
        "term capped to MAX_TERM_CHARS + '*'"
    );
    assert!(db.search_project(p, &long, 50).is_ok());

    // Many tokens are capped to MAX_TERMS.
    let many = vec!["tok"; 100].join(" ");
    assert_eq!(fts_query(&many).unwrap().split(' ').count(), MAX_TERMS);

    // The result count is capped no matter what the caller asks for.
    for i in 0..30 {
        db.create_task(p, "bulk", &format!("common token n{i}"), 1)
            .unwrap();
    }
    let hits = db.search_project(p, "common", usize::MAX).unwrap();
    assert!(hits.len() <= MAX_RESULTS);
}

#[test]
fn fts_equivalent_to_like_for_whole_word_at_scale() {
    // On a few-hundred-row corpus, a distinctive whole-word token must return the
    // same rows through the FTS path and the substring path. (Equivalence holds
    // for a token that never appears inside another word, where token-prefix and
    // substring matching coincide.)
    let db = db();
    let p = project(&db, "/tmp/scale-eq");
    let filler = "the quick brown fox jumps over the lazy dog ".repeat(40);
    let mut planted = Vec::new();
    for i in 0..400 {
        let t = db.create_task(p, "task", "filler prompt", 1).unwrap();
        let r = db
            .create_run(t.id, "agent", &format!("/wt/{i}"), &format!("b{i}"))
            .unwrap();
        let body = if i % 97 == 0 {
            planted.push(r.id);
            format!("{filler} zorptok {filler}")
        } else {
            filler.clone()
        };
        db.save_run_output(r.id, &body).unwrap();
    }
    planted.sort_unstable();

    let via_fts = db
        .search_fts(p, &fts_query("zorptok").unwrap(), MAX_RESULTS as i64)
        .unwrap();
    let via_like = db.search_like(p, "zorptok", MAX_RESULTS as i64).unwrap();
    assert_eq!(ids(&via_fts, SearchKind::Run), planted);
    assert_eq!(
        ids(&via_fts, SearchKind::Run),
        ids(&via_like, SearchKind::Run),
        "FTS and LIKE must agree on a whole-word query"
    );
}

#[test]
fn fts_is_not_slower_than_a_full_scan_at_scale() {
    // With a few hundred multi-KB transcripts, the indexed FTS lookup for a rare
    // term should be no slower than the substring full scan - and in practice far
    // faster. We assert only the safe direction (FTS <= scan) to avoid CI flake,
    // and print both for the record.
    let db = db();
    let p = project(&db, "/tmp/scale-speed");
    let filler = "alpha beta gamma delta epsilon zeta eta theta iota kappa ".repeat(60);
    for i in 0..400 {
        let t = db.create_task(p, "task", "filler", 1).unwrap();
        let r = db
            .create_run(t.id, "agent", &format!("/wt/{i}"), &format!("b{i}"))
            .unwrap();
        let body = if i == 200 {
            format!("{filler} raresignal {filler}")
        } else {
            filler.clone()
        };
        db.save_run_output(r.id, &body).unwrap();
    }

    let fts = fts_query("raresignal").unwrap();
    let iterations = 200;

    let start = Instant::now();
    for _ in 0..iterations {
        let hits = db.search_fts(p, &fts, MAX_RESULTS as i64).unwrap();
        assert_eq!(hits.iter().filter(|r| r.kind == SearchKind::Run).count(), 1);
    }
    let fts_elapsed = start.elapsed();

    let start = Instant::now();
    for _ in 0..iterations {
        let hits = db.search_like(p, "raresignal", MAX_RESULTS as i64).unwrap();
        assert_eq!(hits.iter().filter(|r| r.kind == SearchKind::Run).count(), 1);
    }
    let like_elapsed = start.elapsed();

    eprintln!("SEARCH_SPEED rows=400 iters={iterations} fts={fts_elapsed:?} like={like_elapsed:?}");
    assert!(
        fts_elapsed <= like_elapsed,
        "indexed FTS search must not be slower than the full scan (fts={fts_elapsed:?}, like={like_elapsed:?})"
    );
}
