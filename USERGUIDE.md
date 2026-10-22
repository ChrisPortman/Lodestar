# Lodestar User Guide

This guide is for the people who use Lodestar day to day: **Project Leaders**, who run projects and their status Meetings, and **Members**, who own the work being tracked. If you're installing or running Lodestar for a team, see [docs/OPERATIONS.md](docs/OPERATIONS.md) instead.

Lodestar helps a team keep track of a project's open **Actions**, **Risks** and **Blockers** and review them in regular status **Meetings**. It also helps Project Leaders report the project's status to managers and stakeholders. [GLOSSARY.md](GLOSSARY.md) defines each term precisely.

## Contents

1. [Getting started](#getting-started)
2. [Projects](#projects)
3. [Actions, Blockers and Risks](#actions-blockers-and-risks)
4. [Comments](#comments)
5. [Health Rating](#health-rating)
6. [Project on a Page](#project-on-a-page)
7. [Members and the Project Leader](#members-and-the-project-leader)
8. [Running a Meeting (Project Leaders)](#running-a-meeting-project-leaders)
9. [Past Meetings and Minutes](#past-meetings-and-minutes)
10. [Exporting a Project](#exporting-a-project)
11. [People](#people)
12. [Quick reference](#quick-reference)

## Getting started

### Choosing who you are

When you first open Lodestar it asks **Who are you?**

- If your name is listed, click it.
- If not, type your name in the box and click **I'm new**.

The person you pick is your **Acting Person**. Every change you make is recorded against them. The header shows **Acting as _your name_**.

> Lodestar doesn't have sign-in yet, so it trusts whoever you say you are. Always check that the header shows your own name before making changes, especially on a shared computer. To switch, use the drop-down next to **Acting as** in the header. Choose **New Person…** there to add someone who isn't listed.

### Finding your way around

The header has links to **Projects** and **People**, and a **Dark mode** button to switch between light and dark themes.

## Projects

The **Projects** page lists every project. Each card shows:

- the **Project Code** (e.g. `WEBR`) and name,
- the description,
- the current [Health Rating](#health-rating),
- the **Leader**, or _none_.

A card has a red edge if the project is rated Red, and an amber edge if it's rated Amber or has no Project Leader.

### Creating a project

1. Click **New Project**.
2. Enter a **Name**. Lodestar suggests a **Project Code** from it. You can change the suggestion: it must start with an uppercase letter, followed by 1 to 9 uppercase letters or digits.
3. Optionally add a **Description**.
4. Click **Create**.

You become the project's first Member and its Project Leader.

> **The Project Code can't be changed later.** It's part of every Reference in the project, so choose carefully.

### Editing, archiving and unarchiving

On a project's card:

- **Edit** changes the name and description.
- **Archive** hides a finished project. Projects are never deleted.
- Tick **Show archived** to see archived projects again, and **Unarchive** to bring one back.

Archived projects are read-only.

## Actions, Blockers and Risks

Open a project to see its panels: **Actions**, then **Blockers**, then **Risks**, then **Project Comments**.

Each item has a **Reference** such as `A-001` (Action), `B-002` (Blocker) or `R-003` (Risk). References are numbered within the project and never reused, so they're a reliable way to talk about an item in a Meeting or an email.

### Working with any item

- **+ Add** creates a new item. Type a description and click **Add**.
- **Edit** changes an item's description. In the text box, **Ctrl+Enter** saves and **Esc** cancels.
- If someone else changes the description while you're editing, Lodestar warns you and shows the new text so you don't overwrite it unknowingly.
- Closed (or cleared) items are tucked away under a collapsible **Closed** (or **Cleared**) heading under the open items. Use **Reopen** to bring one back.

Each panel has controls for sorting and filtering:

- **Sort** by Reference and other fields shown below.
- **Show** open items, closed items, or both.
- For Actions and Blockers, filter by person, or click **Unassigned only** to find work nobody owns.

### Actions

An Action is a piece of work. Each one has:

- an **Assignee**: one Member, or _Unassigned_. Choose **New Person…** to add someone to Lodestar and the project in one step.
- an optional **Due** date.
- a **Close** button for when the work is done.

Actions can be sorted by Reference, Due date or Assignee.

An open Action's card has a **red edge** if it's overdue or blocked, and an **amber edge** if nobody is assigned. Both mean the Action needs attention.

### Blockers

A Blocker is something currently stopping progress. Each one has:

- an **Accountable** Member who is responsible for clearing it, or _Unassigned_.
- **Blocks**: the Actions it's holding up. Use **Link Action…** to link one, and the **×** next to an Action to unlink it. A linked Action shows that it's blocked.
- a **Clear** button for when it's resolved.

A Blocker with nobody accountable has an amber edge.

### Risks

A Risk is something that might harm the project. Rate each one from 1 to 5 for:

- **Likelihood**: how likely it is to happen,
- **Impact**: how bad it would be if it did.

Lodestar multiplies them to get a **Score**:

| Score   | Severity | Card edge |
| ------- | -------- | --------- |
| 15–25   | High     | Red       |
| 8–14    | Medium   | Amber     |
| 1–7     | Low      | None      |

Risks can be sorted by Reference or Score. Use **Close** when a Risk no longer applies.

## Comments

Every Action, Blocker and Risk has a **Comments** section, and the project as a whole has **Project Comments** at the bottom of the page. Use them for progress updates, decisions and context.

- Click **Comments (_n_)** to expand them. Comments that other people added since you last opened the project in this browser are marked **new**.
- Comments are plain text and show their author and when they were written.
- Only the author can edit or delete their Comment. An edited Comment shows when it was edited, and a deleted one is shown as _Comment deleted by author_.

## Health Rating

The Health Rating is the overall judgement of how the project is going, shown at the top of the project page:

- **▲ Red**: in trouble
- **● Amber**: at risk
- **✓ Green**: on track
- **— Not rated**

Click **Edit** beside it, pick a rating, optionally add a short comment explaining why, and click **Save**. The rating appears on the Projects list and on the Project on a Page.

## Project on a Page

Click **Project on a Page** at the top of a project to get a one-page status report. It's meant for Project Leaders reporting upwards, to their managers or stakeholders, who want to know where the project stands without the detail. It shows:

- the project name, Project Leader and Health Rating (with its comment),
- the project description,
- every open Action with its assignee, due date and any Blockers (overdue Actions and unassigned ones are highlighted),
- every open Blocker and Risk.

It also shows when it was generated. Use your browser's **Print** (Ctrl+P) to print it or save it as a PDF; the page is laid out for printing.

## Members and the Project Leader

Only Members of a project can be assigned Actions or be accountable for its Blockers.

To manage Members, open the project menu (the **☰** button at the top right of the project) and choose **Manage Members**. Each Member is listed with a count of their open items.

- **Add**: choose a person from the list, or **New Person…** to add someone new, then click **Add**.
- **Make leader**: makes that Member the Project Leader.
- **Remove**: removes them from the project.

A project should always have a Project Leader. If it has none, the project shows **No Project Leader**, and nobody can start a Meeting until someone is made leader.

## Running a Meeting (Project Leaders)

A Meeting is a status session in which the Project Leader walks the attendees through the project's Actions, Blockers and Risks. Lodestar records what changed and produces the Minutes for you.

### Starting

1. Open the project and click **☰ → Meetings**.
2. Click **Start a Meeting now**. Only the Project Leader sees this button.

A **Meeting** panel opens on the right with a running timer.

### While the Meeting runs

- **Here**: tick each Member who is present. You're always included as the leader.
- **Meeting Comments**: type a note and press **Enter** to record it, for example a decision or an announcement. Each is timestamped.
- Work through the panels as usual: update assignees and due dates, close Actions, clear Blockers, re-rate Risks and add Comments. Changes made during the Meeting go into the Minutes.
- **Close** tucks the panel away into a thin strip on the right edge of the screen, which shows the elapsed time and number of Comments. Click the strip to bring it back.

While your Meeting is running, the project is **locked** for everyone else: they see _Meeting in progress, led by…_ and can look but not change anything. Members can still be added and removed.

> Keep the project open in your browser. If Lodestar hasn't heard from you for about two minutes, the lock lapses and another Project Leader may take over the Meeting. If you return to a project with a Meeting still running, Lodestar asks whether to **Carry on** or **Cancel the Meeting**.

### Ending

1. Click **End Meeting…**.
2. Check the **Who attended?** list. If someone is missing, click **Mark anyone missing in Here**, tick them, and click **End Meeting…** again.
3. Click **Create Minutes**.

The Minutes open straight away. They're fixed once created and are never regenerated.

To abandon a Meeting instead, click **Cancel** and confirm with **Yes, cancel it**. Its Meeting Comments are discarded and no Minutes are created; changes you made to items stay.

If you stop being the Project Leader, or another leader takes over the Meeting, Lodestar can't create your Minutes. It shows your Meeting Comments so you can copy them somewhere safe.

## Past Meetings and Minutes

Anyone can read past Minutes from **☰ → Meetings → Past Meetings**. Each entry shows when the Meeting started, how long it ran, who led it and how many attended. Times are shown in your own time zone.

Minutes record when the Meeting happened, who led and attended it, and, for Actions, Blockers and Risks, what was new and what changed, along with the Comments made.

To share them, for example in an email or chat:

- **Copy formatted** copies them with headings and lists, ready to paste into an email.
- **Copy plain text** copies them as plain text.

Use the tabs at the top to preview either version.

## Exporting a Project

To keep a complete copy of a project, choose **☰ → Export**. Tick **Include history** to add the full Change History, then click **Export** to download it.

An export contains the project's Members and every Action, Blocker and Risk, including closed ones. It's an archive for your records, not a report, and can't be imported back into Lodestar. For a summary to share, use the [Project on a Page](#project-on-a-page).

## People

The **People** page lists everyone Lodestar knows about, across all projects. Click **Rename** to correct someone's display name. People are never deleted, so their history stays intact.

## Quick reference

| I want to…                              | Do this                                                     |
| --------------------------------------- | ----------------------------------------------------------- |
| Change who I'm acting as                | Use the drop-down next to **Acting as** in the header       |
| Start a project                         | **Projects → New Project**                                  |
| Find work nobody owns                   | Click **Unassigned only** on the Actions or Blockers panel  |
| Show that an Action is blocked          | On the Blocker, use **Link Action…**                        |
| Update the project's overall status     | **Edit** next to the Health Rating                          |
| Report status to managers/stakeholders  | **Project on a Page**, then print or save as PDF            |
| Add someone to a project                | **☰ → Manage Members → Add**                                |
| Hand the project to someone else        | **☰ → Manage Members → Make leader**                        |
| Run a status Meeting                    | **☰ → Meetings → Start a Meeting now**                      |
| Send out Minutes                        | **☰ → Meetings**, open the Meeting, **Copy formatted**      |
| Keep a full copy of a project           | **☰ → Export**                                              |
| Save or cancel while editing text       | **Ctrl+Enter** to save, **Esc** to cancel                   |
