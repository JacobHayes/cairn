//! The shipped agent instructions (I4; ARCHITECTURE, MCP endpoint): `instructions/` is a
//! skill, `SKILL.md` and one file per workflow, compiled in so they version with the tools.
//! The MCP server sends the guide as its initialize instructions and serves each workflow
//! as a prompt, so a connected agent has them with no setup; the same files are published
//! as the skill.

/// One workflow: its prompt name, what it is for, and its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Workflow {
    /// The prompt's name: the file's name without `.md`.
    pub name: &'static str,
    /// What it is for, from its front matter.
    pub description: &'static str,
    /// The workflow, without its front matter.
    pub text: &'static str,
}

/// `SKILL.md` as published: front matter and body.
pub const SKILL: &str = include_str!("../../../instructions/SKILL.md");

/// The workflow files, by name.
const WORKFLOW_FILES: [(&str, &str); 8] = [
    (
        "author-route",
        include_str!("../../../instructions/workflows/author-route.md"),
    ),
    (
        "structure-journey",
        include_str!("../../../instructions/workflows/structure-journey.md"),
    ),
    (
        "walk-decisions",
        include_str!("../../../instructions/workflows/walk-decisions.md"),
    ),
    (
        "record-answers",
        include_str!("../../../instructions/workflows/record-answers.md"),
    ),
    (
        "propose-breakdown",
        include_str!("../../../instructions/workflows/propose-breakdown.md"),
    ),
    (
        "summarize-frontier",
        include_str!("../../../instructions/workflows/summarize-frontier.md"),
    ),
    (
        "resolve-date-conflict",
        include_str!("../../../instructions/workflows/resolve-date-conflict.md"),
    ),
    (
        "handle-stale-proposal",
        include_str!("../../../instructions/workflows/handle-stale-proposal.md"),
    ),
];

/// The guide an agent reads when it connects: `SKILL.md` without its front matter.
#[must_use]
pub fn guide() -> &'static str {
    split(SKILL).1
}

/// Every workflow, in the order the guide lists them.
#[must_use]
pub fn workflows() -> Vec<Workflow> {
    WORKFLOW_FILES
        .iter()
        .map(|(name, file)| {
            let (front, text) = split(file);
            Workflow {
                name,
                description: field(front, "description").unwrap_or_default(),
                text,
            }
        })
        .collect()
}

/// A file's front matter (between its opening and closing `---` lines) and its body; no
/// front matter when it does not open with one.
fn split(file: &str) -> (&str, &str) {
    let Some(rest) = file.strip_prefix("---\n") else {
        return ("", file);
    };
    match rest.split_once("\n---\n") {
        Some((front, body)) => (front, body.trim_start()),
        None => ("", file),
    }
}

/// The value of a one-line `key: value` field in front matter.
fn field<'a>(front: &'a str, key: &str) -> Option<&'a str> {
    front.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_has_a_name_and_a_description() {
        let (front, body) = split(SKILL);
        assert_eq!(field(front, "name"), Some("cairn"));
        assert!(field(front, "description").is_some_and(|text| !text.is_empty()));
        assert_eq!(guide(), body);
        assert!(!guide().starts_with("---"));
    }

    #[test]
    fn every_workflow_has_a_description_and_is_linked_from_the_guide() {
        for workflow in workflows() {
            assert!(!workflow.description.is_empty(), "{}", workflow.name);
            assert!(workflow.text.starts_with("# "), "{}", workflow.name);
            let link = format!("](workflows/{}.md)", workflow.name);
            assert!(guide().contains(&link), "{} is not linked", workflow.name);
        }
        let linked = guide().matches("](workflows/").count();
        assert_eq!(
            linked,
            workflows().len(),
            "every link is to a workflow served"
        );
    }

    #[test]
    fn front_matter_splits_from_the_body() {
        let cases = [
            ("---\nname: a\n---\n\n# Body\n", "name: a", "# Body\n"),
            ("# No front matter\n", "", "# No front matter\n"),
            ("---\nunclosed\n", "", "---\nunclosed\n"),
        ];
        for (file, front, body) in cases {
            assert_eq!(split(file), (front, body), "{file:?}");
        }
    }
}
