use std::collections::HashMap;
use vaux_mqtt::QoSLevel;

const ROOT_NODE: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscriber {
    pub session_id: Box<str>,
    pub qos: QoSLevel,
    pub no_local: bool,
}

#[derive(Debug, Default)]
struct TopicNode {
    parent: u32,
    segment: Box<str>,
    children: HashMap<Box<str>, u32>,
    subscribers: Vec<Subscriber>,
    wildcard_subscribers: Vec<Subscriber>,
}

#[derive(Debug)]
pub struct TopicTree {
    nodes: Vec<TopicNode>,
}

impl Default for TopicTree {
    fn default() -> Self {
        Self::new()
    }
}

impl TopicTree {
    pub fn new() -> Self {
        let root = TopicNode {
            parent: 0,
            segment: "".into(),
            ..Default::default()
        };
        Self { nodes: vec![root] }
    }

    pub fn subscribe(
        &mut self,
        filter: &str,
        session_id: &str,
        qos: QoSLevel,
        no_local: bool,
    ) {
        let segments: Vec<&str> = filter.split('/').collect();
        let node_idx = self.ensure_path(&segments);

        let subscriber = Subscriber {
            session_id: session_id.into(),
            qos,
            no_local,
        };

        let last_segment = *segments.last().unwrap();
        let list = if last_segment == "#" {
            &mut self.nodes[node_idx as usize].wildcard_subscribers
        } else {
            &mut self.nodes[node_idx as usize].subscribers
        };

        if let Some(existing) = list.iter_mut().find(|s| *s.session_id == *session_id) {
            existing.qos = qos;
            existing.no_local = no_local;
        } else {
            list.push(subscriber);
        }
    }

    pub fn unsubscribe(&mut self, filter: &str, session_id: &str) -> bool {
        let segments: Vec<&str> = filter.split('/').collect();
        let Some(node_idx) = self.find_path(&segments) else {
            return false;
        };

        let last_segment = *segments.last().unwrap();
        let list = if last_segment == "#" {
            &mut self.nodes[node_idx as usize].wildcard_subscribers
        } else {
            &mut self.nodes[node_idx as usize].subscribers
        };

        let before = list.len();
        list.retain(|s| *s.session_id != *session_id);
        list.len() < before
    }

    pub fn unsubscribe_all(&mut self, session_id: &str) {
        for node in &mut self.nodes {
            node.subscribers.retain(|s| *s.session_id != *session_id);
            node.wildcard_subscribers
                .retain(|s| *s.session_id != *session_id);
        }
    }

    pub fn matching_subscribers(&self, topic: &str) -> Vec<Subscriber> {
        let segments: Vec<&str> = topic.split('/').collect();
        let mut results = Vec::new();
        self.collect_matches(ROOT_NODE, &segments, 0, &mut results);
        results
    }

    fn collect_matches(
        &self,
        node_idx: u32,
        segments: &[&str],
        depth: usize,
        results: &mut Vec<Subscriber>,
    ) {
        let node = &self.nodes[node_idx as usize];

        // '#' at this level matches everything at and below
        results.extend(node.wildcard_subscribers.iter().cloned());

        if depth >= segments.len() {
            results.extend(node.subscribers.iter().cloned());
            // '#' child matches zero remaining levels
            if let Some(&hash_idx) = node.children.get("#") {
                let hash_node = &self.nodes[hash_idx as usize];
                results.extend(hash_node.wildcard_subscribers.iter().cloned());
            }
            return;
        }

        let segment = segments[depth];

        // exact match on this segment
        if let Some(&child_idx) = node.children.get(segment) {
            self.collect_matches(child_idx, segments, depth + 1, results);
        }

        // '+' wildcard matches any single level
        if let Some(&plus_idx) = node.children.get("+") {
            self.collect_matches(plus_idx, segments, depth + 1, results);
        }

        // '#' child matches this level and everything below
        if let Some(&hash_idx) = node.children.get("#") {
            let hash_node = &self.nodes[hash_idx as usize];
            results.extend(hash_node.wildcard_subscribers.iter().cloned());
        }
    }

    fn ensure_path(&mut self, segments: &[&str]) -> u32 {
        let mut current = ROOT_NODE;
        for &segment in segments {
            if let Some(&child_idx) = self.nodes[current as usize].children.get(segment) {
                current = child_idx;
            } else {
                let new_idx = self.nodes.len() as u32;
                self.nodes.push(TopicNode {
                    parent: current,
                    segment: segment.into(),
                    ..Default::default()
                });
                self.nodes[current as usize]
                    .children
                    .insert(segment.into(), new_idx);
                current = new_idx;
            }
        }
        current
    }

    fn find_path(&self, segments: &[&str]) -> Option<u32> {
        let mut current = ROOT_NODE;
        for &segment in segments {
            current = *self.nodes[current as usize].children.get(segment)?;
        }
        Some(current)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_subscribe_and_match_exact() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/temp/room1", "client-1", QoSLevel::AtLeastOnce, false);

        let subs = tree.matching_subscribers("sensor/temp/room1");
        assert_eq!(subs.len(), 1);
        assert_eq!(&*subs[0].session_id, "client-1");
        assert_eq!(subs[0].qos, QoSLevel::AtLeastOnce);

        let subs = tree.matching_subscribers("sensor/temp/room2");
        assert_eq!(subs.len(), 0);
    }

    #[test]
    fn test_subscribe_plus_wildcard() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/+/room1", "client-1", QoSLevel::AtMostOnce, false);

        let subs = tree.matching_subscribers("sensor/temp/room1");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("sensor/humidity/room1");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("sensor/temp/room2");
        assert_eq!(subs.len(), 0);

