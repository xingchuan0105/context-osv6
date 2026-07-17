//! §7.1 Channel materialization from `CapabilitySet`.

use super::types::Channel;
use crate::capabilities::CapabilitySet;

/// Map product capabilities to worker channels that **must** exist this turn.
///
/// LLM orchestrator cannot remove these nodes; it only writes briefs / topology
/// among them (and optional multi-hop re-dispatch).
pub fn materialize_channels(caps: CapabilitySet) -> Vec<Channel> {
    let mut out = Vec::new();
    if caps.rag {
        out.push(Channel::Rag);
    }
    if caps.search {
        out.push(Channel::Search);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_chat_materializes_nothing() {
        assert!(materialize_channels(CapabilitySet::default()).is_empty());
    }

    #[test]
    fn rag_only() {
        assert_eq!(
            materialize_channels(CapabilitySet {
                rag: true,
                search: false
            }),
            vec![Channel::Rag]
        );
    }

    #[test]
    fn search_only() {
        assert_eq!(
            materialize_channels(CapabilitySet {
                rag: false,
                search: true
            }),
            vec![Channel::Search]
        );
    }

    #[test]
    fn dual_both_channels() {
        assert_eq!(
            materialize_channels(CapabilitySet {
                rag: true,
                search: true
            }),
            vec![Channel::Rag, Channel::Search]
        );
    }
}
