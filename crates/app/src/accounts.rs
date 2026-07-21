//! The Accounts surface: provider accounts with usage bars and the active
//! account marked. Explicit actions switch or delete an account.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use gpui::prelude::*;
use gpui::{div, px, App, Context, Entity, Hsla, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::{empty, Button};
use crate::state::{AccountRow, Root};

/// A sign-in probe's state for one account. Kept module-side because a probe is
/// an on-demand background action and the Accounts view is a pure render with no
/// entity of its own; the `Root` entity (state.rs) is intentionally not touched.
/// Keyed by account id.
#[derive(Clone)]
enum ProbeState {
    Running,
    Done(agent::probe::Auth),
}

fn probe_cache() -> &'static Mutex<HashMap<i64, ProbeState>> {
    static CACHE: OnceLock<Mutex<HashMap<i64, ProbeState>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The cached probe state for an account, if any check has been run.
fn probe_state(id: i64) -> Option<ProbeState> {
    probe_cache().lock().ok()?.get(&id).cloned()
}

/// Kick off a background sign-in probe for one account, mirroring how the
/// Settings surface backgrounds the agent CLI probes: mark it running, run the
/// blocking check on the background executor, store the verdict, and notify so
/// the card redraws. Never runs on the render path - only the re-check button
/// calls this.
fn recheck(id: i64, provider: String, cx: &mut Context<Root>) {
    if let Ok(mut cache) = probe_cache().lock() {
        cache.insert(id, ProbeState::Running);
    }
    cx.notify();
    let executor = cx.background_executor().clone();
    cx.spawn(async move |handle, cx| {
        let auth = executor
            .spawn(async move { agent::probe::check(&provider) })
            .await;
        if let Ok(mut cache) = probe_cache().lock() {
            cache.insert(id, ProbeState::Done(auth));
        }
        handle.update(cx, |_, cx| cx.notify()).ok();
    })
    .detach();
}

pub fn accounts_view(
    rows: Vec<AccountRow>,
    input: Entity<guise::TextInput>,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = guise::theme::theme(cx);
    let track = theme.surface_hover().hsla();
    let fill = theme.primary().hsla();
    let mut col = div()
        .id("accounts-scroll")
        .flex()
        .flex_col()
        .size_full()
        .gap_4()
        .p(px(20.0))
        .overflow_y_scroll();
    col = col.child(Title::new("Accounts").order(2));
    col = col.child(
        Text::new("Accounts are local labels for provider identities and their usage history. The active account marks your preferred identity in Asylum; it does not sign in or change the provider CLI session. Authentication stays with each provider’s CLI.")
            .size(Size::Sm)
            .dimmed(),
    );

    // Add-account form: provider and optional label, Enter or the button adds.
    let add = handle.clone();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().flex_1().min_w_0().child(input))
            .child(
                Button::new("add-account", "Add account")
                    .size(Size::Sm)
                    .variant(Variant::Filled)
                    .on_click(move |_, _, cx| {
                        add.update(cx, |root, cx| root.add_account_from_input(cx));
                    }),
            ),
    );

    if rows.is_empty() {
        return col.child(empty(
            "Connect your first account",
            "Enter a provider and optional label above to organize work and personal identities and keep their usage records separate.",
        ));
    }

    for row in rows {
        col = col.child(account_card(row, track, fill, handle.clone()));
    }
    col
}

