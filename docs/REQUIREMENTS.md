# Lodestar: Requirements (MVP)

Requirements for the MVP described in [OBJECTIVE.md](./OBJECTIVE.md). Terms in **bold** are defined in [GLOSSARY.md](../GLOSSARY.md). Decisions are recorded in [docs/adr/](./adr/).

Priority uses MoSCoW: **M** = must, **S** = should, **C** = could. Anything not listed is out of scope for the MVP.

## 1. Functional requirements

### 1.1 People and the Acting Person

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-01 | M | Anyone can create a **Person** with a display name. **People** exist across the whole system, not inside a **Project**. |
| FR-02 | M | When the UI is opened, the user must select an existing **Person** or create one. That Person becomes the **Acting Person** for the session. |
| FR-03 | M | The UI shows the current **Acting Person** and lets the user switch to a different one. |
| FR-04 | M | The **Acting Person** is held in a long-lived session cookie, so it is remembered per browser between visits (ADR-0006). |
| FR-05 | M | A **Person**'s display name (required, up to 100 characters) can be edited. People are never deleted. |

### 1.2 Projects

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-10 | M | A **Project** can be created with a name and an optional description. |
| FR-18 | M | Every description is required, trimmed, must not be empty, and can be up to 4,000 characters. A Project name is required, up to 100 characters. |
| FR-11 | M | When a Project is created, a **Project Code** is suggested from its name (FR-16). The user can edit the suggestion before saving. Once the Project is created, the code never changes, even if the Project is renamed. |
| FR-16 | M | The suggested code is an acronym of the name. Filler words (a, an, the, of, and, for, …) and non-letters are ignored. With four or more remaining words, it is the first letter of each of the first four. With fewer words, each word gives its letters in order and earlier words give the extra letters, until there are four (`Cake Eater` → `CAEA`, `Big Red Dog` → `BIRD`, `Website` → `WEBS`). If a word runs out of letters, the next word gives more. If the result is already taken, a number is added to the end (`WEBR1`, `WEBR2`, …). |
| FR-17 | M | A **Project Code** must be unique, and must start with an uppercase letter followed by 1–9 uppercase letters or digits. Suggested and edited codes are validated the same way. |
| FR-12 | M | The **Person** who creates a **Project** automatically becomes a **Member** and its **Project Leader**. |
| FR-13 | M | The **Project Leader** can be changed to any other **Member**. It is shown for display only and grants no permissions in the MVP. If the Project Leader is removed as a Member, the Project has no Project Leader until one is set, and the UI flags this. |
| FR-14 | M | A **Project** can be archived and unarchived. While a Project is archived, nothing in it can be changed (its items, Members or Health Rating), and it is hidden from the default Project list. |
| FR-15 | M | **Projects** are never deleted through the UI or the API. |

### 1.3 Members

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-20 | M | Any **Person** can be added to a **Project** as a **Member**. |
| FR-21 | M | A **Member** can be removed from a **Project**. Every open **Action** assigned to them, and every open **Blocker** they're accountable for, becomes unassigned. |
| FR-22 | M | Closed items keep their reference to a removed **Member**. A removed Person can be added back as a Member, but their old items aren't reassigned to them. |
| FR-23 | M | The UI makes open items with no assignee stand out, and lets them be filtered. |

### 1.4 Actions

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-30 | M | An **Action** has a description, an optional assignee (one **Member**), an optional due date (dates in the past are allowed), and a state of open or closed. |
| FR-31 | M | An **Action** can be created, edited, closed and reopened. |
| FR-32 | M | When an **Action** is created, it is given the next **Reference** in its Project (`A-001`, `A-002`, …). |
| FR-33 | S | An open **Action** past its due date is highlighted. |

### 1.5 Risks

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-40 | M | A **Risk** has a description, a likelihood (1–5), an impact (1–5), and a state of open or closed. |
| FR-41 | M | A **Risk**'s score is likelihood × impact. It's calculated, not stored, and used for sorting. |
| FR-42 | M | A **Risk** can be created, edited (including its likelihood and impact), closed and reopened. |
| FR-43 | M | When a **Risk** is created, it is given the next **Reference** in its Project (`R-001`, …). |
| FR-44 | M | Turning a **Risk** into a **Blocker** is done by hand: close the Risk, then create a Blocker. No automatic conversion. |

### 1.6 Blockers

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-50 | M | A **Blocker** has a description, an optional accountable **Member**, a state of open or cleared, and links to zero or more **Actions** in the same Project that it blocks. |
| FR-51 | M | A **Blocker** can be created, edited, cleared and reopened, and its links to **Actions** can be added or removed. |
| FR-52 | M | When a **Blocker** is created, it is given the next **Reference** in its Project (`B-001`, …). |
| FR-54 | M | Links between Blockers and Actions can be added or removed whatever state either item is in. A Blocker only counts as blocking while it is open. |
| FR-53 | S | An **Action** shows any open **Blockers** that block it. |

