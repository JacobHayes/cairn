-- C14, C16: the open proposals of a destination, and of a creator, newest first. The first
-- also serves the delete of every proposal of a journey, whatever its status (A19).
CREATE INDEX proposals_by_destination
  ON proposals (destination_kind, destination_id, status, created_at, id);
CREATE INDEX proposals_by_creator ON proposals (created_by, status, created_at, id);