fn account_card(
    row: AccountRow,
    track: Hsla,
    fill: Hsla,
    handle: Entity<Root>,
) -> impl IntoElement {
    let id = row.account.id;
    let active = row.account.active;
    let provider = row.account.provider.clone();

    // Sign-in probe: what we can offer for this provider, and the last result.
    let kind = agent::probe::kind(&provider);
    let state = probe_state(id);
    let checking = matches!(state, Some(ProbeState::Running));
    let show_recheck = matches!(kind, agent::probe::Kind::Probeable);
    let pill = signin_pill(id, &kind, &state);

    let mut card = div().flex().flex_col().gap_2().child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Text::new(SharedString::from(provider.clone())).bold())
                    .child(
                        Text::new(SharedString::from(row.account.label.clone()))
                            .size(Size::Sm)
                            .dimmed(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .children(pill)
                    .child(if active {
                        Badge::new("active")
                            .color(ColorName::Green)
                            .variant(Variant::Light)
                    } else {
                        Badge::new("available")
                            .color(ColorName::Gray)
                            .variant(Variant::Light)
                    }),
            ),
    );

    if let Some(usage) = &row.usage {
        let pct = usage.fraction().map(|f| (f * 100.0) as u32).unwrap_or(0);
        let limit = usage
            .limit
            .map(|l| l.to_string())
            .unwrap_or_else(|| "∞".into());
        card = card.child(
            Text::new(SharedString::from(format!(
                "Usage: {} / {} ({pct}%)",
                usage.used, limit
            )))
            .size(Size::Xs)
            .dimmed(),
        );
        card = card.child(usage_bar(pct, track, fill));
    }

    let activate = handle.clone();
    let recheck_handle = handle.clone();
    let remove = handle;
    card = card.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .children((!active).then(|| {
                Button::new(
                    SharedString::from(format!("activate-account-{id}")),
                    "Use account",
                )
                .size(Size::Xs)
                .variant(Variant::Filled)
                .on_click(move |_, _, cx| {
                    activate.update(cx, |root, cx| {
                        if let Err(error) = root.db.activate_account(id) {
                            root.push_error("Could not switch account", error.to_string());
                        }
                        cx.notify();
                    });
                })
            }))
            .child(
                Button::new(SharedString::from(format!("delete-account-{id}")), "Delete")
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .on_click(move |_, _, cx| {
                        remove.update(cx, |root, cx| {
                            root.confirm = Some(crate::run::ConfirmAction::DeleteAccount(id));
                            cx.notify();
                        });
                    }),
            )
            .children(show_recheck.then(move || {
                Button::new(
                    SharedString::from(format!("recheck-account-{id}")),
                    if checking {
                        "Checking…"
                    } else {
                        "Check sign-in"
                    },
                )
                .size(Size::Xs)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    let provider = provider.clone();
                    recheck_handle.update(cx, |_, cx| recheck(id, provider, cx));
                })
            })),
    );
    Card::new().padding(Size::Md).child(card)
}

/// The sign-in pill for a card header. `None` for providers we have no probe
/// for, so no sign-in claim is made. For probeable providers it reflects the
/// cached probe state (never checked / checking / a verdict); for providers with
/// no safe check it shows a fixed Unknown carrying the reason as a tooltip.
fn signin_pill(
    id: i64,
    kind: &agent::probe::Kind,
    state: &Option<ProbeState>,
) -> Option<impl IntoElement> {
    let (label, color, reason): (&'static str, ColorName, Option<String>) = match kind {
        agent::probe::Kind::Absent => return None,
        agent::probe::Kind::Static(auth) => auth_pill(auth),
        agent::probe::Kind::Probeable => match state {
            None => ("Not checked", ColorName::Gray, None),
            Some(ProbeState::Running) => ("Checking…", ColorName::Blue, None),
            Some(ProbeState::Done(auth)) => auth_pill(auth),
        },
    };
    let badge = Badge::new(label).color(color).variant(Variant::Light);
    let mut wrap = div().id(SharedString::from(format!("signin-{id}")));
    if let Some(reason) = reason {
        wrap = wrap.tooltip(guise::tooltip(reason));
    }
    Some(wrap.child(badge))
}

/// Map a decided sign-in verdict to a pill (label, colour, optional tooltip).
fn auth_pill(auth: &agent::probe::Auth) -> (&'static str, ColorName, Option<String>) {
    match auth {
        agent::probe::Auth::SignedIn => ("Signed in", ColorName::Green, None),
        agent::probe::Auth::SignedOut => ("Signed out", ColorName::Red, None),
        agent::probe::Auth::Unknown(reason) => ("Unknown", ColorName::Yellow, Some(reason.clone())),
    }
}

/// A slim usage meter, 0..100.
fn usage_bar(pct: u32, track: Hsla, fill: Hsla) -> impl IntoElement {
    let filled = (pct.min(100)) as f32 / 100.0;
    div().w_full().h(px(6.0)).rounded(px(3.0)).bg(track).child(
        div()
            .h_full()
            .rounded(px(3.0))
            .bg(fill)
            .w(gpui::relative(filled)),
    )
}
