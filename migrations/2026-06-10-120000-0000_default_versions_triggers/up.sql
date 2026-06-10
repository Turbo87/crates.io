-- Maintain the `default_versions` table entirely from database triggers, the
-- same way `reverse_dependencies` is maintained.
--
-- A crate's default version is derived purely from its `versions` rows: the set
-- of versions that exist, and the `yanked` flag of each. A trigger reacts to
-- each of those inputs (version inserted, version deleted, `yanked` toggled) and
-- recomputes the crate's `default_versions` row. Crate deletions are handled by
-- the existing `ON DELETE CASCADE` foreign key, not by these triggers.
--
-- This replaces the previous split design where the initial row and the
-- `num_versions` counter were maintained by `update_num_versions_from_versions`
-- while the chosen `version_id` was maintained by application code and a
-- background job. Both responsibilities now live in `rebuild_default_version`.


-- The version id that should be a crate's default version, or NULL when the
-- crate has no versions. The ordering matches the previous Rust implementation:
--
--   1. non-yanked before yanked
--   2. releases before pre-releases (the post-patch byte of `semver_ord_v2` is
--      0x03 for releases and < 0x03 for pre-releases)
--   3. higher semver first
--   4. higher id first as a tie-breaker
CREATE FUNCTION compute_default_version(p_crate_id integer) RETURNS integer
LANGUAGE sql STABLE AS $$
    SELECT v.id
    FROM versions v
    WHERE v.crate_id = p_crate_id
    ORDER BY v.yanked,
             get_byte(v.semver_ord_v2, octet_length(v.semver_ord_v2) - 1) <> 3,
             v.semver_ord_v2 DESC,
             v.id DESC
    LIMIT 1;
$$;

COMMENT ON FUNCTION compute_default_version(integer) IS
    'Returns the version id that should be the crate''s default version, or NULL if it has no versions.';


-- Reconcile a crate's `default_versions` row with its current `versions`. When
-- the crate has versions, the row is upserted with the freshly chosen default
-- and the version count; when it has none, the (now dangling) row is removed.
--
-- Removing the row before commit is what lets a version delete succeed: the
-- `default_versions.version_id` foreign key is `NO ACTION DEFERRABLE INITIALLY
-- DEFERRED`, so deleting the current default version only fails at commit unless
-- this trigger has repointed or removed the row first.
CREATE FUNCTION rebuild_default_version(p_crate_id integer) RETURNS void
LANGUAGE plpgsql VOLATILE AS $$
DECLARE
    new_version_id integer := compute_default_version(p_crate_id);
    version_count integer;
BEGIN
    IF new_version_id IS NULL THEN
        DELETE FROM default_versions
        WHERE default_versions.crate_id = p_crate_id;
    ELSE
        SELECT count(*) INTO version_count
        FROM versions
        WHERE versions.crate_id = p_crate_id;

        INSERT INTO default_versions (crate_id, version_id, num_versions)
        VALUES (p_crate_id, new_version_id, version_count)
        ON CONFLICT (crate_id) DO UPDATE
            SET version_id = EXCLUDED.version_id,
                num_versions = EXCLUDED.num_versions;
    END IF;
END;
$$;

COMMENT ON FUNCTION rebuild_default_version(integer) IS
    'Rebuilds the `default_versions` row (chosen version and version count) for the given crate id.';


-- Replace the old `num_versions`-only trigger: the new triggers below maintain
-- both `version_id` and `num_versions`.
DROP TRIGGER IF EXISTS trigger_update_num_versions_from_versions ON versions;
DROP FUNCTION IF EXISTS update_num_versions_from_versions();

--
-- AFTER INSERT ON versions
--

CREATE FUNCTION default_versions_versions_insert() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM rebuild_default_version(affected.crate_id)
    FROM (SELECT DISTINCT crate_id FROM inserted) AS affected;
    RETURN NULL;
END;
$$;

CREATE TRIGGER default_versions_versions_insert
    AFTER INSERT ON versions
    REFERENCING NEW TABLE AS inserted
    FOR EACH STATEMENT
    EXECUTE FUNCTION default_versions_versions_insert();

--
-- AFTER DELETE ON versions
--

CREATE FUNCTION default_versions_versions_delete() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM rebuild_default_version(affected.crate_id)
    FROM (SELECT DISTINCT crate_id FROM deleted) AS affected;
    RETURN NULL;
END;
$$;

CREATE TRIGGER default_versions_versions_delete
    AFTER DELETE ON versions
    REFERENCING OLD TABLE AS deleted
    FOR EACH STATEMENT
    EXECUTE FUNCTION default_versions_versions_delete();

--
-- AFTER UPDATE OF yanked ON versions
--

-- A column list (`UPDATE OF yanked`) is incompatible with transition tables, so
-- this is a row-level trigger guarded by a `WHEN` clause, like the equivalent
-- `reverse_dependencies` trigger. Yanks affect a single version at a time, so
-- recomputing per row is fine.
CREATE FUNCTION default_versions_versions_yanked() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM rebuild_default_version(NEW.crate_id);
    RETURN NULL;
END;
$$;

CREATE TRIGGER default_versions_versions_yanked
    AFTER UPDATE OF yanked ON versions
    FOR EACH ROW
    WHEN (OLD.yanked IS DISTINCT FROM NEW.yanked)
    EXECUTE FUNCTION default_versions_versions_yanked();

-- Note: existing `default_versions` rows are already correct (they were
-- maintained by the previous trigger + application code), so no backfill is
-- needed here. Use the `crates-admin default-versions update` command to
-- reconcile rows if necessary.
