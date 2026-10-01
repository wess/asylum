//! The Marketplace: plugins that can be added and packaged skills. A plugin
//! is an MCP server — remote (HTTPS, signed in with OAuth or a token) or a
//! local command. `fields` are what the user supplies when connecting; the
//! secret ones go to the keychain.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Field {
  pub name: &'static str,
  pub label: &'static str,
  pub secret: bool,
  /// May be left blank.
  pub optional: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Entry {
  pub id: &'static str,
  pub name: &'static str,
  pub category: &'static str,
  pub description: &'static str,
  /// oauth | token | http | command
  pub kind: &'static str,
  pub url: &'static str,
  pub command: &'static str,
  pub args: &'static [&'static str],
  /// For token plugins: the header the token goes in (default Authorization).
  pub header: &'static str,
  pub fields: &'static [Field],
  pub multi_account: bool,
}

const fn e(id: &'static str, name: &'static str, category: &'static str, description: &'static str) -> Entry {
  Entry { id, name, category, description, kind: "oauth", url: "", command: "", args: &[], header: "", fields: &[], multi_account: true }
}

const TOKEN: &[Field] = &[Field { name: "token", label: "Access token", secret: true, optional: false }];

pub const PLUGINS: &[Entry] = &[
  Entry { kind: "token", url: "https://api.githubcopilot.com/mcp/", fields: TOKEN, ..e("github", "GitHub", "Engineering", "Repositories, issues, pull requests, and Actions.") },
  Entry { url: "https://mcp.linear.app/mcp", ..e("linear", "Linear", "Engineering", "Issues, projects, cycles, and comments.") },
  Entry { url: "https://mcp.notion.com/mcp", ..e("notion", "Notion", "Knowledge", "Search, read, and edit pages and databases.") },
  Entry { url: "https://mcp.atlassian.com/v1/mcp", ..e("jira", "Jira & Confluence", "Engineering", "Jira issues and Confluence pages through Atlassian.") },
  Entry {
    kind: "command",
    command: "uvx",
    args: &["mcp-atlassian"],
    fields: &[
      Field { name: "JIRA_URL", label: "Jira site URL, e.g. https://acme.atlassian.net", secret: false, optional: false },
      Field { name: "JIRA_USERNAME", label: "Atlassian account email", secret: false, optional: false },
      Field { name: "JIRA_API_TOKEN", label: "API token", secret: true, optional: false },
      Field { name: "CONFLUENCE_URL", label: "Confluence URL (optional), e.g. https://acme.atlassian.net/wiki", secret: false, optional: true },
      Field { name: "CONFLUENCE_USERNAME", label: "Confluence email (optional)", secret: false, optional: true },
      Field { name: "CONFLUENCE_API_TOKEN", label: "Confluence API token (optional)", secret: true, optional: true },
    ],
    ..e("jiratoken", "Jira & Confluence (API token)", "Engineering", "Your Jira and Confluence with an Atlassian API token, through the open-source mcp-atlassian server. Needs uv.")
  },
  Entry { url: "https://mcp.sentry.dev/mcp", ..e("sentry", "Sentry", "Engineering", "Errors, issues, releases, and traces.") },
  Entry { url: "https://mcp.stripe.com", ..e("stripe", "Stripe", "Finance", "Customers, payments, invoices, and subscriptions.") },
  Entry { url: "https://mcp.supabase.com/mcp", ..e("supabase", "Supabase", "Engineering", "Projects, databases, and edge functions.") },
  Entry { url: "https://mcp.vercel.com", ..e("vercel", "Vercel", "Engineering", "Deployments, projects, and logs.") },
  Entry { url: "https://huggingface.co/mcp", ..e("huggingface", "Hugging Face", "Research", "Models, datasets, Spaces, and papers.") },
  Entry { kind: "http", url: "https://mcp.deepwiki.com/mcp", multi_account: false, ..e("deepwiki", "DeepWiki", "Research", "Ask questions about any public GitHub repository.") },
  Entry { kind: "http", url: "https://mcp.context7.com/mcp", multi_account: false, ..e("context7", "Context7", "Research", "Current documentation for libraries and frameworks.") },
  Entry {
    kind: "command",
    command: "uvx",
    args: &["workspace-mcp", "--single-user"],
    fields: &[
      Field { name: "GOOGLE_OAUTH_CLIENT_ID", label: "Google OAuth client ID", secret: false, optional: false },
      Field { name: "GOOGLE_OAUTH_CLIENT_SECRET", label: "Google OAuth client secret", secret: true, optional: false },
      Field { name: "USER_GOOGLE_EMAIL", label: "Google account email", secret: false, optional: false },
    ],
    multi_account: false,
    ..e("google", "Google Workspace", "Productivity", "Gmail, Calendar, Drive, Docs, Sheets, and Slides.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@modelcontextprotocol/server-slack"],
    fields: &[
      Field { name: "SLACK_BOT_TOKEN", label: "Slack bot token (xoxb-…)", secret: true, optional: false },
      Field { name: "SLACK_TEAM_ID", label: "Workspace ID (T…)", secret: false, optional: false },
    ],
    ..e("slack", "Slack", "Communication", "Read channels and threads, post messages, and react.")
  },
  Entry {
    kind: "command",
    command: "uvx",
    args: &["pagerduty-mcp", "--enable-write-tools"],
    fields: &[Field { name: "PAGERDUTY_USER_API_KEY", label: "PagerDuty API key", secret: true, optional: false }],
    ..e("pagerduty", "PagerDuty", "Engineering", "Incidents, services, schedules, and on-call.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@modelcontextprotocol/server-brave-search"],
    fields: &[Field { name: "BRAVE_API_KEY", label: "Brave Search API key", secret: true, optional: false }],
    multi_account: false,
    ..e("brave", "Brave Search", "Research", "Web and local search with the Brave Search API.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@modelcontextprotocol/server-postgres"],
    fields: &[Field { name: "DATABASE_URL", label: "Postgres connection URL", secret: true, optional: false }],
    ..e("postgres", "Postgres", "Data", "Read-only SQL against a Postgres database.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@playwright/mcp@latest"],
    multi_account: false,
    ..e("playwright", "Playwright", "Engineering", "A second, scriptable browser for testing web apps.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@winor30/mcp-server-datadog"],
    fields: &[
      Field { name: "DATADOG_API_KEY", label: "Datadog API key", secret: true, optional: false },
      Field { name: "DATADOG_APP_KEY", label: "Datadog application key", secret: true, optional: false },
      Field { name: "DATADOG_SITE", label: "Datadog site (e.g. datadoghq.com)", secret: false, optional: false },
    ],
    ..e("datadog", "Datadog", "Engineering", "Monitors, logs, metrics, dashboards, and incidents.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@tsmztech/mcp-server-salesforce"],
    fields: &[
      Field { name: "SALESFORCE_CONNECTION_TYPE", label: "Connection type (User_Password or OAuth_2.0_Client_Credentials)", secret: false, optional: false },
      Field { name: "SALESFORCE_USERNAME", label: "Username (or client ID)", secret: false, optional: false },
      Field { name: "SALESFORCE_PASSWORD", label: "Password (or client secret)", secret: true, optional: false },
      Field { name: "SALESFORCE_TOKEN", label: "Security token", secret: true, optional: false },
      Field { name: "SALESFORCE_INSTANCE_URL", label: "Instance URL", secret: false, optional: false },
    ],
    ..e("salesforce", "Salesforce", "Sales", "Accounts, contacts, opportunities, and SOQL queries.")
  },
  Entry { kind: "http", url: "http://127.0.0.1:3845/mcp", multi_account: false, ..e("figma", "Figma", "Design", "Design context from the Figma desktop app's local server.") },
];

