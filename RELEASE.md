# Tuwunel 1.8.3

August 5, 2026

### New Features & Enhancements

- **QR-code login** 📲 <ins>Take a picture of Element Web with Element X: **login instantly.**</ins> (MSC4108). Rendezvous sessions are served over the MSC4388 transport, with bounded in-memory payloads, a session cap and TTL, and their own rate limit. The native OIDC server grows to meet it, serving account login for the device authorization grant and account management with no identity provider configured at all. Raised by @rjwalters in (#525).

- **Video thumbnails.** A client that uploads a video without a thumbnail left the server nothing to preview, so `media_video_thumbnail_command` names a program that extracts one still frame for the image thumbnailer to scale and crop. Inspired by @az4521 in (#397) and implemented by @x86pup, who also caps what the thumbnailer will decode at all with `media_thumbnail_max_pixels`, checked against the image header before any decoder allocates. Requests whose thumbnail would only reproduce the source skip generation entirely, shipped by @lhjt in (#521).

- **Native journald logging.** Under systemd every line landed in the journal at info priority, so priority filtering in `journalctl` was blind to Tuwunel's warnings and errors. Events are now submitted natively with their severity, target, and source location, under a `log_journald` option that defaults on. Shipped by @byteflavour in (#534) and extended by @x86pup, whose layer streams the console-formatted line into each entry and records tracing fields under an `F_` prefix, so the journal can be filtered by room or event id.

- **systemd socket activation and configuration reload**, courtesy of @x86pup. A socket unit ships for activation, `systemctl reload` is wired into the Debian and Red Hat units, the Arch unit refreshes its systemd credentials on reload, and a reload now replays the startup command line, so a server started with `-c` or `-O` rebuilds the configuration it actually had. TURN and registration secrets are re-read at each use, so rotating either file no longer needs a restart, and an in-place restart strips the activation variables it can no longer honor. Documented alongside.

- **Android builds work**, courtesy of @x86pup. The TLS segment is aligned for bionic's loader, SMTP and client TLS verify against bundled webpki roots, `dns_servers` becomes a requirement where the system resolver would need a JVM, and jemalloc and rust-rocksdb are advanced for the libgcc fix.

- **LDAP login hardening** from @x86pup. Filter and bind-DN metacharacters in the login localpart are escaped, an empty password is rejected before the bind is attempted, accounts that did not originate in LDAP and deactivated accounts are refused, and error handling, timeouts, and password-file IO are tightened.

- **Appservices** gain third-party network lookup forwarding: the client `/thirdparty/*` endpoints fan out to every registered bridge, carrying protocol metadata verbatim. One-time keys are proxied ahead of the fallback key and appservice device keys are overlaid across client and federation paths (MSC3983, MSC3984). A stalled transaction is retried after a ping, and appservice users are excluded from user-directory search.

- **Push badges carry the account-wide unread count**, derived across joined rooms at delivery, refreshed on demand, recovered from the queue at startup, and hooked into the read paths that change it. An explicit zero is preserved and only a true opt-out is honored. Reported and diagnosed by @lhjt.

- **State resolution** gets a reworked conflicted-subgraph walker, an event-ID sha256 codec with a matching map hasher, mainline positions read from a map, an auth difference hashed by event ID, and short hashes persisted with their state diffs.

- A **Matrix Authentication Service provider guide** documents the integration end to end, with a startup warning when `mas_secret` is set but no MAS identity provider is configured.

- @dasha-uwu adds a `rooms info` command, `users set-profile-key`, and a historical filter on `admin query users iter-users`, and removes the abandoned MSC4373 EDU-type preference endpoint from the unstable surface.

- The room directory admin commands accept a room alias and surface it in public-rooms responses ahead of the canonical alias, while publishing an unknown room is refused. Contributed by @x86pup, who also falls URL previews back to `twitter:` card tags when the `og:` values are empty, and newline-delimits `list-backups` output.

- Thank you @Xerusion for documenting Traefik root-domain delegation in (#529), and @byteflavour for raising the btrfs WAL fallocate disk-usage footgun in (#535), now documented.

- Compliance status pages for the Complement test families join the documentation, `admin query raw flush` forces a RocksDB memtable flush, room-scoped policy recovery lands as an admin command, database writes go through atomic batches that watcher notifications take as their single source of truth, the runtime can report tokio scheduler latency histograms, and the pool-thread and cache defaults are relaxed.

- Admin `rebuild-relation-index` and `rebuild-thread-index` move from `!admin server` to the debug suite; existing invocations need the new path.

### Bug Fixes

- Thank you @lhjt for catching in (#515) that appservice-authenticated client requests inferred presence and activity for the user they act as; the exclusion landed in (#517), with activity context passed through the ping arguments behind it.

- Reopening the database in-process, which a module reload does, restored the configured backup a second time over everything written since; the restore is now claimed rather than read (cd7100398). Separately, the `-O` loop that sets the restore option ran after the check meant to refuse it, so a database could roll back on every start (8e3e3393d). Both repaired by @x86pup. Sincere apologies to anyone who restored a 1.8.2 backup more than they meant to.

- Public read receipts are monotonic again. A re-posted receipt took a fresh stream position and re-sent the EDU to appservices and over federation; the stored position now gates the write. Reported by @lhjt in (#516), who shipped the first fix in (#518). Private read markers are monotonic too, and the sender's own send is marked read without publishing a receipt for it. The deletion sweep is bounded by the encoded room prefix, which had let it cross into a sibling room whose id merely shared a prefix (7190ab8fd).

- Soft-failed events are handled correctly in three places. A withheld membership or power-level change is kept out of current state instead of being applied locally, which is the outcome withholding it exists to prevent (cb03606b8). The rejection marker used to reject every later attempt, so an event withheld over a policy refusal could never return once that refusal lapsed; it now expires on the shared upgrade backoff and reports to the origin as withheld rather than failed (5b1fc2d93). Policy-server refusals expire after 24 hours (6651f9ebc), and a corrupt state-after room is contained rather than failing the whole `/sync` response (b565d92d8).

- Left rooms stay in sync when their cached leave state came from a sibling conduwuit-lineage server, thanks to @x86pup: a lone event object is lifted into a one-element array and anything else read as no cached state. To-device events are handled only for local active or appservice-claimed recipients, so a bridge still receives its own.

- Non-unix and BSD builds are repaired again, courtesy of @obodnikov: the signals trace import (#526), the in-place restart import (#527), the `cfg(unix)` gating that the listener refactor dropped (#528), and the `sys/limits` nix imports and page size (#536). Device major and minor conversion is fixed for the BSD builds alongside.

- Native OIDC against Matrix Authentication Service completes. MAS's policy allowlist rejects any scope past `openid` and `email`, so our unconfigured default of `openid email profile` failed every authorization. Reported by @utop-top in (#530).

- Short-id allocation is serialized, so a racing or repeated caller observes the winner's rows instead of minting a second short id for one identity; a repeated event id inside a single batch reached this deterministically (5cff8b8a5). Room search tokens are purged by shortroomid prefix, in one atomic pass (59d722457, 423fbce29).

- Keyed mutex entries are reaped on every release path. A contender that never became a guard, through cancellation or a failed `try_lock`, left its entry in the map forever (a2bac8d06).

- Federation retry wakes land uniformly across the backoff interval rather than within a fixed three-second jitter, so a cohort of destinations that failed together stops retrying together (26000244f). A stale queue wake is rejected, and the resolver's in-flight deduplication is actually awaited, so concurrent lookups for one destination share a single resolution.

- @okias reported a broken documentation link in (#522), fixed along with the packaging READMEs, which now use absolute rendered-docs links.

- @x86pup landed several more fixes: the admin console is skipped when standard input is not a terminal, appservice response-body read failures are logged, and a failure to notify an appservice of an invite returns a generic error. @dasha-uwu repaired `get_all_user_mxcs`, which left a trailing user-id record unconsumed and panicked debug builds for any user with uploaded media. Elsewhere, `admin debug` reports per-column errors instead of panicking on an invalid property name, an unchecked float conversion is guarded, a defaulted listen address that fails to bind is skipped rather than taking the whole listener down, and the RocksDB environment is held in one process-global slot so one database's shutdown cannot strand another mid-close.
