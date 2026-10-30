//! The migration that makes a stage an ordinary group, on a database frozen at migration 13
//! (see `at_migration_13.sql`) with groups holding the four fields a stage used to have, their
//! history, and proposals that edited them. An opening becomes a requirement and a closing a
//! due-by (merged into the group's own rule where it has one); a bound that did not gate or
//! close is dropped; groups whose own rule cannot take the closing stop the migration and are
//! all listed; stored events still read back, including a closing edit on a group with its
//! own rule and groups of a copied graph; a proposal's added nodes are converted, its other
//! stage edits and stage-bound review items dropped, and an open one that lost an edit is
//! discarded with a reason.

#[cfg(test)]
mod plain_stages {
    use std::path::PathBuf;

    use cairn_schema::{Mutation, ReviewItem, Write};
    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    const NODE: &str = "INSERT INTO nodes (graph_id, key, id, kind, title, opens_at, closes_at, gates, closes, due_by) VALUES";

    fn groups(extra: &str) -> String {
        format!(
            r#"
{NODE} ('journey/j_one', 'n_both', 'both', 'group', 'Both', 'n_kick', 'n_gate', 1, 1, NULL);
{NODE} ('journey/j_one', 'n_loose', 'loose', 'group', 'Loose', 'n_kick', 'n_gate', 0, 0, NULL);
{NODE} ('journey/j_one', 'n_zero', 'zero', 'group', 'Zero', NULL, 'n_gate', 1, 1, '{{"after":"n_kick"}}');
{NODE} ('journey/j_one', 'n_same', 'same', 'group', 'Same', NULL, 'n_gate', 1, 1, '{{"before":"n_gate","offset":3}}');
{NODE} ('journey/j_one', 'n_plain', 'plain', 'group', 'Plain', NULL, NULL, NULL, NULL, NULL);
{extra}
"#
        )
    }

