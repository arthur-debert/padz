- Pad updates now expose an `outcomes` array with an `updated` kind, canonical
  display path, title, and `structured`/`content`/`refresh` update kind. Repeated
  complete/reopen requests, same-parent moves, and empty `delete --completed`
  requests now expose typed `notices` instead of prose-only `messages`; mixed
  status requests expose `status_changed` outcomes alongside the paths and
  requested statuses that were no-ops. This intentionally extends the structured
  modification schema and removes the migrated English messages. Human wording,
  status display, and semantic `info`/`success` styling remain unchanged and are
  rendered by `modification_result.jinja`.
