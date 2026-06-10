DROP TRIGGER default_versions_versions_yanked ON versions;
DROP TRIGGER default_versions_versions_delete ON versions;
DROP TRIGGER default_versions_versions_insert ON versions;

DROP FUNCTION default_versions_versions_yanked();
DROP FUNCTION default_versions_versions_delete();
DROP FUNCTION default_versions_versions_insert();
DROP FUNCTION rebuild_default_version(integer);
DROP FUNCTION compute_default_version(integer);

-- Restore the previous `num_versions`-only trigger (see the
-- `2025-02-11-163554_fix-num-versions-trigger` migration). This only maintains
-- `num_versions` and the initial `version_id`; the chosen default `version_id`
-- was kept up to date by application code and the `update_default_version`
-- background job.
CREATE OR REPLACE FUNCTION update_num_versions_from_versions() RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        INSERT INTO default_versions (crate_id, version_id, num_versions)
        VALUES (NEW.crate_id, NEW.id, 1)
        ON CONFLICT (crate_id) DO UPDATE
        SET num_versions = default_versions.num_versions + 1;
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        UPDATE default_versions
        SET num_versions = num_versions - 1
        WHERE crate_id = OLD.crate_id;
        RETURN OLD;
    END IF;
END
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_num_versions_from_versions
     AFTER INSERT OR DELETE ON versions
     FOR EACH ROW
     EXECUTE PROCEDURE update_num_versions_from_versions();
