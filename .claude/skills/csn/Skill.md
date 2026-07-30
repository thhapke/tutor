---
name: csn
description: Work with Core Schema Notation (CSN) files - create, validate, transform, and generate CSN schemas. Triggers when user mentions CSN, CDS models, or works with .csn files.
---

# Core Schema Notation (CSN) Reference

## Overview

CSN (pronounced "Season") is a JSON-based notation for compact representations of CDS models. It's similar to JSON Schema but extends it to capture full Entity-Relationship Models, suitable for generating OData/EDM, OpenAPI interfaces, and persistence models for SQL/NoSQL databases.

## Top-Level Structure

```javascript
{
  "requires": [...],      // Array of imported models (optional)
  "definitions": {...},   // Dictionary of named definitions (optional)
  "extensions": [...],    // Array of unapplied extensions (optional)
  "i18n": {...}          // Dictionary of translations (optional)
}
```

## Definitions

All definitions are entries in the `definitions` dictionary with fully qualified names as keys.

### Common Properties

- `kind` – "context", "service", "entity", "type", "action", "function", or "annotation" (optional for types)
- `type` – Base type reference
- `elements` – Dictionary of elements for structured types
- `items` – Item type definition for arrays
- `enum` – Dictionary of enum members
- `@<annotation>` – Annotations (any property prefixed with @)

### Type Definitions

#### Scalar Types

```javascript
{
  "MyString": {"type": "cds.String", "length": 100},
  "MyDecimal": {"type": "cds.Decimal", "precision": 11, "scale": 3}
}
```

**Built-in CDS Types:**

- `cds.UUID` – RFC 4122 UUIDs → NVARCHAR(36)
- `cds.Boolean` → BOOLEAN
- `cds.Integer`, `cds.Int16`, `cds.Int32`, `cds.Int64` → INTEGER/SMALLINT/BIGINT
- `cds.UInt8` → TINYINT
- `cds.Decimal(p,s)` → DECIMAL
- `cds.Double` → DOUBLE
- `cds.Date` → DATE
- `cds.Time` → TIME
- `cds.DateTime` → TIMESTAMP (sec precision)
- `cds.Timestamp` → TIMESTAMP (µs precision)
- `cds.String(length)` → NVARCHAR (default: 255)
- `cds.Binary(length)` → VARBINARY (default: 255)
- `cds.LargeBinary` → BLOB
- `cds.LargeString` → NCLOB

#### Structured Types

```javascript
{
  "Address": {
    "elements": {
      "street": {"type": "cds.String"},
      "city": {"type": "cds.String"},
      "zip": {"type": "cds.String", "length": 10}
    }
  }
}
```

Optional `includes` property lists types/aspects to include elements from.

#### Arrayed Types

```javascript
{
  "IntArray": {
    "items": {"type": "cds.Integer"}
  }
}
```

#### Enumeration Types

```javascript
{
  "Gender": {
    "enum": {
      "male": {},
      "female": {},
      "non_binary": {"val": "non-binary"}
    }
  },
  "Status": {
    "type": "cds.Integer",
    "enum": {
      "submitted": {"val": 1},
      "fulfilled": {"val": 2}
    }
  }
}
```

### Entity Definitions

```javascript
{
  "Products": {
    "kind": "entity",
    "elements": {
      "ID": {"type": "cds.Integer", "key": true},
      "title": {"type": "cds.String", "notNull": true},
      "price": {"type": "cds.Decimal", "precision": 11, "scale": 3}
    }
  }
}
```

**Element Properties:**

- `key: true` – Part of primary key
- `virtual: true` – Ignored in persistence mapping
- `notNull: true` – SQL NOT NULL constraint
- `default` – Default value/expression
- `localized: true` – Declared as localized

### View Definitions

Views are entities with queries. Two variants:

#### Full Query (CQN format)

```javascript
{
  "ProductView": {
    "kind": "entity",
    "query": {
      "SELECT": {
        "from": {"ref": ["Products"]},
        "columns": [{"ref": ["ID"]}, {"ref": ["title"]}]
      }
    }
  }
}
```

#### Simple Projection

```javascript
{
  "ProductView": {
    "kind": "entity",
    "projection": {
      "from": {"ref": ["Products"]},
      "columns": ["*"]
    }
  }
}
```

**Optional Properties:**

- `elements` – Declared signature (otherwise inferred)
- `params` – Dictionary of parameter definitions

### Associations

