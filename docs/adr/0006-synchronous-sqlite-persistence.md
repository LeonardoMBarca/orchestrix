# ADR-0006 — Synchronous SQLite persistence with rusqlite

- **Status:** Accepted
- **Date:** 2026-10-14
- **Owners:** Orchestrix Core
- **Related issues:** OX-005; future OX-007/OX-008 host integration

## Context

OX-004 provides a synchronous, guarded domain and portable runtime observer. OX-005 must make their state durable without putting SQL in domain entities, adding an async service or treating every domain error as a transaction failure. SQLite was selected previously; its Rust access library remained open in [Technology Stack](../TECH_STACK.md).

The current increment needs short transactions, versioned migrations, validated restoration, append-only audit events, persisted command results and two-connection conflict protection. It has no Tokio daemon or dispatch loop. A single account with serial execution remains a supported configuration; database concurrency is still relevant when two local callers use the same file.

## Decision

Implement one SQLite boundary in the existing Core crate using **rusqlite 0.40.2**, with default features disabled and only `bundled` enabled. Cargo.lock fixes the complete dependency graph. The selected version uses `libsqlite3-sys 0.38.2`; the tested linked SQLite version is **3.53.2**, built with the already installed MSVC toolchain rather than requiring a system SQLite installation. [Versioned manifest](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/Cargo.toml), [build/features documentation](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/README.md).

SQL, connection configuration and migrations belong to `persistence`, outside the pure `domain` module. Do not introduce a generic repository framework, ORM, database backend interface or a crate per entity. Preserve current state plus audit events; this does not introduce full event sourcing. [Persistence contract](../contracts/persistence.md) defines the command/transaction/restoration boundary.

Use an explicit local file path, foreign-key enforcement, WAL, `synchronous=FULL`, a **250 ms busy timeout** and `BEGIN IMMEDIATE` for command/migration writes. Acquire the write transaction before reading the idempotency record and expected revision. There is no automatic unbounded retry. WAL permits concurrent readers but still has one writer; this configuration is not a claim of unrestricted write parallelism or support for a shared network filesystem. [SQLite transactions](https://www.sqlite.org/lang_transaction.html), [WAL](https://www.sqlite.org/wal.html), [PRAGMAs](https://www.sqlite.org/pragma.html).

Future async host integration must keep blocking database work outside Tokio executor tasks, using an owned database worker or a bounded blocking operation around the **whole short transaction**. Do not hold a transaction across provider execution or `.await`. `rusqlite::Connection` is `Send` and is not `Sync`; choosing it does not authorize sharing a connection concurrently without an ownership boundary. No Tokio dependency or database actor is introduced by OX-005. [Connection API](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html).

## Alternatives considered

| Option | Relevant strengths | Reason for this decision |
| --- | --- | --- |
| rusqlite 0.40.2 | Direct synchronous SQLite API, explicit transactions and bindings, optional bundled build. | Fits the current synchronous Core and keeps the access boundary small. Its actual behavior is validated with one file-backed implementation. |
| SQLx 0.9.0 | Async-facing SQLite connection, migration facilities and optional compile-time query checks. | Appropriate for an async application, but its additional async worker boundary is not needed by this increment. It remains a documented comparison rather than a second implementation. |

SQLx's SQLite connection uses a background thread and channels because the underlying SQLite API blocks. The choice therefore concerns the application's ownership and API model; it is **not** a benchmark claim that rusqlite is faster, more durable or universally simpler. [Versioned SQLx SQLite API](https://docs.rs/sqlx/0.9.0/sqlx/struct.SqliteConnection.html), [SQLx manifest](https://raw.githubusercontent.com/launchbadge/sqlx/v0.9.0/Cargo.toml).

Using a system SQLite library was not selected: `bundled` fixes the tested SQLite source with the lockfile and avoids requiring users/developers to supply a compatible library. The trade-off is compiling bundled C code and owning SQLite version/security updates. Neither extensions, SQLCipher nor build-time binding generation is enabled. rusqlite and its SQLite bindings are MIT licensed; SQLx is MIT OR Apache-2.0; SQLite itself is public domain. [rusqlite manifest](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/Cargo.toml), [SQLx manifest](https://raw.githubusercontent.com/launchbadge/sqlx/v0.9.0/Cargo.toml), [SQLite copyright](https://www.sqlite.org/copyright.html).

## Consequences

The domain remains testable without a database. A durable command can preserve a business failure and its evidence while a technical storage error rolls back the entire operation. A global store revision initially rejects stale writers conservatively; finer aggregate revisions can be considered only when a real workload requires them.

WAL creates sidecar files and still permits busy errors. `FULL` favors conservative durability over minimum write latency; injected rollback tests cannot certify recovery after power loss, storage failure or every filesystem. Application callers must handle bounded contention and uncertain responses explicitly rather than retrying external execution.

## Validation

The single [file-backed implementation](../../crates/orchestrix-core/src/persistence/mod.rs) passed temporary-file tests of migration from a populated v1 database to v2, transaction rollback, close/reopen, two writers, expected revisions and bounded busy waiting. The settings test asserts the linked SQLite version and actual foreign-key/WAL/FULL/timeout values. SQLx was compared through its versioned documentation; no second implementation or comparative performance benchmark was built. [SQLite tests](../../crates/orchestrix-core/src/persistence/tests.rs).

The separate [workflow suite](../../crates/orchestrix-core/tests/persistence_workflow.rs) passed **eight parent tests**, including full Run/Task/attempt/evidence restoration and continued delivery, negative integration/application evidence, stale artifacts/approval, quarantine/redaction and injected rollback. Its ignored helper is explicitly invoked by a passing parent test: a second process opens an Approved Task with observer/artifact history, integrates it and exits before the parent reopens and verifies the new fact. These results accept the access-library choice; [current status](../DEVELOPMENT_STATUS.md) owns the completion decision for the entire ticket and remaining milestones.

## Revisit conditions

Revisit the ownership/API choice if a measured async workload requires a different boundary, blocking work cannot be bounded, or maintenance/platform constraints outweigh this implementation. A larger schema alone is not a reason to add another access library. Changing the access library must preserve the transaction, idempotency and domain restoration contracts.
