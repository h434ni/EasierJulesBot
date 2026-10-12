use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepo {
    #[serde(default)]
    pub branches: Vec<Branch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub name: Option<String>,
    pub id: Option<String>,
    pub github_repo: Option<GithubRepo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSourcesResponse {
    #[serde(default)]
    pub sources: Vec<Source>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestOutput {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOutput {
    pub pull_request: Option<PullRequestOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub name: String,
    pub url: Option<String>,
    pub title: Option<String>,
    pub state: Option<String>,
    pub automation_mode: Option<String>,
    #[serde(default)]
    pub outputs: Vec<SessionOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsResponse {
    #[serde(default)]
    pub sessions: Vec<Session>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFailed {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessaged {
    pub agent_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    #[serde(default)]
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanGenerated {
    pub plan: Option<Plan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdated {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPatch {
    pub unidiff_patch: Option<String>,
    pub suggested_commit_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    pub git_patch: Option<GitPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BashOutput {
    pub command: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub change_set: Option<ChangeSet>,
    pub bash_output: Option<BashOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub name: Option<String>,
    pub create_time: Option<String>,
    pub description: Option<String>,
    pub session_completed: Option<serde_json::Value>,
    pub session_failed: Option<SessionFailed>,
    pub agent_messaged: Option<AgentMessaged>,
    pub plan_generated: Option<PlanGenerated>,
    pub plan_approved: Option<serde_json::Value>,
    pub progress_updated: Option<ProgressUpdated>,
    pub user_messaged: Option<serde_json::Value>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListActivitiesResponse {
    #[serde(default)]
    pub activities: Vec<Activity>,
    pub next_page_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_with_pr() {
        let json = r#"{
            "name": "sessions/test-123",
            "url": "https://jules.google.com/session/test-123",
            "title": "Fix bug in auth",
            "state": "IN_PROGRESS",
            "automationMode": "AUTO_CREATE_PR",
            "outputs": [
                {
                    "pullRequest": {
                        "url": "https://github.com/org/repo/pull/42"
                    }
                }
            ]
        }"#;

        let session: Session = serde_json::from_str(json).unwrap();
        assert_eq!(session.name, "sessions/test-123");
        assert_eq!(session.state.as_deref(), Some("IN_PROGRESS"));
        assert_eq!(session.automation_mode.as_deref(), Some("AUTO_CREATE_PR"));
        assert_eq!(session.outputs.len(), 1);
        let pr = session.outputs[0].pull_request.as_ref().unwrap();
        assert_eq!(pr.url.as_deref(), Some("https://github.com/org/repo/pull/42"));
    }

    #[test]
    fn test_parse_activity_with_patch_and_bash() {
        let json = r#"{
            "name": "activities/act-1",
            "createTime": "2026-10-12T09:00:00Z",
            "description": "Agent wrote new tests",
            "agentMessaged": {
                "agentMessage": "I added 3 unit tests"
            },
            "artifacts": [
                {
                    "changeSet": {
                        "gitPatch": {
                            "unidiffPatch": "--- a/test.rs\n+++ b/test.rs\n@@ -1 +1 @@\n+assert!(true);",
                            "suggestedCommitMessage": "Add unit tests"
                        }
                    }
                },
                {
                    "bashOutput": {
                        "command": "cargo test",
                        "exitCode": 0
                    }
                }
            ]
        }"#;

        let act: Activity = serde_json::from_str(json).unwrap();
        assert_eq!(act.create_time.as_deref(), Some("2026-10-12T09:00:00Z"));
        assert_eq!(
            act.agent_messaged.unwrap().agent_message.as_deref(),
            Some("I added 3 unit tests")
        );
        assert_eq!(act.artifacts.len(), 2);
        let patch = act.artifacts[0]
            .change_set
            .as_ref()
            .unwrap()
            .git_patch
            .as_ref()
            .unwrap();
        assert_eq!(patch.suggested_commit_message.as_deref(), Some("Add unit tests"));
        let bash = act.artifacts[1].bash_output.as_ref().unwrap();
        assert_eq!(bash.command.as_deref(), Some("cargo test"));
        assert_eq!(bash.exit_code, Some(0));
    }
}

