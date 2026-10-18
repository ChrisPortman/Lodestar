# Project Manager

## Objective

A simple web tool that allows for the week to week tracking and management of projects, including
open action items, open risks, and blockers.  A project manager or team lead would use this platform
to drive frequent (e.g. weekly) project status update meetings where the project delivery team is in
attendance.

The tool should be useful in helping to maintain team focus, and momentum.

During a meeting, the team would provide updates on their assigned open actions which may or may not
include closing an action.  New actions may be created and assigned.

Open risks items will will be tracked in terms of likeliness and impact, each of which may be
updated during a meeting.

Blockers will also be tracked.  Blockers are items that block other actions or progress down certain
lines.  They may not block all progress.  Blockers are tracked in terms of whether they still exist,
and who is accountable for clearing them.

At any point, the project will be able to produce a "Project on a Page" report, that includes
summary tables of each of these things, as well as an overall project health rating.

## Non-Goals

This project is not intended to replace more fully featured project management tools such as
Microsoft Project or Jira.  It is not intended to implement any specific project methodology (Agile,
PMBOK, Prince2 etc).  At this stage it is not intended to provide start to finish project planning
tools such as Gantt Charts, resource allocation and planning etc.

## Longer Term Goals

This section is to inform early architectural decision, but not to form MVP scope.  No significant
ground work should be done in the persuit of items derived from this section. These items may or may
not eventuate.

* Integrations with other systems may be relevant e.g. linking actions with ticket
systems (e.g. Github Issues, Gitlab Issues, Jira etc).
* Project health tracking and calculating based action/blocker staleness.
* Other project planning functions (methodology agnostic)

## Technical Implementation Decisions

The solution shall be a server client architecture with an REST-full API first backend and a SPA
front end.

The backend will be a Rust application.  It will use axum as the web framework, Tokio for the async
runtime.  It will be configured by configuration file OR environment variable or both.  It should
use a "ports and adapters" architecture.  Data will be stored in a RDMS.

The frontend will use VueJS, tailwind.

The frontend artifacts will be "compiled into" and served from the backend binary.  E.g.
`include_bytes!(...)`.

Administrators will be able to deploy as:

* A single binary onto a Linux host and manage with a systemd unit.
* Docker with an OCI image
* Kubernetes using a Helm chart

For MVP, a single binary output that is executable on a linux desktop is sufficient.

For MVP, the RDMS implementation will be SQLite, but infuture will support others such as Postgres,
MySQL etc.

No authentication is required for the MVP, but will be added probably in a second phase.  Followed
by per project authorisation and basic project permissions managed by a project leader.

All code should be assessed for security and quality and architectural sanity.