pub fn plugin(id: &str) -> Option<&'static Entry> {
  PLUGINS.iter().find(|p| p.id == id)
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillPack {
  pub name: &'static str,
  pub description: &'static str,
  pub instructions: &'static str,
  /// Marketplace connectors that provide what the skill needs (any one).
  pub needs: &'static [&'static str],
}

/// Either Atlassian connector: the hosted one (sign in) or the API-token one.
pub const JIRA: &[&str] = &["jira", "jiratoken"];

/// How Jira skills name the connector's tools, which differ by server.
macro_rules! JIRA_TOOLS_TEXT { () => { "Tools: use the Jira connector's search (JQL), get, create, edit, transition, and comment tools. With the Atlassian sign-in connector they are named like searchJiraIssuesUsingJql, getJiraIssue, createJiraIssue, editJiraIssue, transitionJiraIssue, and addCommentToJiraIssue (some need the site's cloudId from getAccessibleAtlassianResources). With the API-token connector use jira_search, jira_get_issue, jira_create_issue, jira_update_issue, jira_get_transitions, jira_transition_issue, jira_add_comment, jira_get_agile_boards, jira_get_sprints_from_board, jira_get_sprint_issues, and jira_get_project_versions. If neither is connected, ask the user to add Jira in the Marketplace.\n" } }