    const HISTORY: &str = r#"
INSERT INTO "events" VALUES (60, 'p_g1', 0, 'journey', 'j_one', 'node_added', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000060, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node":{"key":"n_ev","id":"ev","kind":"group","title":"Ev","opens_at":"n_kick","closes_at":"n_gate"}}}}}]');
INSERT INTO "events" VALUES (61, 'p_g2', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000061, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ev","value":{"opens_at":"n_gate"}}}}}}]');
INSERT INTO "events" VALUES (62, 'p_g3', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000062, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ev","value":{"gates":false}}}}}}]');
INSERT INTO "events" VALUES (63, 'p_g4', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000063, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ev","value":{"closes_at":null}}}}}}]');
INSERT INTO "events" VALUES (64, 'p_g5', 0, 'journey', 'j_one', 'node_added', 'u_lead', NULL, NULL, '{"node":"n_ruled"}', 1791288000000000064, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node":{"key":"n_ruled","id":"ruled","kind":"group","title":"Ruled","due_by":{"after":"n_kick","offset":0}}}}}}]');
INSERT INTO "events" VALUES (65, 'p_g6', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ruled"}', 1791288000000000065, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ruled","value":{"closes_at":"n_gate"}}}}}}]');
INSERT INTO "events" VALUES (66, 'p_g7', 0, 'route', 'r_copy', 'draft_opened', 'u_lead', NULL, NULL, '{"route":"r_copy"}', 1791288000000000066, NULL, '[{"put":{"graph":{"graph":{"route_version":{"route":"r_copy","version":1}},"record":{"node":{"key":"n_ruled","id":"ruled","kind":"group","title":"Ruled","due_by":{"after":"n_kick","offset":0}}}}}},{"copy_graph":{"from":{"route_version":{"route":"r_copy","version":1}},"to":{"route_draft":"r_copy"}}}]');
INSERT INTO "events" VALUES (67, 'p_g8', 0, 'route', 'r_copy', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ruled"}', 1791288000000000067, NULL, '[{"put":{"graph":{"graph":{"route_draft":"r_copy"},"record":{"node_field":{"node":"n_ruled","value":{"closes_at":"n_gate"}}}}}}]');
INSERT INTO "events" VALUES (68, 'p_g9', 0, 'journey', 'j_one', 'proposal_created', 'u_lead', NULL, NULL, '{"proposal":"pr_ev"}', 1791288000000000068, NULL, '[{"put":{"proposal":{"id":"pr_ev","destination":{"journey":"j_one"},"revision":1,"status":"open","draft":{"title":"Stage","destination_revision":1,"mutations":[{"op":"add_node","node":{"key":"n_stg","id":"stg","kind":"group","title":"Stg","opens_at":"n_kick"}},{"op":"set_node_field","node":"n_both","value":{"closes_at":"n_gate"}}],"items":[{"item":"violation","violation":{"code":"stage_bound_not_milestone","at":{"field":"closes_at"},"message":"m"}}]},"created_by":"u_lead","created_at":"2026-10-18T21:14:56Z"}}}]');
INSERT INTO "events" VALUES (69, 'p_g10', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ruled"}', 1791288000000000069, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ruled","value":{"due_by":{"before":"n_x"}}}}}}}]');
INSERT INTO "proposals" VALUES ('pr_bad', 'journey', 'j_one', 1, 'open', 'Unsayable stage', NULL, 1, '[{"op":"add_node","node":{"key":"n_bad","id":"bad","kind":"group","title":"Bad","closes_at":"n_gate","due_by":{"before":"n_x","offset":3}}}]', '[]', NULL, 'u_lead', 1791288000000000000);
INSERT INTO "proposals" VALUES ('pr_stage', 'journey', 'j_one', 1, 'open', 'Stage edit', NULL, 1, '[{"op":"set_node_field","node":"n_both","value":{"opens_at":"n_gate"}}]', '[]', NULL, 'u_lead', 1791288000000000000);
INSERT INTO "proposals" VALUES ('pr_old', 'journey', 'j_one', 1, 'applied', 'Mixed', NULL, 1, '[{"op":"set_node_field","node":"n_both","value":{"closes_at":"n_gate"}},{"op":"transition","node":"n_work","transition":"complete"}]', '[]', NULL, 'u_lead', 1791288000000000000);
INSERT INTO "proposals" VALUES ('pr_add', 'journey', 'j_one', 1, 'open', 'Add a stage', NULL, 1, '[{"op":"add_node","node":{"key":"n_stg","id":"stg","kind":"group","title":"Stg","opens_at":"n_kick","closes_at":"n_gate"}}]', '[{"item":"violation","violation":{"code":"contradictory_chain","at":{},"message":"m","chains":{"chains":[{"chain":{"constraints":[{"before":"n_kick:finish","after":"n_gate:start","days":0,"source":"stage_close:n_stg"}]},"shortfall_days":1}]}}},{"item":"exclusion","node":"n_x","excluded":false}]', NULL, 'u_lead', 1791288000000000000);
"#;

    fn open(name: &str, script: &str) -> (PathBuf, Result<SqliteStore, String>) {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("plain-stages-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(script).unwrap();
        drop(connection);
        let opened = SqliteStore::open(&path).map_err(|error| error.to_string());
        (path, opened)
    }

    fn column<T: rusqlite::types::FromSql>(path: &PathBuf, sql: &str) -> Option<T> {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row(sql, [], |row| row.get(0))
            .ok()
    }

    fn requires(path: &PathBuf, node: &str) -> Vec<String> {
        let connection = rusqlite::Connection::open(path).unwrap();
        connection
            .prepare("SELECT requires FROM edges WHERE graph_id = 'journey/j_one' AND node = ?1 ORDER BY requires")
            .unwrap()
            .query_map([node], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn due_by(path: &PathBuf, node: &str) -> Option<serde_json::Value> {
        let stored: Option<String> = column(
            path,
            &format!(
                "SELECT due_by FROM nodes WHERE graph_id = 'journey/j_one' AND key = '{node}'"
            ),
        );
        stored.map(|text| serde_json::from_str(&text).unwrap())
    }

    #[test]
    fn bounds_become_requirements_and_deadlines() {
        let (path, opened) = open("map", &groups(HISTORY));
        drop(opened.unwrap());

        let columns: Vec<String> = rusqlite::Connection::open(&path)
            .unwrap()
            .prepare("SELECT name FROM pragma_table_info('nodes')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for gone in ["opens_at", "closes_at", "gates", "closes"] {
            assert!(!columns.iter().any(|name| name == gone), "{gone}");
        }

        assert_eq!(requires(&path, "n_both"), ["n_kick"]);
        assert_eq!(
            due_by(&path, "n_both").unwrap(),
            serde_json::json!({"before": "n_gate"})
        );
        assert_eq!(requires(&path, "n_loose"), [] as [std::string::String; 0]);
        assert_eq!(
            due_by(&path, "n_loose"),
            None,
            "a bound that did not close is dropped"
        );
        assert_eq!(
            due_by(&path, "n_zero").unwrap(),
            serde_json::json!({"after": ["n_gate", "n_kick"]}),
            "a zero-offset rule takes the milestone as one more source"
        );
        assert_eq!(
            due_by(&path, "n_same").unwrap(),
            serde_json::json!({"before": "n_gate", "offset": 3}),
            "a rule already before the milestone is left alone"
        );
        assert_eq!(due_by(&path, "n_plain"), None);
    }

    #[test]
    fn stored_history_still_reads_back() {
        let (path, opened) = open("history", &groups(HISTORY));
        drop(opened.unwrap());
        let delta = |seq: i64| -> serde_json::Value {
            let text: String = column(
                &path,
                &format!("SELECT delta FROM events WHERE seq = {seq}"),
            )
            .unwrap();
            serde_json::from_str(&text).unwrap()
        };
        let created = delta(60);
        let node = &created[0]["put"]["graph"]["record"]["node"];
        assert_eq!(node["requires"], serde_json::json!(["n_kick"]));
        assert_eq!(node["due_by"], serde_json::json!({"before": "n_gate"}));
        assert!(node.get("opens_at").is_none() && node.get("closes_at").is_none());

        let moved = delta(61);
        assert_eq!(
            moved.as_array().unwrap().len(),
            2,
            "the old requirement goes, the new comes"
        );
        assert_eq!(
            moved[0]["remove"]["in_graph"]["key"]["edge"]["requires"],
            "n_kick"
        );
        assert_eq!(
            moved[1]["put"]["graph"]["record"]["edge"]["requires"],
            "n_gate"
        );

        let dropped = delta(62);
        assert_eq!(
            dropped.as_array().unwrap().len(),
            1,
            "a group that stops gating loses its requirement"
        );
        assert_eq!(
            dropped[0]["remove"]["in_graph"]["key"]["edge"]["requires"],
            "n_gate"
        );

        let cleared = delta(63);
        assert_eq!(
            cleared[0]["put"]["graph"]["record"]["node_field"]["value"],
            serde_json::json!({"due_by": null})
        );

        let merged = serde_json::json!({"due_by": {"after": ["n_gate", "n_kick"], "offset": 0}});
        assert_eq!(
            delta(65)[0]["put"]["graph"]["record"]["node_field"]["value"],
            merged,
            "a closing edit on a group with its own rule rewrites the rule"
        );
        assert_eq!(
            delta(67)[0]["put"]["graph"]["record"]["node_field"]["value"],
            merged,
            "a graph copied whole carries its groups' rules to the copy"
        );
        assert_eq!(
            delta(69)[0]["put"]["graph"]["record"]["node_field"]["value"],
            serde_json::json!({"due_by": {"before": ["n_gate", "n_x"]}}),
            "a rule written while a closing still bounds the group keeps the closing"
        );
    }

    #[test]
    fn everything_stored_reads_back() {
        let (path, opened) = open("typed", &groups(HISTORY));
        drop(opened.unwrap());
        let connection = rusqlite::Connection::open(&path).unwrap();
        let strings = |sql: &str| -> Vec<String> {
            connection
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        for delta in strings("SELECT delta FROM events") {
            serde_json::from_str::<Vec<Write>>(&delta).expect("a stored delta deserializes");
        }
        for mutations in strings("SELECT mutations FROM proposals") {
            serde_json::from_str::<Vec<Mutation>>(&mutations)
                .expect("a stored proposal's mutations deserialize");
        }
        for items in strings("SELECT items FROM proposals") {
            serde_json::from_str::<Vec<ReviewItem>>(&items)
                .expect("a stored proposal's items deserialize");
        }
        let created: String = column(&path, "SELECT delta FROM events WHERE seq = 68").unwrap();
        let created: serde_json::Value = serde_json::from_str(&created).unwrap();
        let draft = &created[0]["put"]["proposal"]["draft"];
        assert_eq!(
            draft["mutations"].as_array().unwrap().len(),
            1,
            "the stage edit is dropped from a proposal's event, the added stage kept"
        );
        assert_eq!(
            draft["mutations"][0]["node"]["requires"],
            serde_json::json!(["n_kick"])
        );
        assert_eq!(draft["items"], serde_json::json!([]));
    }

    #[test]
    fn proposals_lose_stage_edits() {
        let (path, opened) = open("proposals", &groups(HISTORY));
        drop(opened.unwrap());
        let status = |id: &str| -> String {
            column(
                &path,
                &format!("SELECT status FROM proposals WHERE id = '{id}'"),
            )
            .unwrap()
        };
        assert_eq!(status("pr_stage"), "discarded");
        assert_eq!(
            status("pr_bad"),
            "discarded",
            "a stage whose rule cannot take its closing is not kept weaker"
        );
        let reason: String = column(
            &path,
            "SELECT description FROM proposals WHERE id = 'pr_stage'",
        )
        .unwrap();
        assert!(
            !reason.is_empty(),
            "an open proposal is discarded with a reason"
        );
        let mutations: String = column(
            &path,
            "SELECT mutations FROM proposals WHERE id = 'pr_stage'",
        )
        .unwrap();
        assert_eq!(mutations, "[]");

        assert_eq!(status("pr_old"), "applied");
        let kept: serde_json::Value = serde_json::from_str(
            &column::<String>(&path, "SELECT mutations FROM proposals WHERE id = 'pr_old'")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            kept.as_array().unwrap().len(),
            1,
            "a settled proposal keeps its other mutations"
        );
        assert_eq!(kept[0]["op"], "transition");
        assert_eq!(
            status("pr_add"),
            "open",
            "a proposal that only added a stage is converted, not discarded"
        );
        let added: serde_json::Value = serde_json::from_str(
            &column::<String>(&path, "SELECT mutations FROM proposals WHERE id = 'pr_add'")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(added[0]["node"]["requires"], serde_json::json!(["n_kick"]));
        assert_eq!(
            added[0]["node"]["due_by"],
            serde_json::json!({"before": "n_gate"})
        );
        assert_eq!(
            status("pr_one"),
            "open",
            "a proposal with no stage edit is untouched"
        );
    }

    #[test]
    fn groups_that_cannot_take_their_closing_stop_the_migration() {
        let bad = groups(&format!(
            "{NODE} ('journey/j_one', 'n_bad_a', 'bada', 'group', 'A', NULL, 'n_gate', 1, 1, '{{\"after\":\"n_kick\",\"offset\":2}}');\n\
             {NODE} ('journey/j_one', 'n_bad_b', 'badb', 'group', 'B', NULL, 'n_gate', 1, 1, '{{\"after\":\"n_kick\",\"offset\":5}}');"
        ));
        let (path, opened) = open("refused", &bad);
        let Err(message) = opened else {
            panic!("the migration must refuse");
        };
        assert!(
            message.contains("n_bad_a") && message.contains("n_bad_b"),
            "{message}"
        );
        let closes: Option<String> = column(
            &path,
            "SELECT closes_at FROM nodes WHERE graph_id = 'journey/j_one' AND key = 'n_bad_a'",
        );
        assert_eq!(closes.as_deref(), Some("n_gate"));
    }
}
