//! The map's links: topics resting on topics (foundations) and topics that
//! are part of broader ones, at any depth. kizn_core::graph decides; this
//! stores links and says plainly why one is refused.

use kizn_core::graph::{Edge, layers, would_loop};
use kizn_store::LinkRow;

use crate::command::LinkedTopic;
use crate::{AppError, Ctx};

impl Ctx {
    /// Links `upper` above `lower`: "rests_on" (lower is a foundation of
    /// upper) or "part_of" (lower is a piece of upper). Refused when it
    /// would make a loop, links a topic to itself, or is already there.
    pub(crate) fn link_topics(&self, upper_id: &str, lower_id: &str, kind: &str) -> Result<(), AppError> {
        if !["rests_on", "part_of"].contains(&kind) {
            return Err(AppError::Refused(format!("there's no link called {kind:?}")));
        }
        let title = |id: &str| -> Result<String, AppError> {
            Ok(self.store.topic(id)?.ok_or_else(|| AppError::Refused("there's no such topic".into()))?.title)
        };
        let (upper, lower) = (title(upper_id)?, title(lower_id)?);
        if upper_id == lower_id {
            return Err(AppError::Refused("a topic can't rest on itself".into()));
        }
        let links = self.store.links()?;
        if links.iter().any(|l| {
            (l.upper_id == upper_id && l.lower_id == lower_id) || (l.upper_id == lower_id && l.lower_id == upper_id)
        }) {
            return Err(AppError::Refused(format!("“{upper}” and “{lower}” are already linked")));
        }
        let edges: Vec<Edge> = links.iter().map(|l| (l.upper_id.as_str(), l.lower_id.as_str())).collect();
        if would_loop(&edges, upper_id, lower_id) {
            return Err(AppError::Refused(format!(
                "that would make a loop: “{upper}” is already among the foundations of “{lower}”"
            )));
        }
        self.store.insert_link(&LinkRow {
            id: self.id(),
            upper_id: upper_id.into(),
            lower_id: lower_id.into(),
            kind: kind.into(),
            created_at: self.now(),
        })?;
        self.changed(upper_id);
        self.changed(lower_id);
        Ok(())
    }

    pub(crate) fn unlink(&self, link_id: &str) -> Result<(), AppError> {
        let link = self
            .store
            .links()?
            .into_iter()
            .find(|l| l.id == link_id)
            .ok_or_else(|| AppError::Refused("there's no such link".into()))?;
        self.store.delete_link(link_id)?;
        self.changed(&link.upper_id);
        self.changed(&link.lower_id);
        Ok(())
    }

    /// A topic's links, each way and of each kind, and its layer on the map.
    pub(crate) fn topic_links(&self, topic_id: &str) -> Result<TopicLinks, AppError> {
        let links = self.store.links()?;
        let titled = |l: &LinkRow, other: &str| -> Option<LinkedTopic> {
            self.store.topic(other).ok().flatten().map(|t| LinkedTopic {
                link_id: l.id.clone(),
                topic_id: t.id,
                title: t.title,
            })
        };
        let mut out = TopicLinks::default();
        for l in &links {
            match (l.kind.as_str(), l.upper_id == topic_id, l.lower_id == topic_id) {
                ("rests_on", true, _) => out.rests_on.extend(titled(l, &l.lower_id)),
                ("rests_on", _, true) => out.supports.extend(titled(l, &l.upper_id)),
                ("part_of", true, _) => out.parts.extend(titled(l, &l.lower_id)),
                ("part_of", _, true) => out.part_of.extend(titled(l, &l.upper_id)),
                _ => {}
            }
        }
        let edges: Vec<Edge> = links.iter().map(|l| (l.upper_id.as_str(), l.lower_id.as_str())).collect();
        out.layer = layers(&[topic_id], &edges).get(topic_id).copied().unwrap_or(0) as i64;
        Ok(out)
    }
}

#[derive(Default)]
pub(crate) struct TopicLinks {
    pub rests_on: Vec<LinkedTopic>,
    pub supports: Vec<LinkedTopic>,
    pub parts: Vec<LinkedTopic>,
    pub part_of: Vec<LinkedTopic>,
    pub layer: i64,
}
