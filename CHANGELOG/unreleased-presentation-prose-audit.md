- Retire the unused generic `CmdMessage` presentation pipeline. Reusable command
  results and CLI projections no longer serialize always-empty `messages` arrays;
  generic pad modifications now serialize a typed semantic `action` token such as
  `pin` instead of a human past-tense verb such as `Pinned`. This intentional
  structured-schema cleanup is fixture-tested. Human wording, pluralization,
  ordering, styles, and terminal layout remain unchanged in CLI templates.