```javascript
{
  "Books": {
    "kind": "entity",
    "elements": {
      // Basic to-one association
      "author": {
        "type": "cds.Association",
        "target": "Authors"
      },
      // To-many association
      "reviews": {
        "type": "cds.Association",
        "target": "Reviews",
        "cardinality": {"max": "*"}
      },
      // Unmanaged with ON condition
      "publisher": {
        "type": "cds.Association",
        "target": "Publishers",
        "on": [{"ref": ["publisher", "ID"]}, "=", {"ref": ["publisher_ID"]}]
      },
      // With explicit keys
      "genre": {
        "type": "cds.Association",
        "target": "Genres",
        "keys": [
          {"ref": ["category"], "as": "cat"},
          {"ref": ["name"]}
        ]
      }
    }
  }
}
```

**Association Properties:**

- `type` – "cds.Association" or "cds.Composition"
- `target` – Target entity name
- `cardinality` – `{src?, min?, max}` (default: [0..1])
- `on` – ON condition for unmanaged associations (CQN expression)
- `keys` – Explicit target keys (CQN projection format)

### Services

```javascript
{
  "CatalogService": {
    "kind": "service"
  }
}
```

### Actions & Functions

```javascript
{
  // Unbound action (top-level)
  "CatalogService.cancelOrder": {
    "kind": "action",
    "params": {
      "orderID": {"type": "cds.Integer"},
      "reason": {"type": "cds.String"}
    },
    "returns": {
      "elements": {
        "success": {"type": "cds.Boolean"},
        "message": {"type": "cds.String"}
      }
    }
  },

  // Entity with bound function
  "Books": {
    "kind": "entity",
    "elements": {...},
    "actions": {
      "validate": {
        "kind": "function",
        "returns": {"type": "cds.Boolean"}
      }
    }
  }
}
```

**Properties:**

- `kind` – "action" or "function"
- `params` – Dictionary of parameter type definitions
- `returns` – Type definition of response

## Literals

### Standard (JSON-like)

- Booleans: `true`, `false`, `null`
- Numbers: `11`, `2.4`
- Strings: `"foo"`
- Dates/Times: `"2016-11-24"`, `"16:11Z"`, `"2016-11-24T16:11Z"`
- Records: `{"foo": value, ...}`
- Arrays: `[value, ...]`

### CSN-Specific

- Unparsed Expressions: `{"=": "foo.bar < 9"}`
- Enum Symbols: `{"#": "asc"}`

## Annotations

Annotations are properties prefixed with `@`:

```javascript
{
  "Employees": {
    "kind": "entity",
    "@title": "Mitarbeiter",
    "@readonly": true,
    "elements": {
      "firstname": {"type": "cds.String", "@title": "Vorname"},
      "surname": {"type": "cds.String", "@title": "Nachname"}
    }
  }
}
```

## Extensions (Aspects)

Extensions array contains unapplied modifications:

```javascript
{
  "extensions": [
    // Extend with named aspect
    {
      "extend": "Products",
      "includes": ["Temporal"]
    },

    // Extend with anonymous aspect (add elements)
    {
      "extend": "Products",
      "@foo": true,
      "elements": {
        "newField": {"type": "cds.String"}
      }
    },

    // Annotate existing definition
    {
      "annotate": "Products",
      "@readonly": true,
      "elements": {
        "ID": {"@label": "Product ID"}
      }
    }
  ]
}
```

**Extension Forms:**

- `{extend: <name>, includes: [<aspect>]}` – Include named aspect
- `{extend: <name>, <property>: <value>}` – Add elements/annotations
- `{annotate: <name>, <property>: <value>}` – Add/override annotations only

## Imports

```javascript
{
  "requires": ["@sap/cds/common", "./db/schema"]
}
```

Module names are absolute or relative paths (starting with `./` or `../`).

## Naming Rules

Definition names must:

- Be non-empty strings
- Not start/end with `.` or `::`
- Not contain `..` or `:::`
- Not contain `::` more than once

**Important:** All references are case-sensitive. Use qualified names exactly as defined.

## Key Differences from JSON Schema

1. **Entity-Relationship Focus** – Built-in support for entities, associations, compositions
2. **CQN Queries** – Views defined with parsed SQL-like queries
3. **Aspects/Extensions** – Mixin and extension mechanism
4. **Annotations** – First-class support with `@` prefix
5. **Services & Actions** – OData-style operations
6. **Rich Type System** – Database-oriented types (UUID, Decimal, DateTime, etc.)

## Typical Workflow

1. **Parse** CDS source files → CSN
2. **Transform** CSN (apply extensions, resolve includes)
3. **Generate** target artifacts (SQL DDL, OData EDMX, OpenAPI, etc.)

## References

- All type references use fully qualified names (e.g., `"cds.String"`)
- References are case-sensitive
- Avoid case-only differences to ensure SQL compatibility
