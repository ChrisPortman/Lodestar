# Lodestar

Week-to-week tracking of a project's open Actions, Risks and Blockers, and the status Meetings that review them.

## Language

**Project**:
A body of work whose Actions, Risks and Blockers are tracked together and reported on. A finished Project is archived, not deleted.

**Project Code**:
A short, unique, unchangeable label for a Project, derived from its name (e.g. WEBR), used to tell References apart across Projects.
_Avoid_: Key, slug, prefix

**User Account** (future):
A way of signing in to the tool. Each User Account is linked to exactly one Person, and holds that Person's permissions, such as the Project Leader role.
_Avoid_: User, login, Person

**Acting Person**:
The Person on whose behalf changes are being made. Before User Accounts exist, this is whichever Person the user selected or created. Afterwards, it is the Person linked to the signed-in User Account.
_Avoid_: Current user, logged-in user

**Member**:
A Person who has been added to a Project. Only Members can be assigned Actions or be accountable for that Project's Blockers.
_Avoid_: Participant, team member

**Project Leader**:
The Member shown as leading a Project. By default this is the Person who created the Project. A Project can have no Project Leader in the MVP. Once permissions exist, a Project Leader will be required, because they manage permissions within the Project.
_Avoid_: Project manager, team lead, owner

**Person**:
A real individual, as the tool knows them: someone who can be a Member, be assigned Actions, or be accountable for Blockers. One Person can be in many Projects. A Person may be linked to a User Account, but doesn't have to be. People are never deleted.
_Avoid_: User, team member, resource

**Action**:
A piece of work assigned to at most one Member, which is either open or closed. An open Action with no assignee needs attention.
_Avoid_: Task, action item, todo

**Risk**:
Something that might harm the Project, rated by likelihood and impact, which is either open or closed.
_Avoid_: Issue

**Blocker**:
Something currently stopping some progress on the Project. At most one Member is accountable for clearing it, and it may optionally be linked to the Actions it blocks.
_Avoid_: Impediment, dependency

**Health Rating**:
An overall judgement of the Project's state, either Red, Amber or Green, with an optional comment.
_Avoid_: Status, RAG

**Project on a Page**:
A report summarising a Project's open Actions, Risks and Blockers, plus its Health Rating.
_Avoid_: Status report, dashboard

**Project Export**:
A complete, downloadable copy of a Project: its Members, every Action, Risk and Blocker (closed ones included), and the People they mention, optionally with its Change History. It is an archive, not a report, and cannot be imported back.
_Avoid_: Backup, dump, report

**Reference**:
The short, human-readable label shown for an Action, Risk or Blocker, made of a type prefix and a number (e.g. A-001, R-001, B-001). It is numbered within its Project and never reused.
_Avoid_: ID, key, number

**Comment**:
A dated, plain-text note written by a Member about an Action, Risk, Blocker, Meeting or the Project as a whole. Only its author can edit or delete it. A deleted Comment is kept and shown as deleted by its author. Comment changes are recorded in Change History as changes to the Comment, not to the thing it is about.
_Avoid_: Note, update, remark

**Meeting**:
A time-boxed session, run by a Project Leader, in which a Project's Actions, Risks and Blockers are reviewed with its attendees. Its leader is always one of its attendees.
_Avoid_: Session, standup, review

**Meeting Minutes**:
The fixed record of a Meeting: when it happened, who attended, and the Changes and Comments made to the Project's Actions, Risks and Blockers during it. Produced once, when the Meeting ends, and never regenerated.
_Avoid_: Notes, summary, report

**Change History**:
The recorded sequence of every change made to an Action, Risk, Blocker or Health Rating. Each entry records the Acting Person who made the change. It is used for reporting only, never to rebuild current state.
_Avoid_: Audit log, updates
