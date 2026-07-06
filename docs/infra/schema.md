# Database Schema

> **Complete data model for [PROJECT NAME].**
> This is the authoritative reference for all collections, tables, and their shapes.
> Feature docs reference this file for entity definitions — the canonical type lives here.
> When this file exceeds ~500 lines, split into per-domain files (e.g. `docs/infra/schema/auth.md`).
> Keep this file as an index pointing to the domain files.
> Last updated: [YYYY-MM-DD]

---

## Database Overview

**Type:** [e.g. Firestore / PostgreSQL / MongoDB / SQLite]
**Structure:** [e.g. Document store organized by collection / Relational with normalized tables]

**Collections / Tables:**
- [`[name]`](#[name]) — [one line on what it stores]
- [`[name]`](#[name]) — [one line on what it stores]
- [`[name]`](#[name]) — [one line on what it stores]

---

## [Collection/Table Name]

**Owned by feature:** [`[feature-name]`](../features/[feature-name].md)

```typescript
// Use your project language's type syntax
type [EntityName] = {
  // Identity
  id: string                    // Auto-generated UUID / Firestore doc ID / etc.

  // Core fields
  [field]: [type]               // [What this is for]
  [field]: [type]               // [What this is for]
  [field]: [type]               // [Valid values: X, Y, Z]

  // Relations
  [foreignKeyField]: string     // References [OtherEntity].id
  [foreignKeyField]: string     // References [OtherEntity].id

  // Metadata
  createdAt: Date
  updatedAt: Date
  deletedAt?: Date              // Present if soft-deleted
}
```

**Indexes:**
- `[field]` — [why this index exists, what queries it supports]
- `[field1, field2]` — [compound index for X query]

**Notes:**
- [Any non-obvious things about this collection — e.g. "Documents are never hard-deleted, always soft-deleted via deletedAt"]
- [e.g. "Max document size is 1MB — large arrays are stored in subcollections"]

---

## [Collection/Table Name]

**Owned by feature:** [`[feature-name]`](../features/[feature-name].md)

```typescript
type [EntityName] = {
  id: string
  [field]: [type]               // [description]
  [field]: [type]               // [description]
  createdAt: Date
  updatedAt: Date
}
```

**Indexes:**
- [index + reasoning]

**Notes:**
- [Non-obvious notes]

---

## Relationships

> How entities relate to each other across collections/tables.

```
[EntityA] 1──── N [EntityB]
  id              [entityAId]

[EntityB] 1──── N [EntityC]
  id              [entityBId]

[EntityA] N──── N [EntityD]
  (via [junction_table/subcollection])
```

---

## Soft Delete Pattern

> [If this project uses soft deletes, document the pattern here]

All deletes in this system are soft. An entity is considered deleted when `deletedAt` is set. All queries must include a filter to exclude deleted records. The repository layer handles this automatically — never write raw queries that don't filter on `deletedAt`.

---

## Migration History

> [If using a relational database with migrations]

| Migration | Date | What changed |
|---|---|---|
| `[migration file name]` | [YYYY-MM-DD] | [What was added/changed/removed] |
| `[migration file name]` | [YYYY-MM-DD] | [What was added/changed/removed] |
