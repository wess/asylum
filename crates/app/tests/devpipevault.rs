use super::*;

// The pure helpers. Signing in and loading touch the network and the keep, so
// they are not exercised here.

fn entry(scope: devpipe::Scope, scope_id: i64, name: &str, kind: devpipe::Kind) -> devpipe::Entry {
    devpipe::Entry {
        scope,
        scope_id,
        name: name.to_string(),
        kind,
        updated_at: String::new(),
        last_used_at: None,
    }
}

fn grant(box_id: i64, scope: devpipe::Scope, scope_id: i64, name: &str) -> devpipe::Grant {
    devpipe::Grant {
        box_id,
        scope,
        scope_id,
        name: name.to_string(),
    }
}

#[test]
fn a_key_identifies_one_entry_across_scopes() {
    // Two entries can share a name in different scopes — that is what shadowing
    // is — so the key has to carry the scope or the wrong row gets revealed.
    let a = entry(devpipe::Scope::Global, 0, "KEY", devpipe::Kind::Secret);
    let b = entry(devpipe::Scope::Box, 7, "KEY", devpipe::Kind::Secret);
    assert_ne!(key_of(&a), key_of(&b));
    assert_eq!(key_of(&a), "global:0:KEY");
    assert_eq!(key_of(&b), "box:7:KEY");
}

#[test]
fn grants_are_matched_on_scope_not_just_name() {
    // A grant for the global KEY must not appear to cover a box-scoped KEY,
    // which is a different entry that shadows it.
    let global = entry(devpipe::Scope::Global, 0, "KEY", devpipe::Kind::Secret);
    let boxed = entry(devpipe::Scope::Box, 7, "KEY", devpipe::Kind::Secret);
    let grants = vec![grant(3, devpipe::Scope::Global, 0, "KEY")];

    assert_eq!(granted_boxes(&grants, &global).len(), 1);
    assert_eq!(granted_boxes(&grants, &boxed).len(), 0);
}

#[test]
fn an_ungranted_secret_says_so_plainly() {
    let secret = entry(devpipe::Scope::Global, 0, "KEY", devpipe::Kind::Secret);
    assert_eq!(readable_by(&[], &secret), "No box can read this");

    let grants = vec![grant(3, devpipe::Scope::Global, 0, "KEY")];
    assert!(readable_by(&grants, &secret).contains("1 box"));
}

#[test]
fn a_value_is_never_described_as_ungranted() {
    // Values were never withheld; saying "no box can read this" about one would
    // teach a model that is not true.
    let value = entry(devpipe::Scope::Global, 0, "REGION", devpipe::Kind::Value);
    assert_eq!(readable_by(&[], &value), "Readable by your boxes");
}

#[test]
fn the_base_url_is_overridable_but_defaults_to_devpipe() {
    // Read from the environment so a self-hosted control plane works without a
    // rebuild. The default is the hosted one.
    let previous = std::env::var("DEVPIPE_URL").ok();
    unsafe { std::env::remove_var("DEVPIPE_URL") };
    assert_eq!(base(), "https://devpipe.com/api");
    unsafe { std::env::set_var("DEVPIPE_URL", "http://127.0.0.1:3000/api") };
    assert_eq!(base(), "http://127.0.0.1:3000/api");
    // An empty value is not an endpoint; fall back rather than call "".
    unsafe { std::env::set_var("DEVPIPE_URL", "") };
    assert_eq!(base(), "https://devpipe.com/api");
    match previous {
        Some(v) => unsafe { std::env::set_var("DEVPIPE_URL", v) },
        None => unsafe { std::env::remove_var("DEVPIPE_URL") },
    }
}
