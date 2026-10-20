-- A login keeps the name its provider gave (H3), so the user record's copy, set once at
-- creation and never refreshed, goes. Logins that exist start with no name: the dropped
-- column mixed names with fallbacks (an email, a subject), so none of it is carried over.
-- An identity may have no name, only emails: the column loses its NOT NULL, which SQLite
-- cannot alter, so the names move through a new column.

ALTER TABLE users DROP COLUMN name;

ALTER TABLE user_logins ADD COLUMN name TEXT CHECK (name IS NULL OR length(name) > 0);

ALTER TABLE identities ADD COLUMN display TEXT CHECK (display IS NULL OR length(display) > 0);
UPDATE identities SET display = name;
ALTER TABLE identities DROP COLUMN name;
ALTER TABLE identities RENAME COLUMN display TO name;
