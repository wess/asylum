use crate::Template;

pub fn template(kind: Template, title: &str, created: i64) -> String {
    let kind_name = match kind {
        Template::Blank => "note",
        Template::Task => "task",
        Template::Decision => "decision",
        Template::Investigation => "investigation",
        Template::Retrospective => "retrospective",
    };
    let body = match kind {
        Template::Blank => "",
        Template::Task => {
            "## Outcome\n\n## Context\n\n## Constraints\n\n## Acceptance criteria\n\n- [ ] "
        }
        Template::Decision => {
            "## Decision\n\n## Context\n\n## Options considered\n\n## Consequences\n"
        }
        Template::Investigation => "## Question\n\n## Evidence\n\n## Findings\n\n## Next step\n",
        Template::Retrospective => {
            "## Outcome\n\n## What worked\n\n## What did not\n\n## Follow-ups\n\n- [ ] "
        }
    };
    format!(
        "---\ntitle: {title}\ntype: {kind_name}\ncreated: {created}\ntags:\n  - {kind_name}\n---\n\n# {title}\n\n{body}"
    )
}
