//! GraphQL query/mutation documents used by the [`Client`](crate::Client).

/// Teams visible to the token.
pub const TEAMS: &str = "query { teams { nodes { id key name } } }";

/// Recent issues with their workflow state.
pub const ISSUES: &str =
    "query { issues(first: 50) { nodes { id identifier title url priority state { name } } } }";

/// Projects with their state.
pub const PROJECTS: &str = "query { projects(first: 50) { nodes { id name state } } }";

/// Create an issue (`$input: IssueCreateInput!`).
pub const CREATE_ISSUE: &str = "mutation($input: IssueCreateInput!) { issueCreate(input: $input) { success issue { id identifier url } } }";