pub const SKILLS: &[SkillPack] = &[
  SkillPack {
    name: "Inbox triage",
    description: "Sort new email into act / reply / read / archive and draft replies for approval.",
    needs: &[],
    instructions: "When to use: asked to triage or clean up email.\nInputs: a connected mailbox.\n1. List unread messages since the last triage (or the last 24 hours).\n2. Classify each: needs action, needs reply, FYI, or noise.\n3. For replies, draft with draft_email in the user's voice; never send without approval.\n4. Summarize: counts per bucket, then the action items with sender and deadline.\nValidation: every unread message is in exactly one bucket.\nOutput: a short digest and the drafts.",
  },
  SkillPack {
    name: "Daily digest",
    needs: &[],
    description: "A source-linked summary of what changed across connected apps, with decisions owed.",
    instructions: "When to use: morning digest or on request.\n1. Gather activity since the last digest from connected apps (chat, email, calendar, trackers).\n2. Group by project. Link every item to its source.\n3. Flag decisions the user owes and meetings needing prep.\n4. Keep it under 200 words; put anything else under 'Also'.\nAsk the user to mark items useful or noise and remember their preference.",
  },
  SkillPack {
    name: "Bug reproduction",
    needs: &[],
    description: "Turn a bug report into reliable repro steps with evidence.",
    instructions: "When to use: given a bug report, issue link, or error.\n1. Read the report and any linked logs or traces.\n2. Reproduce in the browser or shell on the computer; record exact steps, inputs, versions.\n3. Capture evidence: screenshots, console errors, failing command output (save files to the workspace).\n4. Narrow to the smallest repro. Note what does NOT reproduce.\nOutput: numbered steps, expected vs actual, evidence file paths, and a likely area of code.",
  },
  SkillPack {
    name: "Expense report",
    needs: &[],
    description: "Collect receipts, categorize spend, and prepare a report for approval.",
    instructions: "When to use: month-end or when receipts arrive.\n1. Find receipts in email and the workspace receipts folder.\n2. Extract merchant, date, amount, currency, category.\n3. Write expenses.csv in the workspace; flag duplicates and missing receipts.\n4. Ask before submitting anything to an expense system.\nOutput: totals by category and the CSV path.",
  },
  SkillPack {
    name: "Meeting prep",
    needs: &[],
    description: "Brief the user before each meeting: who, why, open threads, and suggested agenda.",
    instructions: "When to use: before a calendar event or when asked.\n1. Read the event: attendees, description, linked docs.\n2. Look up each external attendee and recent threads with them.\n3. List open questions and decisions.\nOutput: a one-screen brief with a 3-point agenda.",
  },
  SkillPack {
    name: "Competitive research",
    needs: &[],
    description: "Research competitors and summarize positioning, pricing, and recent moves.",
    instructions: "When to use: asked about competitors or a market.\n1. Identify the competitor set (ask if unclear).\n2. For each: product, pricing, target customer, recent launches (last 90 days), with source links.\n3. Save a comparison table to the workspace as markdown.\nOutput: key takeaways first, then the table path.",
  },
  SkillPack {
    name: "Talent scout",
    needs: &[],
    description: "Source candidates for a role, screen against criteria, and draft outreach for approval.",
    instructions: "When to use: given a role and criteria.\n1. Confirm must-haves and nice-to-haves.\n2. Search sourcing sites in the browser; record each candidate with profile link and why they fit.\n3. Skip anyone already contacted (check memory and the workspace list).\n4. Draft outreach with draft_email; never send without approval.\nOutput: a ranked shortlist saved to candidates.csv.",
  },
  SkillPack {
    name: "Jira triage",
    description: "Triage new Jira issues: fill in type, priority, component, and labels, flag duplicates, and ask reporters what's missing.",
    needs: JIRA,
    instructions: concat!("When to use: asked to triage a Jira project or queue, or on a routine.\nInputs: a project key (ask if unknown) and how far back to look (default: untriaged issues from the last 7 days).\n", JIRA_TOOLS_TEXT!(), "1. Search with JQL, e.g. project = KEY AND status in (\"To Do\", Open, Backlog) AND created >= -7d AND (priority is EMPTY OR component is EMPTY) ORDER BY created DESC.\n2. Read each issue in full. Decide type, priority, component, and labels from the description; check for likely duplicates with a text search on the key terms.\n3. Propose the changes as a table (key, summary, proposed fields, duplicate of, question for the reporter) and ask before applying anything.\n4. On approval, update fields, link duplicates, and comment with any question for the reporter. Never close or delete issues.\nValidation: every issue in the search has a proposal or a stated reason to skip.\nOutput: counts by priority, the table, and links to each issue."),
  },
  SkillPack {
    name: "Jira bug report",
    description: "Turn a bug report, error, or conversation into a well-formed Jira bug, after checking for duplicates.",
    needs: JIRA,
    instructions: concat!("When to use: asked to file a bug, or after reproducing one.\nInputs: the report, the project key, and any evidence in the workspace.\n", JIRA_TOOLS_TEXT!(), "1. Search for an existing issue first (text ~ \"key words\" in the project, last 90 days). If one matches, add the new details as a comment instead and stop.\n2. Check the project's issue types and required fields before creating.\n3. Draft: a summary under 80 characters, steps to reproduce (numbered), expected vs actual, environment and version, frequency, and evidence (attach or link workspace files).\n4. Show the draft and ask before creating it. Never set assignee or sprint unless told.\nOutput: the new issue key and link."),
  },
  SkillPack {
    name: "Sprint report",
    description: "Summarize the active sprint: done, in progress, blocked, scope changes, and risk to the sprint goal.",
    needs: JIRA,
    instructions: concat!("When to use: asked how a sprint is going, or for a sprint review.\nInputs: a board or project (ask if unknown).\n", JIRA_TOOLS_TEXT!(), "1. Find the active sprint (or the one named) and its issues; without board tools, search sprint in openSprints() AND project = KEY.\n2. Group by status category: done, in progress, to do. Sum story points where the field exists.\n3. Flag blocked or flagged issues, anything unchanged for 3+ days, and issues added after the sprint started.\n4. Compare remaining work to days left and say plainly whether the goal is at risk.\nOutput: a short status line, then sections Done, In progress, Blocked, Added mid-sprint, Risks, each item linked."),
  },
  SkillPack {
    name: "Jira standup",
    description: "A per-person standup from Jira: what moved yesterday, what's in progress, and what's blocked.",
    needs: JIRA,
    instructions: concat!("When to use: before a standup, or on a weekday morning routine.\nInputs: a project or team (ask if unknown).\n", JIRA_TOOLS_TEXT!(), "1. Search issues updated in the last working day: project = KEY AND updated >= -1d (on Mondays, -3d).\n2. For each assignee: issues moved to done, issues moved into progress, new comments they wrote, and anything blocked or flagged.\n3. Keep each person to three lines. Link every issue.\nOutput: the standup grouped by person, then a Blockers section at the top if there are any."),
  },
  SkillPack {
    name: "Jira release notes",
    description: "Write release notes from the issues in a Jira fix version, grouped and in plain language.",
    needs: JIRA,
    instructions: concat!("When to use: preparing a release.\nInputs: the project key and fix version (list the project's versions and ask if unknown).\n", JIRA_TOOLS_TEXT!(), "1. Search fixVersion = \"VERSION\" AND project = KEY.\n2. Group into New, Improved, Fixed, and Internal (skip Internal unless asked). Rewrite each summary for users, not engineers.\n3. Note any issue in the version that isn't done yet.\n4. Save release-notes-VERSION.md to the workspace and show it.\nOutput: the notes and the list of unfinished issues."),
  },
  SkillPack {
    name: "Backlog grooming",
    description: "Find stale, duplicate, and incomplete backlog issues and propose what to close, merge, or clarify.",
    needs: JIRA,
    instructions: concat!("When to use: asked to clean up a backlog.\nInputs: the project key.\n", JIRA_TOOLS_TEXT!(), "1. Search the backlog: project = KEY AND statusCategory = \"To Do\" ORDER BY updated ASC.\n2. Flag: no update in 90+ days, missing description or acceptance criteria, likely duplicates (similar summaries), and epics with no open children.\n3. Propose one action per issue: close as stale, merge into KEY-123, ask for detail, or keep. Ask before changing anything; prefer comments over closing.\nOutput: a table of proposals and a count per action."),
  },
  SkillPack {
    name: "Action items to Jira",
    description: "Turn meeting notes or a thread into Jira issues with owners and due dates, after you review them.",
    needs: JIRA,
    instructions: concat!("When to use: given meeting notes, a transcript, or a chat thread with decisions.\nInputs: the notes and a project key.\n", JIRA_TOOLS_TEXT!(), "1. Extract each action item: what, who, by when. Skip decisions without an action.\n2. Look up each owner's Jira account; if a name doesn't match exactly one person, leave it unassigned and say so.\n3. Check for existing issues covering the same work.\n4. Show the proposed issues and ask before creating them.\nOutput: the created issue keys, linked, with owners and due dates."),
  },
];

#[cfg(test)]
#[path = "../tests/catalog.rs"]
mod tests;
