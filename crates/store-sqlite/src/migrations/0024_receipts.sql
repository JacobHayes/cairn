-- A conversation's receipt lines (author tool) are now plain words with no node keys. Those
-- written before are Markdown naming nodes by key, which cannot be put into the new words, so
-- they are dropped and the positions closed up. A line already in the new words is kept.
DELETE FROM conversation_messages
WHERE author = 'tool'
  AND NOT (content LIKE 'Applied a change to %'
        OR content LIKE 'That change was already applied to %'
        OR content LIKE 'A proposal for % is ready for review.'
        OR content LIKE 'Discarded a proposal for %'
        OR content LIKE 'The turn stopped: %');

CREATE TEMP TABLE kept_messages AS
SELECT conversation, row_number() OVER (PARTITION BY conversation ORDER BY position) - 1 AS position,
       author, at, content
FROM conversation_messages;
DELETE FROM conversation_messages;
INSERT INTO conversation_messages SELECT * FROM kept_messages;
DROP TABLE kept_messages;
