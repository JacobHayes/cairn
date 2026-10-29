-- A node inserted from a route keeps the route and version it came from, as a journey does:
-- two nullable columns, set together. The code step that follows moves each insertion record
-- onto its root.

ALTER TABLE nodes ADD COLUMN from_route TEXT;

ALTER TABLE nodes ADD COLUMN from_version INTEGER CHECK (from_version >= 1);
