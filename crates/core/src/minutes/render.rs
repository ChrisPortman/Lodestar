//! Renders stored [`Minutes`] as plain text and as email-safe HTML (#83).
//!
//! One walk over the Minutes ([`render`]) drives both formats through
//! [`Writer`], so they always agree on layout and wording (#68, as amended by
//! #75). Times are shown in UTC: the server renders without a viewer's zone,
//! and the output must be deterministic.

use std::fmt::Write as _;

use super::{Minutes, NewItem, Section, UpdatedItem};

/// The Minutes as plain text, for pasting where formatting is lost.
pub fn to_text(minutes: &Minutes) -> String {
    let mut w = Text(String::new());
    render(minutes, &mut w);
    w.0
}

/// The Minutes as an HTML fragment with inline styles only, for email and
/// docs. All user text is escaped.
pub fn to_html(minutes: &Minutes) -> String {
    let mut w = Html(String::from(
        r#"<div style="font-family:Arial,Helvetica,sans-serif;font-size:14px;line-height:1.4;color:#1a1a1a">"#,
    ));
    render(minutes, &mut w);
    w.0.push_str("</div>\n");
    w.0
}

/// The parts of the Minutes layout; each format decides how they look.
trait Writer {
    fn heading(&mut self, title: &str, when: &str, leader: &str, attendees: &[String]);
    fn section(&mut self, name: &str);
    fn subsection(&mut self, name: &str);
    fn new_items(&mut self, items: &[NewItem]);
    fn updated_item(&mut self, item: &UpdatedItem);
    fn meeting_comments(&mut self, comments: &[String]);
}

fn render(m: &Minutes, w: &mut impl Writer) {
    let when = format!(
        "{}, {}–{} UTC ({} min)",
        m.start.format("%a %-d %b %Y"),
        m.start.format("%H:%M"),
        m.end.format("%H:%M"),
        m.duration_minutes
    );
    w.heading(&m.project_name, &when, &m.leader, &m.attendees);
    for (name, section) in [
        ("Action", &m.actions),
        ("Risk", &m.risks),
        ("Blocker", &m.blockers),
    ] {
        let Some(Section { new, updated }) = section else {
            continue;
        };
        if new.is_empty() && updated.is_empty() {
            continue;
        }
        w.section(&format!("{name}s"));
        if !new.is_empty() {
            w.subsection(&format!("New {name}s"));
            w.new_items(new);
        }
        if !updated.is_empty() {
            w.subsection(&format!("Updated {name}s"));
            for item in updated {
                w.updated_item(item);
            }
        }
    }
    if !m.meeting_comments.is_empty() {
        w.section("Meeting Comments");
        w.meeting_comments(&m.meeting_comments);
    }
}

/// `Title (Assigned to Sam Lee, Due 31 Oct)`, before escaping.
fn new_item_tail(item: &NewItem) -> String {
    if item.state.is_empty() {
        item.title.clone()
    } else {
        format!("{} ({})", item.title, item.state.join(", "))
    }
}

/// Writing to a `String` cannot fail, so `write!` results are ignored below.
struct Text(String);

impl Writer for Text {
    fn heading(&mut self, title: &str, when: &str, leader: &str, attendees: &[String]) {
        let _ = writeln!(self.0, "{title}\n{when}");
        let _ = writeln!(
            self.0,
            "Led by {leader}. Attendees: {}",
            attendees.join(", ")
        );
    }

    fn section(&mut self, name: &str) {
        let _ = writeln!(self.0, "\n{}", name.to_uppercase());
    }

    fn subsection(&mut self, name: &str) {
        let _ = writeln!(self.0, "\n{name}");
    }

    fn new_items(&mut self, items: &[NewItem]) {
        for item in items {
            let _ = writeln!(self.0, "  {}: {}", item.reference, new_item_tail(item));
            for c in &item.comments {
                let _ = writeln!(self.0, "    \"{c}\"");
            }
        }
    }

    fn updated_item(&mut self, item: &UpdatedItem) {
        let _ = writeln!(self.0, "  {}: {}", item.reference, item.title);
        for change in &item.changes {
            let _ = writeln!(self.0, "    - {change}");
        }
        for c in &item.comments {
            let _ = writeln!(self.0, "    - \"{c}\"");
        }
    }

    fn meeting_comments(&mut self, comments: &[String]) {
        for c in comments {
            let _ = writeln!(self.0, "  \"{c}\"");
        }
    }
}

const LIST: &str = r#"<ul style="margin:0 0 8px;padding-left:20px">"#;

/// Escapes text for HTML element content and attribute values.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

struct Html(String);

impl Html {
    fn comment(&mut self, c: &str) {
        let _ = write!(self.0, "<li><i>{}</i></li>", esc(c));
    }
}

