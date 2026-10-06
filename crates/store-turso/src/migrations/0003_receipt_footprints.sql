-- H5: what each commit moved besides its target, kept with its receipt so a stale patch is
-- told what intervened even after a hard delete has removed the events (A19): the
-- revisions it left the proposals it wrote at, and what it wrote in the deployment.

ALTER TABLE patch_receipts
  ADD COLUMN proposals TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(proposals));

ALTER TABLE patch_receipts
  ADD COLUMN deployment_touched TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(deployment_touched));