### 1.7 References

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-60 | M | **References** are numbered separately for each type and each Project, increase strictly, and are never reused. The number is zero-padded to at least 3 digits (`A-001`); past 999 it simply grows (`A-1000`). |
| FR-61 | M | **References** are display labels only. They are never used as identifiers in the API or the database (ADR-0001). |
| FR-62 | M | Wherever items from more than one Project appear together, the **Reference** is shown with the **Project Code** in front (e.g. `WEBR/A-003`). |

### 1.8 Health Rating

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-70 | M | A **Project** has a current **Health Rating** of Red, Amber or Green, with an optional comment. |
| FR-71 | M | The **Health Rating** is set by hand. No suggested or calculated value is offered in the MVP. |
| FR-72 | M | A **Project** may have no **Health Rating** yet. The UI shows this as "not rated". |

### 1.9 Change History

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-80 | M | Every change to an **Action**, **Risk**, **Blocker** or **Health Rating** (including creating, closing and reopening) is recorded in the **Change History**. Each entry includes a timestamp, the **Acting Person**, and the values before and after the change. |
| FR-81 | M | Entries in the **Change History** are never edited or deleted. |
| FR-82 | M | By default, the UI and API show only current state. The MVP doesn't need a way to view history. |
| FR-83 | M | Changes to **Members** (adding and removing), and the unassignments that removal causes, are also recorded. |

### 1.10 Meeting-focused UI

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-90 | M | The Project view shows open **Actions**, **Risks** and **Blockers** first. Closed or cleared items are kept in a separate tab or a collapsed list so they don't distract during a meeting. |
| FR-91 | M | Common meeting tasks can be done inline, without leaving the Project view: closing an Action, reassigning it, changing a Risk's likelihood or impact, clearing a Blocker, and adding a new item. |
| FR-93 | M | Descriptions can be multi-line and are shown with their line breaks. Editing a description needs an explicit Save (or Cancel). Discrete fields (assignee, due date, likelihood, impact) and Close/Clear apply straight away. |
| FR-94 | M | The meeting view is a single column with Actions, Blockers and Risks panels in that order. Members are managed in a panel that's opened on demand, not shown all the time. |
| FR-92 | S | Lists can be sorted and filtered by assignee, state and due date, and Risks also by score. |

### 1.11 Project on a Page

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-100 | M | Each **Project** has a **Project on a Page** view showing the current state: Project details, the date it was generated, **Project Leader**, **Health Rating** with its comment, and summary tables of open **Actions**, **Risks** (sorted by score) and **Blockers**. |
| FR-101 | M | The **Project on a Page** is set up for printing (print stylesheet, sensible page breaks) and prints to PDF through the browser. |
| FR-102 | C | Customising the report (sections, columns) is a possible later addition and isn't required. |

### 1.12 Data export

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-110 | S | A **Project** can be exported as JSON: its current state, including all its **Comments** (deleted ones too, with their original text), plus its **Change History**. |

### 1.13 Comments

| ID | Pri | Requirement |
|----|-----|-------------|
| FR-120 | S | A **Member** can add a **Comment** to an **Action**, **Risk** or **Blocker** (open or closed), or to the **Project** as a whole. Each Comment records its author and when it was created. |
| FR-121 | S | **Comments** are plain text, up to 4,000 characters, and can't be empty. Markdown is accepted but shown as typed, with line breaks kept. |
| FR-122 | S | Only the author can edit or delete a **Comment**, and only while they're a **Member**. An edited Comment keeps its created time and also shows `edited: <datetime>` in a subtle typeface. |
| FR-123 | S | Deleting a **Comment** can't be undone. The Comment is kept and shows as *Comment deleted by author*, and the API no longer returns its text. |
| FR-124 | S | No **Comments** can be added, edited or deleted on an archived **Project**. |
| FR-125 | S | Adding, editing and deleting a **Comment** are recorded in the **Change History** as changes to the Comment. They don't count as changes to the item or **Project** it's about. |
| FR-126 | S | **Comments** are shown with the item or **Project** they're about, in a list that's collapsed by default, oldest first. |
| FR-127 | S | A collapsed **Comment** list, and the item's row in the Project view, are marked when they hold new Comments: ones created by someone else since the user last opened that **Project** in this browser. Edits don't make a Comment new. If the browser has no record of a last visit, no Comments count as new. |

## 2. Non-functional requirements

### 2.1 Architecture and technology

| ID | Pri | Requirement |
|----|-----|-------------|
| NFR-01 | M | The backend is written in Rust, using axum and Tokio, and structured as ports and adapters. The domain logic has no dependency on HTTP or the database. |
| NFR-02 | M | Storage goes through a repository port. SQLite is the MVP adapter, and the schema and queries must not block adding Postgres or MySQL adapters later. |
| NFR-03 | M | The frontend is a Vue single-page app styled with Tailwind. Its build output is embedded in the backend binary and served from it. |
| NFR-04 | M | The MVP is delivered as one self-contained Linux executable. OCI image and Helm chart packaging come later. |
| NFR-05 | M | All entities use UUIDv7s as their primary keys and as their identifiers in the API (ADR-0001). |

