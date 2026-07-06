# API Contracts

> **Complete API reference for [PROJECT NAME].**
> Feature docs link here for full endpoint details.
> This is the contract — it defines what the API promises to clients.
> Last updated: [YYYY-MM-DD]

---

## Global Conventions

**Base URL:** `[https://api.yourproject.com]` / `[http://localhost:PORT]`
**Auth:** [e.g. All endpoints require `Authorization: Bearer <token>` header unless marked public]
**Content-Type:** `application/json` for all requests and responses
**Dates:** ISO 8601 format — `YYYY-MM-DDTHH:mm:ssZ`

**Standard error response:**
```json
{
  "error": "Human readable message",
  "code": "MACHINE_READABLE_CODE",
  "details": {} // Optional, for validation errors
}
```

**Standard success response (single entity):**
```json
{
  "data": { ... }
}
```

**Standard success response (list):**
```json
{
  "data": [ ... ],
  "meta": {
    "total": 100,
    "page": 1,
    "pageSize": 20
  }
}
```

---

## Endpoints

---

### [Feature Name] Endpoints

#### `GET /[path]`

**Description:** [What this returns]
**Auth:** Required / Public
**Feature doc:** [`docs/features/[feature].md`](../features/[feature].md)

**Query parameters:**
| Param | Type | Required | Description |
|---|---|---|---|
| `[param]` | string | No | [what it does] |
| `[param]` | number | No | [what it does, default value] |

**Response `200`:**
```json
{
  "data": {
    "[field]": "[type — description]",
    "[field]": "[type — description]"
  }
}
```

**Errors:**
| Code | HTTP | When |
|---|---|---|
| `[ERROR_CODE]` | 404 | [When this happens] |
| `[ERROR_CODE]` | 403 | [When this happens] |

---

#### `POST /[path]`

**Description:** [What this creates/does]
**Auth:** Required / Public
**Feature doc:** [`docs/features/[feature].md`](../features/[feature].md)

**Request body:**
```json
{
  "[field]": "[type] — required — [description]",
  "[field]": "[type] — optional — [description]",
  "[field]": "[type] — required — [valid values: X | Y | Z]"
}
```

**Response `201`:**
```json
{
  "data": {
    "id": "string",
    "[field]": "[value]"
  }
}
```

**Errors:**
| Code | HTTP | When |
|---|---|---|
| `VALIDATION_ERROR` | 400 | Request body fails validation |
| `[ERROR_CODE]` | 409 | [Conflict scenario] |

---

#### `PUT /[path]/:id`

**Description:** [What this updates]
**Auth:** Required
**Feature doc:** [`docs/features/[feature].md`](../features/[feature].md)

**Path params:**
- `id` — [EntityName] ID

**Request body:**
```json
{
  "[field]": "[type] — optional — [description]"
}
```

**Response `200`:**
```json
{
  "data": { ... }
}
```

**Errors:**
| Code | HTTP | When |
|---|---|---|
| `[ERROR_CODE]` | 404 | Resource not found |
| `[ERROR_CODE]` | 403 | No permission to update |

---

#### `DELETE /[path]/:id`

**Description:** [What this deletes — soft or hard]
**Auth:** Required
**Feature doc:** [`docs/features/[feature].md`](../features/[feature].md)

**Response `200`:**
```json
{
  "data": { "id": "string", "deleted": true }
}
```

---

### [Next Feature Name] Endpoints

<!-- Repeat pattern for each feature -->

---

## Webhooks

> [If this project sends webhooks, document them here]

**Webhook delivery:** [How webhooks are sent — e.g. POST to registered URL, signed with HMAC]

### `[event.name]`

**Triggered when:** [Description]

**Payload:**
```json
{
  "event": "[event.name]",
  "timestamp": "ISO8601",
  "data": {
    "[field]": "[description]"
  }
}
```

---

## Rate Limits

> [Document rate limits if applicable]

| Endpoint group | Limit | Window |
|---|---|---|
| All authenticated endpoints | [N] requests | per minute |
| [Specific endpoint] | [N] requests | per hour |