impl Writer for Html {
    fn heading(&mut self, title: &str, when: &str, leader: &str, attendees: &[String]) {
        let attendees: Vec<String> = attendees.iter().map(|a| esc(a)).collect();
        let _ = write!(
            self.0,
            concat!(
                r#"<h1 style="font-size:20px;margin:0 0 4px">{}</h1>"#,
                r#"<p style="margin:0 0 4px;color:#555555">{}</p>"#,
                r#"<p style="margin:0 0 12px">Led by <b>{}</b>. Attendees: {}</p>"#
            ),
            esc(title),
            esc(when),
            esc(leader),
            attendees.join(", ")
        );
    }

    fn section(&mut self, name: &str) {
        let _ = write!(
            self.0,
            r#"<h2 style="font-size:16px;margin:16px 0 4px">{}</h2>"#,
            esc(name)
        );
    }

    fn subsection(&mut self, name: &str) {
        let _ = write!(
            self.0,
            r#"<h3 style="font-size:14px;margin:8px 0 4px">{}</h3>"#,
            esc(name)
        );
    }

    fn new_items(&mut self, items: &[NewItem]) {
        self.0.push_str(LIST);
        for item in items {
            let _ = write!(
                self.0,
                "<li><b>{}</b>: {}",
                esc(&item.reference),
                esc(&new_item_tail(item))
            );
            if !item.comments.is_empty() {
                self.0
                    .push_str(r#"<ul style="margin:0;padding-left:20px">"#);
                for c in &item.comments {
                    self.comment(c);
                }
                self.0.push_str("</ul>");
            }
            self.0.push_str("</li>");
        }
        self.0.push_str("</ul>");
    }

    fn updated_item(&mut self, item: &UpdatedItem) {
        let _ = write!(
            self.0,
            r#"<h4 style="font-size:14px;font-weight:normal;margin:4px 0"><b>{}</b>: {}</h4>"#,
            esc(&item.reference),
            esc(&item.title)
        );
        if item.changes.is_empty() && item.comments.is_empty() {
            return;
        }
        self.0.push_str(LIST);
        for change in &item.changes {
            let _ = write!(self.0, "<li>{}</li>", esc(change));
        }
        for c in &item.comments {
            self.comment(c);
        }
        self.0.push_str("</ul>");
    }

    fn meeting_comments(&mut self, comments: &[String]) {
        self.0.push_str(LIST);
        for c in comments {
            self.comment(c);
        }
        self.0.push_str("</ul>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minutes::{MINUTES_SCHEMA, NewItem, Section, UpdatedItem};
    use chrono::{TimeZone, Utc};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    /// Every section: new and updated Actions, updated Risks, new Blockers,
    /// and Meeting Comments.
    fn fixture() -> Minutes {
        Minutes {
            schema: MINUTES_SCHEMA,
            project_name: "Office move".into(),
            start: Utc.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 10, 21, 10, 15, 0).unwrap(),
            duration_minutes: 75,
            leader: "Ada Lovelace".into(),
            attendees: s(&["Ada Lovelace", "Sam Lee"]),
            actions: Some(Section {
                new: vec![NewItem {
                    reference: "A-012".into(),
                    title: "Book removalists for stage 2".into(),
                    state: s(&["Assigned to Sam Lee", "Due 31 Oct"]),
                    comments: s(&["Two quotes so far"]),
                }],
                updated: vec![UpdatedItem {
                    reference: "A-009".into(),
                    title: "Draft comms plan".into(),
                    changes: s(&["Due 24 Oct → 7 Nov", "Reassigned to Sam Lee"]),
                    comments: s(&["Waiting on legal sign-off"]),
                }],
            }),
            risks: Some(Section {
                new: vec![],
                updated: vec![UpdatedItem {
                    reference: "R-003".into(),
                    title: "Lease overlap".into(),
                    changes: s(&["Closed"]),
                    comments: vec![],
                }],
            }),
            blockers: Some(Section {
                new: vec![NewItem {
                    reference: "B-002".into(),
                    title: "Data-centre access".into(),
                    state: vec![],
                    comments: vec![],
                }],
                updated: vec![],
            }),
            meeting_comments: s(&["Launch moved", "Good progress"]),
        }
    }

    #[test]
    fn plain_text_covers_every_section() {
        let expected = "\
Office move
Wed 21 Oct 2026, 09:00–10:15 UTC (75 min)
Led by Ada Lovelace. Attendees: Ada Lovelace, Sam Lee

ACTIONS

New Actions
  A-012: Book removalists for stage 2 (Assigned to Sam Lee, Due 31 Oct)
    \"Two quotes so far\"

Updated Actions
  A-009: Draft comms plan
    - Due 24 Oct → 7 Nov
    - Reassigned to Sam Lee
    - \"Waiting on legal sign-off\"

RISKS

Updated Risks
  R-003: Lease overlap
    - Closed

BLOCKERS

New Blockers
  B-002: Data-centre access

MEETING COMMENTS
  \"Launch moved\"
  \"Good progress\"
";
        assert_eq!(to_text(&fixture()), expected);
    }

    #[test]
    fn html_covers_every_section_with_inline_styles() {
        let expected = concat!(
            r#"<div style="font-family:Arial,Helvetica,sans-serif;font-size:14px;line-height:1.4;color:#1a1a1a">"#,
            r#"<h1 style="font-size:20px;margin:0 0 4px">Office move</h1>"#,
            r#"<p style="margin:0 0 4px;color:#555555">Wed 21 Oct 2026, 09:00–10:15 UTC (75 min)</p>"#,
            r#"<p style="margin:0 0 12px">Led by <b>Ada Lovelace</b>. Attendees: Ada Lovelace, Sam Lee</p>"#,
            r#"<h2 style="font-size:16px;margin:16px 0 4px">Actions</h2>"#,
            r#"<h3 style="font-size:14px;margin:8px 0 4px">New Actions</h3>"#,
            r#"<ul style="margin:0 0 8px;padding-left:20px">"#,
            r#"<li><b>A-012</b>: Book removalists for stage 2 (Assigned to Sam Lee, Due 31 Oct)"#,
            r#"<ul style="margin:0;padding-left:20px"><li><i>Two quotes so far</i></li></ul></li>"#,
            r#"</ul>"#,
            r#"<h3 style="font-size:14px;margin:8px 0 4px">Updated Actions</h3>"#,
            r#"<h4 style="font-size:14px;font-weight:normal;margin:4px 0"><b>A-009</b>: Draft comms plan</h4>"#,
            r#"<ul style="margin:0 0 8px;padding-left:20px"><li>Due 24 Oct → 7 Nov</li><li>Reassigned to Sam Lee</li><li><i>Waiting on legal sign-off</i></li></ul>"#,
            r#"<h2 style="font-size:16px;margin:16px 0 4px">Risks</h2>"#,
            r#"<h3 style="font-size:14px;margin:8px 0 4px">Updated Risks</h3>"#,
            r#"<h4 style="font-size:14px;font-weight:normal;margin:4px 0"><b>R-003</b>: Lease overlap</h4>"#,
            r#"<ul style="margin:0 0 8px;padding-left:20px"><li>Closed</li></ul>"#,
            r#"<h2 style="font-size:16px;margin:16px 0 4px">Blockers</h2>"#,
            r#"<h3 style="font-size:14px;margin:8px 0 4px">New Blockers</h3>"#,
            r#"<ul style="margin:0 0 8px;padding-left:20px"><li><b>B-002</b>: Data-centre access</li></ul>"#,
            r#"<h2 style="font-size:16px;margin:16px 0 4px">Meeting Comments</h2>"#,
            r#"<ul style="margin:0 0 8px;padding-left:20px"><li><i>Launch moved</i></li><li><i>Good progress</i></li></ul>"#,
            "</div>\n",
        );
        assert_eq!(to_html(&fixture()), expected);
    }

    #[test]
    fn html_escapes_all_user_text() {
        let evil = r#"<script>alert("x&y")</script>'"#;
        let escaped = "&lt;script&gt;alert(&quot;x&amp;y&quot;)&lt;/script&gt;&#39;";
        let mut m = fixture();
        m.project_name = evil.into();
        m.leader = evil.into();
        m.attendees = vec![evil.into()];
        let actions = m.actions.as_mut().unwrap();
        actions.new[0].reference = evil.into();
        actions.new[0].title = evil.into();
        actions.new[0].state = vec![evil.into()];
        actions.new[0].comments = vec![evil.into()];
        actions.updated[0].title = evil.into();
        actions.updated[0].changes = vec![evil.into()];
        actions.updated[0].comments = vec![evil.into()];
        m.meeting_comments = vec![evil.into()];

        let html = to_html(&m);
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("\"x"), "{html}");
        assert_eq!(html.matches(escaped).count(), 11, "{html}");
    }

    #[test]
    fn empty_minutes_render_only_the_heading() {
        let mut m = fixture();
        m.actions = None;
        m.risks = None;
        m.blockers = None;
        m.meeting_comments.clear();
        assert_eq!(
            to_text(&m),
            "Office move\nWed 21 Oct 2026, 09:00–10:15 UTC (75 min)\n\
             Led by Ada Lovelace. Attendees: Ada Lovelace, Sam Lee\n"
        );
    }
}
