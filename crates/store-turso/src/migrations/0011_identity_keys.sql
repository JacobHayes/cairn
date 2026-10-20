-- Where the old key of each identity goes (migration 0011): written by the code step that
-- reads every typed position a key can sit in, read by the statements after it, dropped with
-- them. An identity's new key is random-looking and derived from nothing a person wrote.
CREATE TABLE identity_key_map (
  old TEXT PRIMARY KEY,
  new TEXT NOT NULL UNIQUE
) STRICT;