### 2.2 API

| ID | Pri | Requirement |
|----|-----|-------------|
| NFR-10 | M | The REST API is served under `/api/v1`. The UI uses only this public API. |
| NFR-11 | M | The OpenAPI document is generated from the code with utoipa and served by the app. The frontend's TypeScript types are generated from it (ADR-0002). |
| NFR-12 | M | Updates use optimistic concurrency. A save based on an out-of-date version of an item is rejected (HTTP 412, using `If-Match` and `ETag`), and the UI asks the user to reload (ADR-0006). |
| NFR-13 | M | Errors use RFC 9457 problem details. The API follows the conventions in ADR-0006: flat URLs, `PUT` with every writable field, `PATCH` only for the finalise flag, no deletion, snake_case, and limit/offset pagination. |

### 2.3 Configuration and operations

| ID | Pri | Requirement |
|----|-----|-------------|
| NFR-20 | M | Configuration can come from a file, from environment variables, or both, with environment variables taking precedence. |
| NFR-21 | M | Settings include at least the bind address and port, the database location, and the log level. |
| NFR-22 | M | Database migrations run automatically on startup. |
| NFR-23 | M | Logs are structured (`tracing`) and written to stdout, at a log level you can configure. |
| NFR-24 | M | A `/api/v1/healthz` endpoint reports whether the app is running and can reach the database. |
| NFR-25 | M | Backups are the administrator's responsibility in the MVP. The documentation explains how to back up the SQLite database safely while it's in use (WAL mode). |

### 2.4 Security

| ID | Pri | Requirement |
|----|-----|-------------|
| NFR-30 | M | By default the app binds to `127.0.0.1`. Listening on other interfaces must be configured deliberately, and the docs warn that the MVP has no authentication. |
| NFR-31 | M | All input is validated at the API boundary, and all database access uses parameterised queries. |
| NFR-32 | M | Responses include security headers, including a Content-Security-Policy that blocks inline scripts and third-party sources. |
| NFR-33 | M | CI runs dependency vulnerability scans (`cargo audit`, `npm audit`) and fails on known high-severity issues. |
| NFR-34 | M | The design must leave room for authentication and per-project authorisation later: every change already records an **Acting Person**, and a **User Account** will link to exactly one **Person**. |

### 2.5 Performance and scale

| ID | Pri | Requirement |
|----|-----|-------------|
| NFR-40 | M | Target scale: one instance, up to about 50 **Projects**, about 100 **People**, a few hundred items per **Project**, and a handful of people using it at once. |
| NFR-41 | M | At that scale, on modest hardware, API responses and page changes complete within 200 ms (95th percentile). |
| NFR-42 | S | The binary starts and is ready to serve requests within 2 seconds. |

### 2.6 Usability and accessibility

| ID | Pri | Requirement |
|----|-----|-------------|
| NFR-50 | M | Supported browsers: current Chrome, Firefox, Edge and Safari on desktop. The layout must work on a laptop screen shared to a meeting display. |
| NFR-51 | C | Mobile layouts are a nice-to-have. |
| NFR-52 | S | Aim for WCAG 2.2 AA. Missing it doesn't block the MVP release. |
| NFR-53 | S | The main meeting tasks (FR-91) can be done using only the keyboard. |

### 2.7 Quality

| ID | Pri | Requirement |
|----|-----|-------------|
| NFR-60 | M | Backend CI runs `cargo fmt --check`, `cargo clippy -D warnings`, and unit plus integration tests against SQLite. |
| NFR-61 | M | Frontend CI runs lint, type checks and unit tests. |
| NFR-62 | M | CI fails if the generated OpenAPI document differs from the committed copy, so changes to the API contract always show up in review. |
| NFR-63 | M | All code is reviewed for security, quality and how well it fits the architecture. There's no fixed coverage percentage. |

## 3. Out of scope (MVP)

- Authentication, user accounts, and per-project permissions
- Modelling meetings
- Showing Change History in the UI
- Calculated or suggested Health Ratings
- Integrations with issue trackers (GitHub, GitLab, Jira)
- Gantt charts, resource planning, or any specific methodology
- Real-time collaboration and live updates
- Postgres or MySQL adapters, OCI image, Helm chart

## 4. Decisions to revisit

- **Risks** have no owner in the MVP. This may change based on user feedback.
- The time a user last viewed a **Project** (FR-127) is kept in the browser's local storage, so it's lost when local storage is cleared or another browser is used. Once **User Accounts** exist, it could be stored on the server.
- **Projects** have no reporting date. The **Project on a Page** shows when it was generated.
