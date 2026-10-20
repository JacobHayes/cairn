//! Migration 0011 on a database frozen at migration 10 (see `at_migration_10.sql`, a dump made
//! by the code before the rename). The expected state is written here, not derived from the
//! current fixtures: which identity each position names, and which text stays as written.

#[cfg(test)]
mod identities {
    use std::path::{Path, PathBuf};

    use std::collections::BTreeMap;

    use cairn_store::{AuthStore, Document, LoadTarget, Store};
    use cairn_store_turso::TursoStore;
    use serde_json::{Value, json};

    const DUMP: &str = include_str!("at_migration_10.sql");

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// A database file holding the frozen dump.
    async fn frozen(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let database = turso::Builder::new_local(path.to_str().unwrap())
            .build()
            .await
            .unwrap();
        let connection = database.connect().unwrap();
        let mut mode = connection
            .query("PRAGMA journal_mode = mvcc", ())
            .await
            .unwrap();
        while mode.next().await.unwrap().is_some() {}
        connection.execute_batch(DUMP).await.unwrap();
        path
    }

    /// The old word for an identity, which the new one contains.
    fn old_word(text: &str) -> bool {
        text.replace("identit", "").contains("entit")
    }

    fn is_key(text: &str) -> bool {
        text.strip_prefix("who_").is_some_and(|rest| {
            rest.len() == 16 && rest.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7'))
        })
    }

