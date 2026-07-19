- Initialization, link/unlink, doctor, and purge now expose semantic structured
  outcomes instead of prose-only `messages` arrays. Initialization reports its
  action plus scope/store path or resolved link target; doctor reports a distinct
  `clean`/`repaired` status with missing/recovered file counts; purge reports a
  distinct `empty`/`purged` status with selected pad identities, total count, and
  descendant count. Human wording, ordering, styles, confirmation and recursive
  safety errors remain unchanged and are rendered by command-specific templates.
