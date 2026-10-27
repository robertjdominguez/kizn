//! The map: topics linked "rests on" (a foundation below) and "part of" (a
//! piece of a broader topic below it). Every link points from an upper
//! topic down to a lower one; any topic can be upper in some links and lower
//! in others, at any depth, fanning out both ways. Pure: the links in,
//! answers out.

use std::collections::{HashMap, HashSet};

/// A link, upper topic first: (upper, lower).
pub type Edge<'a> = (&'a str, &'a str);

/// The topics below `from`, at any depth.
pub fn below<'a>(edges: &[Edge<'a>], from: &str) -> HashSet<&'a str> {
    let mut seen: HashSet<&'a str> = HashSet::new();
    let mut stack: Vec<&str> = vec![from];
    while let Some(t) = stack.pop() {
        for &(up, down) in edges {
            if up == t && seen.insert(down) {
                stack.push(down);
            }
        }
    }
    seen
}

/// The topics above `from`, at any depth.
pub fn above<'a>(edges: &[Edge<'a>], from: &str) -> HashSet<&'a str> {
    let mut seen: HashSet<&'a str> = HashSet::new();
    let mut stack: Vec<&str> = vec![from];
    while let Some(t) = stack.pop() {
        for &(up, down) in edges {
            if down == t && seen.insert(up) {
                stack.push(up);
            }
        }
    }
    seen
}

/// Whether linking `upper` above `lower` would make a loop: `upper` is
/// already somewhere below `lower` (or they're the same topic).
pub fn would_loop(edges: &[Edge], upper: &str, lower: &str) -> bool {
    upper == lower || below(edges, lower).contains(upper)
}

/// Each topic's layer: 0 for a topic with nothing below it, otherwise one
/// more than the highest layer below it (the longest chain down). Topics
/// with no links at all are layer 0.
pub fn layers<'a>(topics: &[&'a str], edges: &[Edge<'a>]) -> HashMap<&'a str, usize> {
    let mut layer: HashMap<&'a str, usize> = HashMap::new();
    fn visit<'a>(
        t: &'a str,
        edges: &[Edge<'a>],
        layer: &mut HashMap<&'a str, usize>,
        path: &mut Vec<&'a str>,
    ) -> usize {
        if let Some(&l) = layer.get(t) {
            return l;
        }
        if path.contains(&t) {
            return 0; // a loop can't be stored, but never recurse forever
        }
        path.push(t);
        let mut l = 0;
        for &(up, down) in edges {
            if up == t {
                l = l.max(visit(down, edges, layer, path) + 1);
            }
        }
        path.pop();
        layer.insert(t, l);
        l
    }
    for &t in topics {
        visit(t, edges, &mut layer, &mut vec![]);
    }
    layer
}

#[cfg(test)]
mod tests {
    use super::*;

    //        spcs
    //       /    \
    //    cf      net
    //   /  \     /
    // proc  fs  linuxnet
    const EDGES: &[Edge] =
        &[("spcs", "cf"), ("spcs", "net"), ("cf", "proc"), ("cf", "fs"), ("net", "linuxnet"), ("cf", "linuxnet")];

    #[test]
    fn reach_goes_any_depth_both_ways() {
        let down = below(EDGES, "spcs");
        assert_eq!(down.len(), 5);
        assert!(above(EDGES, "linuxnet").contains("spcs"));
        assert_eq!(above(EDGES, "linuxnet").len(), 3);
    }

    #[test]
    fn loops_are_refused_at_any_depth() {
        assert!(would_loop(EDGES, "proc", "spcs"));
        assert!(would_loop(EDGES, "cf", "cf"));
        assert!(!would_loop(EDGES, "net", "fs"));
    }

    #[test]
    fn layers_follow_the_longest_chain() {
        let topics = ["spcs", "cf", "net", "proc", "fs", "linuxnet", "alone"];
        let l = layers(&topics, EDGES);
        assert_eq!((l["proc"], l["linuxnet"], l["cf"], l["net"], l["spcs"], l["alone"]), (0, 0, 1, 1, 2, 0));
    }
}
