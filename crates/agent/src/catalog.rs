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

const TOKEN: &[Field] = &[Field { name: "token", label: "Access token", secret: true }];

pub const PLUGINS: &[Entry] = &[
  Entry { kind: "token", url: "https://api.githubcopilot.com/mcp/", fields: TOKEN, ..e("github", "GitHub", "Engineering", "Repositories, issues, pull requests, and Actions.") },
  Entry { url: "https://mcp.linear.app/mcp", ..e("linear", "Linear", "Engineering", "Issues, projects, cycles, and comments.") },
  Entry { url: "https://mcp.notion.com/mcp", ..e("notion", "Notion", "Knowledge", "Search, read, and edit pages and databases.") },
  Entry { url: "https://mcp.atlassian.com/v1/mcp", ..e("jira", "Jira & Confluence", "Engineering", "Jira issues and Confluence pages through Atlassian.") },
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
      Field { name: "GOOGLE_OAUTH_CLIENT_ID", label: "Google OAuth client ID", secret: false },
      Field { name: "GOOGLE_OAUTH_CLIENT_SECRET", label: "Google OAuth client secret", secret: true },
      Field { name: "USER_GOOGLE_EMAIL", label: "Google account email", secret: false },
    ],
    multi_account: false,
    ..e("google", "Google Workspace", "Productivity", "Gmail, Calendar, Drive, Docs, Sheets, and Slides.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@modelcontextprotocol/server-slack"],
    fields: &[
      Field { name: "SLACK_BOT_TOKEN", label: "Slack bot token (xoxb-…)", secret: true },
      Field { name: "SLACK_TEAM_ID", label: "Workspace ID (T…)", secret: false },
    ],
    ..e("slack", "Slack", "Communication", "Read channels and threads, post messages, and react.")
  },
  Entry {
    kind: "command",
    command: "uvx",
    args: &["pagerduty-mcp", "--enable-write-tools"],
    fields: &[Field { name: "PAGERDUTY_USER_API_KEY", label: "PagerDuty API key", secret: true }],
    ..e("pagerduty", "PagerDuty", "Engineering", "Incidents, services, schedules, and on-call.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@modelcontextprotocol/server-brave-search"],
    fields: &[Field { name: "BRAVE_API_KEY", label: "Brave Search API key", secret: true }],
    multi_account: false,
    ..e("brave", "Brave Search", "Research", "Web and local search with the Brave Search API.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@modelcontextprotocol/server-postgres"],
    fields: &[Field { name: "DATABASE_URL", label: "Postgres connection URL", secret: true }],
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
      Field { name: "DATADOG_API_KEY", label: "Datadog API key", secret: true },
      Field { name: "DATADOG_APP_KEY", label: "Datadog application key", secret: true },
      Field { name: "DATADOG_SITE", label: "Datadog site (e.g. datadoghq.com)", secret: false },
    ],
    ..e("datadog", "Datadog", "Engineering", "Monitors, logs, metrics, dashboards, and incidents.")
  },
  Entry {
    kind: "command",
    command: "npx",
    args: &["-y", "@tsmztech/mcp-server-salesforce"],
    fields: &[
      Field { name: "SALESFORCE_CONNECTION_TYPE", label: "Connection type (User_Password or OAuth_2.0_Client_Credentials)", secret: false },
      Field { name: "SALESFORCE_USERNAME", label: "Username (or client ID)", secret: false },
      Field { name: "SALESFORCE_PASSWORD", label: "Password (or client secret)", secret: true },
      Field { name: "SALESFORCE_TOKEN", label: "Security token", secret: true },
      Field { name: "SALESFORCE_INSTANCE_URL", label: "Instance URL", secret: false },
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
}

pub const SKILLS: &[SkillPack] = &[
  SkillPack {
    name: "Inbox triage",
    description: "Sort new email into act / reply / read / archive and draft replies for approval.",
    instructions: "When to use: asked to triage or clean up email.\nInputs: a connected mailbox.\n1. List unread messages since the last triage (or the last 24 hours).\n2. Classify each: needs action, needs reply, FYI, or noise.\n3. For replies, draft with draft_email in the user's voice; never send without approval.\n4. Summarize: counts per bucket, then the action items with sender and deadline.\nValidation: every unread message is in exactly one bucket.\nOutput: a short digest and the drafts.",
  },
  SkillPack {
    name: "Daily digest",
    description: "A source-linked summary of what changed across connected apps, with decisions owed.",
    instructions: "When to use: morning digest or on request.\n1. Gather activity since the last digest from connected apps (chat, email, calendar, trackers).\n2. Group by project. Link every item to its source.\n3. Flag decisions the user owes and meetings needing prep.\n4. Keep it under 200 words; put anything else under 'Also'.\nAsk the user to mark items useful or noise and remember their preference.",
  },
  SkillPack {
    name: "Bug reproduction",
    description: "Turn a bug report into reliable repro steps with evidence.",
    instructions: "When to use: given a bug report, issue link, or error.\n1. Read the report and any linked logs or traces.\n2. Reproduce in the browser or shell on the computer; record exact steps, inputs, versions.\n3. Capture evidence: screenshots, console errors, failing command output (save files to the workspace).\n4. Narrow to the smallest repro. Note what does NOT reproduce.\nOutput: numbered steps, expected vs actual, evidence file paths, and a likely area of code.",
  },
  SkillPack {
    name: "Expense report",
    description: "Collect receipts, categorize spend, and prepare a report for approval.",
    instructions: "When to use: month-end or when receipts arrive.\n1. Find receipts in email and the workspace receipts folder.\n2. Extract merchant, date, amount, currency, category.\n3. Write expenses.csv in the workspace; flag duplicates and missing receipts.\n4. Ask before submitting anything to an expense system.\nOutput: totals by category and the CSV path.",
  },
  SkillPack {
    name: "Meeting prep",
    description: "Brief the user before each meeting: who, why, open threads, and suggested agenda.",
    instructions: "When to use: before a calendar event or when asked.\n1. Read the event: attendees, description, linked docs.\n2. Look up each external attendee and recent threads with them.\n3. List open questions and decisions.\nOutput: a one-screen brief with a 3-point agenda.",
  },
  SkillPack {
    name: "Competitive research",
    description: "Research competitors and summarize positioning, pricing, and recent moves.",
    instructions: "When to use: asked about competitors or a market.\n1. Identify the competitor set (ask if unclear).\n2. For each: product, pricing, target customer, recent launches (last 90 days), with source links.\n3. Save a comparison table to the workspace as markdown.\nOutput: key takeaways first, then the table path.",
  },
  SkillPack {
    name: "Talent scout",
    description: "Source candidates for a role, screen against criteria, and draft outreach for approval.",
    instructions: "When to use: given a role and criteria.\n1. Confirm must-haves and nice-to-haves.\n2. Search sourcing sites in the browser; record each candidate with profile link and why they fit.\n3. Skip anyone already contacted (check memory and the workspace list).\n4. Draft outreach with draft_email; never send without approval.\nOutput: a ranked shortlist saved to candidates.csv.",
  },
];