        // '+' does not match multiple levels
        let subs = tree.matching_subscribers("sensor/temp/sub/room1");
        assert_eq!(subs.len(), 0);
    }

    #[test]
    fn test_subscribe_hash_wildcard() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/#", "client-1", QoSLevel::ExactlyOnce, false);

        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("sensor/temp/room1");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("sensor");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("other/topic");
        assert_eq!(subs.len(), 0);
    }

    #[test]
    fn test_subscribe_hash_root() {
        let mut tree = TopicTree::new();
        tree.subscribe("#", "client-1", QoSLevel::AtMostOnce, false);

        let subs = tree.matching_subscribers("any/topic/at/all");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("single");
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn test_multiple_subscribers_same_topic() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/temp", "client-1", QoSLevel::AtMostOnce, false);
        tree.subscribe("sensor/temp", "client-2", QoSLevel::AtLeastOnce, false);

        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 2);
        let ids: Vec<&str> = subs.iter().map(|s| &*s.session_id).collect();
        assert!(ids.contains(&"client-1"));
        assert!(ids.contains(&"client-2"));
    }

    #[test]
    fn test_overlapping_wildcard_and_exact() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/temp", "exact", QoSLevel::AtMostOnce, false);
        tree.subscribe("sensor/+", "plus", QoSLevel::AtMostOnce, false);
        tree.subscribe("sensor/#", "hash", QoSLevel::AtMostOnce, false);

        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 3);
        let ids: Vec<&str> = subs.iter().map(|s| &*s.session_id).collect();
        assert!(ids.contains(&"exact"));
        assert!(ids.contains(&"plus"));
        assert!(ids.contains(&"hash"));
    }

    #[test]
    fn test_resubscribe_updates_qos() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/temp", "client-1", QoSLevel::AtMostOnce, false);
        tree.subscribe("sensor/temp", "client-1", QoSLevel::ExactlyOnce, false);

        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].qos, QoSLevel::ExactlyOnce);
    }

    #[test]
    fn test_unsubscribe() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/temp", "client-1", QoSLevel::AtMostOnce, false);
        tree.subscribe("sensor/temp", "client-2", QoSLevel::AtMostOnce, false);

        assert!(tree.unsubscribe("sensor/temp", "client-1"));
        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 1);
        assert_eq!(&*subs[0].session_id, "client-2");

        assert!(!tree.unsubscribe("sensor/temp", "client-1"));
    }

    #[test]
    fn test_unsubscribe_wildcard() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/#", "client-1", QoSLevel::AtMostOnce, false);

        assert!(tree.unsubscribe("sensor/#", "client-1"));
        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 0);
    }

    #[test]
    fn test_unsubscribe_nonexistent() {
        let mut tree = TopicTree::new();
        assert!(!tree.unsubscribe("no/such/topic", "client-1"));
    }

    #[test]
    fn test_unsubscribe_all() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/temp", "client-1", QoSLevel::AtMostOnce, false);
        tree.subscribe("sensor/#", "client-1", QoSLevel::AtMostOnce, false);
        tree.subscribe("other/topic", "client-1", QoSLevel::AtMostOnce, false);
        tree.subscribe("sensor/temp", "client-2", QoSLevel::AtMostOnce, false);

        tree.unsubscribe_all("client-1");

        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 1);
        assert_eq!(&*subs[0].session_id, "client-2");

        let subs = tree.matching_subscribers("other/topic");
        assert_eq!(subs.len(), 0);
    }

    #[test]
    fn test_node_reuse() {
        let mut tree = TopicTree::new();
        tree.subscribe("a/b/c", "c1", QoSLevel::AtMostOnce, false);
        tree.subscribe("a/b/d", "c2", QoSLevel::AtMostOnce, false);
        // "a" and "a/b" should be shared: root + a + b + c + d = 5 nodes
        assert_eq!(tree.node_count(), 5);
    }

    #[test]
    fn test_plus_does_not_match_deeper() {
        let mut tree = TopicTree::new();
        tree.subscribe("+/temp", "client-1", QoSLevel::AtMostOnce, false);

        let subs = tree.matching_subscribers("sensor/temp");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("a/b/temp");
        assert_eq!(subs.len(), 0);
    }

    #[test]
    fn test_hash_matches_parent_level() {
        let mut tree = TopicTree::new();
        tree.subscribe("sensor/#", "client-1", QoSLevel::AtMostOnce, false);

        // '#' matches the parent level itself (zero remaining levels)
        let subs = tree.matching_subscribers("sensor");
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn test_deeply_nested_topic() {
        let mut tree = TopicTree::new();
        tree.subscribe("a/b/c/d/e/f", "deep", QoSLevel::AtMostOnce, false);
        tree.subscribe("a/+/c/+/e/+", "wild", QoSLevel::AtMostOnce, false);

        let subs = tree.matching_subscribers("a/b/c/d/e/f");
        assert_eq!(subs.len(), 2);

        let subs = tree.matching_subscribers("a/x/c/y/e/z");
        assert_eq!(subs.len(), 1);
        assert_eq!(&*subs[0].session_id, "wild");
    }

    #[test]
    fn test_empty_topic_segment() {
        let mut tree = TopicTree::new();
        // MQTT allows empty segments: "a//b" has three segments: "a", "", "b"
        tree.subscribe("a//b", "client-1", QoSLevel::AtMostOnce, false);

        let subs = tree.matching_subscribers("a//b");
        assert_eq!(subs.len(), 1);

        let subs = tree.matching_subscribers("a/b");
        assert_eq!(subs.len(), 0);
    }
}