    /// The migrated store, and each identity's key by its name.
    async fn migrated(name: &str) -> (PathBuf, TursoStore, BTreeMap<String, String>) {
        let path = frozen(name).await;
        let store = TursoStore::open(&path).await.unwrap();
        let Some(Document::Deployment(deployment)) =
            store.load(&LoadTarget::Deployment).await.unwrap()
        else {
            panic!("no deployment")
        };
        let value = serde_json::to_value(&deployment).unwrap();
        let by_name = value["identities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|identity| {
                (
                    identity["name"].as_str().unwrap().to_owned(),
                    identity["key"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        (path, store, by_name)
    }

    async fn journey(store: &TursoStore, id: &str) -> Value {
        let Some(Document::Journey(journey)) = store
            .load(&LoadTarget::Journey(cairn_store::build::id(id)))
            .await
            .unwrap()
        else {
            panic!("no journey {id}")
        };
        serde_json::to_value(&journey.graph).unwrap()
    }

    /// Every row of a query, each column as text, read after the store is closed.
    async fn rows(path: &Path, sql: &str) -> Vec<Vec<String>> {
        let database = turso::Builder::new_local(path.to_str().unwrap())
            .build()
            .await
            .unwrap();
        let connection = database.connect().unwrap();
        let mut result = connection.query(sql, ()).await.unwrap();
        let mut out = Vec::new();
        while let Some(row) = result.next().await.unwrap() {
            out.push(
                (0..row.column_count())
                    .map(|i| match row.get_value(i).unwrap() {
                        turso::Value::Text(text) => text,
                        turso::Value::Integer(n) => n.to_string(),
                        turso::Value::Null => String::new(),
                        other => format!("{other:?}"),
                    })
                    .collect(),
            );
        }
        out
    }

    #[test]
    fn identities_get_opaque_keys_and_aliases_follow() {
        run(async {
            let (_, store, by_name) = migrated("keys").await;
            assert_eq!(by_name.len(), 6);
            for key in by_name.values() {
                assert!(is_key(key), "{key}");
            }
            let mut distinct: Vec<_> = by_name.values().collect();
            distinct.sort();
            distinct.dedup();
            assert_eq!(distinct.len(), 6);
            let Some(Document::Deployment(deployment)) =
                store.load(&LoadTarget::Deployment).await.unwrap()
            else {
                panic!()
            };
            let value = serde_json::to_value(&deployment).unwrap();
            let aliases = value["aliases"].as_object().unwrap();
            assert_eq!(aliases.len(), 1);
            let (alias, target) = aliases.iter().next().unwrap();
            assert!(is_key(alias));
            assert_eq!(target, &json!(by_name["Ann"]));
            assert_ne!(alias, &by_name["Ann"]);
            let ann = value["identities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i["name"] == "Ann")
                .unwrap();
            assert_eq!(
                ann["emails"],
                json!(["ann.r@example.org", "ann@example.org"])
            );
        });
    }

    #[test]
    fn every_position_follows_its_identity_and_text_stays() {
        run(async {
            let (_, store, by_name) = migrated("positions").await;
            let (ann, ben, cy) = (&by_name["Ann"], &by_name["Ben"], &by_name["Cy"]);
            let graph = journey(&store, "j_one").await;
            let node = |key: &str| {
                graph["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|n| n["key"] == key)
                    .unwrap()
                    .clone()
            };
            assert_eq!(node("n_who")["answer_type"], "identity");
            assert_eq!(node("n_team")["answer_type"], "identity_list");
            // Conditions over identity decisions follow; the unknown key is mapped, not dropped.
            assert_eq!(node("n_a")["relevant_when"]["equals"]["value"], json!(ann));
            let members = node("n_b")["relevant_when"]["in"]["values"].clone();
            assert_eq!(members[0], json!(ann));
            assert!(is_key(members[1].as_str().unwrap()));
            assert!(!by_name.values().any(|k| k == members[1].as_str().unwrap()));
            assert_eq!(
                node("n_d")["relevant_when"]["all"][0]["contains"]["value"],
                json!(ben)
            );
            assert_eq!(node("n_d")["participations"]["k_watch"], json!([cy, ben]));
            // Text compared with a text decision, and a choice named like the old type, stay as written.
            assert_eq!(node("n_c")["relevant_when"]["equals"]["value"], "e_ann");
            assert_eq!(node("n_choice")["choices"], json!(["entity", "other"]));
            let answers = &graph["state"]["answers"];
            assert_eq!(answers["n_who"], json!({"identity": ann}));
            assert_eq!(answers["n_team"], json!({"identity_list": [cy, ben]}));
            assert_eq!(answers["n_note"], json!({"text": "e_ann"}));
            assert_eq!(answers["n_choice"], json!({"single_choice": "entity"}));
            assert_eq!(graph["state"]["role_fills"]["r_panel"], json!([cy, ben]));
            // A route version's participation follows the lead.
            let Some(Document::RouteVersion(version)) = store
                .load(&LoadTarget::RouteVersion {
                    route: cairn_store::build::id("r_hire"),
                    version: cairn_schema::VersionNumber::FIRST,
                })
                .await
                .unwrap()
            else {
                panic!()
            };
            let version = serde_json::to_value(&version).unwrap();
            let text = version.to_string();
            assert!(text.contains(&by_name["Lead"]), "{text}");
            assert!(!text.contains("e_lead"), "{text}");
        });
    }

    #[test]
    fn proposals_load_with_migrated_tags_and_keys() {
        run(async {
            let (_, store, by_name) = migrated("proposals").await;
            let get = |id: &str| {
                let store = &store;
                let id = cairn_store::build::id(id);
                async move { serde_json::to_value(store.proposal(&id).await.unwrap().unwrap()).unwrap() }
            };
            let deploy = get("pr_deploy").await;
            let mutations = deploy["draft"]["mutations"].as_array().unwrap();
            assert_eq!(mutations[0]["op"], "create_identity");
            let proposed = mutations[0]["identity"]["key"].as_str().unwrap();
            assert!(is_key(proposed));
            assert!(
                !by_name.values().any(|k| k == proposed),
                "a key only a proposal names still gets one"
            );
            assert_eq!(mutations[1]["op"], "edit_identity");
            assert_eq!(mutations[1]["identity"]["key"], json!(by_name["Ben"]));
            assert_eq!(mutations[2]["op"], "merge_identities");
            assert_eq!(mutations[2]["survivor"], json!(by_name["Ben"]));
            assert_eq!(mutations[2]["merged"], json!(by_name["Unused"]));

            let journey_proposal = get("pr_journey").await;
            let rider = journey_proposal["draft"]["mutations"][0]["identity"]["key"]
                .as_str()
                .unwrap()
                .to_owned();
            assert!(is_key(&rider));
            let text = journey_proposal.to_string();
            assert!(
                text.contains(&format!(
                    "\"identity_list\":[\"{rider}\",\"{}\"]",
                    by_name["Ann"]
                )),
                "{text}"
            );
            assert!(
                text.contains(&format!("\"source\":[\"{rider}\"]")),
                "{text}"
            );
            assert!(text.contains("\"answer_type\":\"identity_list\""), "{text}");

            let review = get("pr_review").await;
            let items = review["draft"]["items"].as_array().unwrap();
            // The map is the reviewer's, keyed by a choice that shares the old type's name.
            assert_eq!(
                items[0]["resolution"],
                json!({"map_choices": {"map": {"entity": "other"}}})
            );
            assert_eq!(items[1]["item"], "participation");
            assert_eq!(items[1]["identity"], json!(by_name["Cy"]));
            let violation = items[2]["violation"].to_string();
            assert!(violation.contains("identity_unresolved"), "{violation}");
            assert!(
                !old_word(&violation) && !violation.contains("e_zed"),
                "{violation}"
            );
        });
    }

    #[test]
    fn conflicts_and_retyping_proposals_keep_their_meaning() {
        run(async {
            let (_, store, by_name) = migrated("conflicts").await;
            let get = |id: &str| {
                let store = &store;
                let id = cairn_store::build::id(id);
                async move { serde_json::to_value(store.proposal(&id).await.unwrap().unwrap()).unwrap() }
            };
            // Conflicts over a participation, a field and a node's shape name identities on both sides.
            let conflicts = get("pr_conflicts").await;
            let items = conflicts["draft"]["items"].as_array().unwrap();
            let (ann, cy, ben) = (&by_name["Ann"], &by_name["Cy"], &by_name["Ben"]);
            assert_eq!(items[0]["conflict"]["journey"], json!([cy]));
            assert_eq!(items[0]["conflict"]["route"], json!([ann]));
            let field = &items[1]["conflict"];
            assert_eq!(
                field["journey"]["relevant_when"]["equals"]["value"],
                json!(ann)
            );
            assert_eq!(
                field["route"]["relevant_when"]["equals"]["value"],
                json!(ben)
            );
            let shape = &items[2]["conflict"];
            assert_eq!(shape["journey"]["answer_type"], "identity_list");
            assert_eq!(shape["route"]["answer_type"], "text");

            // A proposal that retypes the note as an identity decision does not change what the
            // journey's text comparison against the note means.
            let retype = get("pr_retype").await;
            assert_eq!(
                retype["draft"]["mutations"][0]["node"]["answer_type"],
                "identity"
            );
            // A condition may name a decision a later mutation adds: both are read as the candidate
            // ends up, so the identity comparison follows its key.
            let forward = get("pr_forward").await;
            let compared =
                &forward["draft"]["mutations"][0]["node"]["relevant_when"]["equals"]["value"];
            assert_eq!(compared, &json!(by_name["Ann"]));

            // A route proposal reads its draft, where Who is text now: the published version's
            // identity decision of the same key does not make the comparison one of identities.
            let route = get("pr_route").await;
            assert_eq!(
                route["draft"]["mutations"][0]["node"]["relevant_when"]["equals"]["value"],
                "e_lead"
            );
            let graph = journey(&store, "j_one").await;
            let n_c = graph["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["key"] == "n_c")
                .unwrap();
            assert_eq!(n_c["relevant_when"]["equals"]["value"], "e_ann");
        });
    }

    #[test]
    fn stored_names_and_receipts_move_and_hashes_stay() {
        run(async {
            let (path, store, by_name) = migrated("stored").await;
            drop(store);
            let names = rows(&path, "SELECT name FROM sqlite_master UNION SELECT name FROM pragma_table_info('answers') UNION SELECT name FROM pragma_table_info('nodes')").await;
            assert!(names.iter().all(|row| !old_word(&row[0])), "{names:?}");
            let sql = rows(&path, "SELECT sql FROM sqlite_master WHERE sql IS NOT NULL").await;
            assert!(sql.iter().all(|row| !old_word(&row[0])), "{sql:?}");
            // The receipt of a deleted journey still names the identity it touched.
            let touched = rows(
                &path,
                "SELECT deployment_touched FROM patch_receipts WHERE patch_id = 'p_gone'",
            )
            .await;
            assert_eq!(
                touched[0][0],
                format!("[{{\"identity\":\"{}\"}}]", by_name["Rider Two"])
            );
            let merged = rows(
                &path,
                "SELECT deployment_touched FROM patch_receipts WHERE patch_id = 'p_merge'",
            )
            .await;
            assert!(merged[0][0].contains("identity_alias"), "{}", merged[0][0]);
            // Content hashes are the originals: a patch retried after the migration is still recognised.
            let hash = rows(
                &path,
                "SELECT content_hash FROM patch_receipts WHERE patch_id = 'p_one'",
            )
            .await;
            assert_eq!(
                hash[0][0],
                "96e81455b2f5e443c58e901dfbe79ad136ac22440deb21850bb008699b3dca1a"
            );
        });
    }

    #[test]
    fn the_event_log_names_identities_by_their_new_keys() {
        run(async {
            let (path, store, by_name) = migrated("events").await;
            drop(store);
            // Every identity-creating event names a new-shape key; none keeps an old tag or key (`e_lead` stays only as the text a route proposal compares).
            let events = rows(&path, "SELECT event_type, subject, delta FROM events").await;
            let known: Vec<&String> = by_name.values().collect();
            for event in &events {
                assert!(
                    !event
                        .iter()
                        .any(|c| ["e_ben", "e_cy", "e_ann_dup", "e_rider2", "e_unused"]
                            .iter()
                            .any(|k| c.contains(k))
                            || [
                                "\"entity\":{",
                                "\"entity\":\"e_",
                                "\"entity\":\"who_",
                                "\"entity_"
                            ]
                            .iter()
                            .any(|k| c.contains(k))
                            || c.contains("_entit")),
                    "{event:?}"
                );
            }
            let created: Vec<_> = events
                .iter()
                .filter(|e| e[0] == "identity_created")
                .collect();
            assert_eq!(created.len(), 6);
            for event in created {
                let subject: Value = serde_json::from_str(&event[1]).unwrap();
                let key = subject["identity"].as_str().unwrap();
                // Either a live identity, or the one merged away (now an alias of Ann's).
                assert!(
                    is_key(key)
                        && (known.contains(&&key.to_owned()) || event[2].contains("Ann R.")),
                    "{event:?}"
                );
            }
        });
    }

    #[test]
    fn sign_in_records_become_logins() {
        run(async {
            let (path, store, _) = migrated("logins").await;
            let logins = store
                .logins_of(&cairn_store::build::id("u_lead"))
                .await
                .unwrap();
            assert_eq!(logins.len(), 2);
            drop(store);
            let log = rows(&path, "SELECT event FROM auth_log ORDER BY seq").await;
            assert_eq!(
                log,
                vec![
                    vec!["login_linked"],
                    vec!["login_linked"],
                    vec!["user_created"]
                ]
            );
            let emails = rows(&path, "SELECT email FROM login_emails").await;
            assert_eq!(emails.len(), 2);
        });
    }

    /// A login keeps its provider's name, and the user record's copy is gone; an identity
    /// may have no name, so the column takes NULL. The logins that existed start with none.
    #[test]
    fn names_move_to_the_login_and_an_identity_may_have_none() {
        run(async {
            let (path, store, _) = migrated("names").await;
            let logins = store
                .logins_of(&cairn_store::build::id("u_lead"))
                .await
                .unwrap();
            assert!(logins.iter().all(|login| login.name.is_none()));
            drop(store);
            let users = rows(&path, "SELECT name FROM pragma_table_info('users')").await;
            assert!(users.iter().all(|column| column[0] != "name"), "{users:?}");
            let columns = rows(
                &path,
                "SELECT name, \"notnull\" FROM pragma_table_info('identities') WHERE name = 'name'",
            )
            .await;
            assert_eq!(columns, vec![vec!["name", "0"]]);
            let named = rows(
                &path,
                "SELECT count(*) FROM identities WHERE name IS NOT NULL",
            )
            .await;
            let total = rows(&path, "SELECT count(*) FROM identities").await;
            assert_eq!(named, total, "every name survived the move");
        });
    }
}
