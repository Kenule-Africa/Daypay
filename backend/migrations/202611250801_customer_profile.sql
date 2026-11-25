-- IMP-P3-009 / IMP-P3-010. The owner approved 202611250601 pending Codex's
-- sequencing; renumbered after the merged 202611250701 session controls.

-- Owner-approved username format (2026-11-25): ASCII letters, digits, underscore
-- and period, 3-30 characters, no leading, trailing or doubled period. Explicit
-- character lists keep the rule independent of database collation, and lower()
-- folds ASCII identically in every locale. NOT VALID leaves existing rows as
-- they are; every new active claim must comply, and retirement stays possible.
ALTER TABLE customers.usernames ADD CONSTRAINT usernames_approved_format CHECK (
    NOT is_active OR (
        username ~ '^@?[0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_.]{3,30}$'
        AND username !~ '^@?\.|\.$|\.\.'
    )
) NOT VALID;
