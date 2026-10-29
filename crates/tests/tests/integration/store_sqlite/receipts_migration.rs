//! The last migration on a database frozen at migration 13 (see `at_migration_13.sql`), with
//! a conversation holding a receipt line in the old Markdown words and one in the new: the
//! old one is dropped, the new one kept, and the dialogue around them reads in order.

#[cfg(test)]
mod receipts {
    use std::path::PathBuf;

    use cairn_store::{ConversationStore, MessageAuthor};
    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    const CONVERSATION: &str = r"
INSERT INTO conversations VALUES ('cv_one', 'u_lead', 'Hello', 1, 5);
INSERT INTO conversation_messages VALUES ('cv_one', 0, 'user', 1, 'Kickoff is done.');
INSERT INTO conversation_messages VALUES ('cv_one', 1, 'tool', 2, 'Marked done: `j_one/n_kickoff`.');
INSERT INTO conversation_messages VALUES ('cv_one', 2, 'assistant', 3, 'Done.');
INSERT INTO conversation_messages VALUES ('cv_one', 3, 'tool', 4, 'Applied a change to this journey.');
INSERT INTO conversation_messages VALUES ('cv_one', 4, 'assistant', 5, 'Also done.');
";

    #[test]
    fn an_old_receipt_is_dropped_and_the_rest_reads_in_order() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("receipts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(CONVERSATION).unwrap();
        drop(connection);
        let store = SqliteStore::open(&path).unwrap();
        let record = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(store.conversation(&"cv_one".parse().unwrap()))
            .unwrap()
            .unwrap();
        let read: Vec<(MessageAuthor, &str)> = record
            .messages
            .iter()
            .map(|message| (message.author, message.content.as_str()))
            .collect();
        assert_eq!(
            read,
            [
                (MessageAuthor::User, "Kickoff is done."),
                (MessageAuthor::Assistant, "Done."),
                (MessageAuthor::Tool, "Applied a change to this journey."),
                (MessageAuthor::Assistant, "Also done."),
            ]
        );
    }
}
