// Snowcap: Synthesizing Network-Wide Configuration Updates
// Copyright (C) 2021  Tibor Schneider
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program; if not, write to the Free Software Foundation, Inc.,
// 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.

//! Parses GML files from Topology Zoo

use super::NodeData;
use crate::netsim::{AsId, LinkWeight};

use petgraph::prelude::*;
use std::collections::HashMap;
use std::fs::read_to_string;
use thiserror::Error;

/// Parses GML files and returns the resulting graph
/// The names will remain the same, except the same name occurs twice. In this case, we will append
/// a _N to the end, where N is a number starting from 1 (_1 is appended to the second occurence,
/// and _2 is appended to the third occurence, etc...).
pub fn parse_gml_graph(
    filename: impl AsRef<str>,
) -> Result<Graph<NodeData, LinkWeight, Undirected, u32>, GmlError> {
    let mut g: Graph<NodeData, LinkWeight, Undirected, u32> =
        Graph::<NodeData, LinkWeight, Undirected, u32>::new_undirected();

    let gml_str = read_to_string(filename.as_ref())?;

    let mut current_state = CurrentState::NotStarted;

    let mut current_as_id: u32 = 65100;
    let mut as_id_lookup: HashMap<String, AsId> = HashMap::new();

    let mut used_labels: HashMap<String, usize> = HashMap::new();
    let mut node_lookup: HashMap<usize, NodeIndex<u32>> = HashMap::new();

    for (i, line) in gml_str.lines().enumerate() {
        let line = line.trim();
        current_state = match current_state {
            CurrentState::NotStarted => {
                if line == "graph [" {
                    CurrentState::None
                } else {
                    return Err(GmlError::UnexpectedToken { line: i, content: String::from(line) });
                }
            }
            CurrentState::None => {
                if line == "node [" {
                    CurrentState::Node { id: None, name: None, external: None }
                } else if line == "edge [" {
                    CurrentState::Edge { source: None, target: None }
                } else {
                    CurrentState::None
                }
            }
            CurrentState::Node { id, name, external } => {
                if let Some(number) = line.strip_prefix("id ") {
                    let id: Option<usize> = Some(number.parse()?);
                    CurrentState::Node { id, name, external }
                } else if line.starts_with("label ") {
                    let len_line: usize = line.len();
                    let mut name: String = String::from(&line[7..len_line - 1]).replace(" ", "_");
                    // increment the num_used in the hashmap
                    let num_used = *used_labels.get(&name).unwrap_or(&0);
                    used_labels.insert(name.clone(), num_used + 1);
                    if num_used > 0 {
                        name.push_str(&format!("_{}", num_used));
                    }
                    let name = Some(name);
                    CurrentState::Node { id, name, external }
                } else if line.starts_with("Internal ") {
                    let external = if line == "Internal 1" {
                        Some(false)
                    } else if line == "Internal 0" {
                        Some(true)
                    } else {
                        return Err(GmlError::UnexpectedToken {
                            line: i,
                            content: String::from(line),
                        });
                    };
                    CurrentState::Node { id, name, external }
                } else if line == "]" {
                    let ext = external.ok_or(GmlError::NodeMissingInternal(i))?;
                    let name = name.ok_or(GmlError::NodeMissingLabel(i))?;
                    let as_id = if !ext {
                        AsId(65001)
                    } else if as_id_lookup.contains_key(&name) {
                        *as_id_lookup.get(&name).unwrap()
                    } else {
                        current_as_id += 1;
                        as_id_lookup.insert(name.clone(), AsId(current_as_id));
                        AsId(current_as_id)
                    };
                    let node_idx =
                        g.add_node(NodeData { name, external: ext, as_id, net_idx: None });
                    let id = id.ok_or(GmlError::NodeMissingId(i))?;
                    if node_lookup.contains_key(&id) {
                        return Err(GmlError::NodeIdNotUnique(i));
                    }
                    node_lookup.insert(id, node_idx);
                    CurrentState::None
                } else {
                    CurrentState::Node { id, name, external }
                }
            }
            CurrentState::Edge { source, target } => {
                if let Some(number) = line.strip_prefix("source ") {
                    let source: Option<usize> = Some(number.parse()?);
                    CurrentState::Edge { source, target }
                } else if let Some(number) = line.strip_prefix("target ") {
                    let target: Option<usize> = Some(number.parse()?);
                    CurrentState::Edge { source, target }
                } else if line == "]" {
                    let source = source.ok_or(GmlError::EdgeMissingSource(i))?;
                    let source_idx =
                        node_lookup.get(&source).ok_or(GmlError::UnknownNodeId(source))?;
                    let target = target.ok_or(GmlError::EdgeMissingTarget(i))?;
                    let target_idx =
                        node_lookup.get(&target).ok_or(GmlError::UnknownNodeId(source))?;
                    // check if the edge already exists
                    if g.contains_edge(*source_idx, *target_idx) {
                        // ignoring the duplicate link
                    } else {
                        g.add_edge(*source_idx, *target_idx, 1.0);
                    }
                    CurrentState::None
                } else {
                    CurrentState::Edge { source, target }
                }
            }
        };
    }

    Ok(g)
}

enum CurrentState {
    NotStarted,
    None,
    Node { id: Option<usize>, name: Option<String>, external: Option<bool> },
    Edge { source: Option<usize>, target: Option<usize> },
}

#[derive(Debug, Error)]
pub enum GmlError {
    /// Io Error
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    /// Unexpected Token
    #[error("Unexpected Token on line {line}: {content}")]
    UnexpectedToken { line: usize, content: String },
    /// ParseIntError
    #[error("Cannot parse an integer! {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    /// Unknown Node Id
    #[error("Unknown node id: {0}")]
    UnknownNodeId(usize),
    /// Node is missing an ID field
    #[error("Node is missing an ID field before line {0}!")]
    NodeMissingId(usize),
    /// Node is missing an label field
    #[error("Node is missing an label field before line {0}!")]
    NodeMissingLabel(usize),
    /// Node is missing an internal field
    #[error("Node is missing an internal field before line {0}!")]
    NodeMissingInternal(usize),
    /// Duplicate Noe Id
    #[error("Node ID is not unique on line {0}!")]
    NodeIdNotUnique(usize),
    /// Edge is missing the source field
    #[error("Ege is missing the source field before line {0}!")]
    EdgeMissingSource(usize),
    /// Edge is missing the target field
    #[error("Ege is missing the target field before line {0}!")]
    EdgeMissingTarget(usize),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_with_switch_gml() {
        let filename = format!("{}/test_files/switch.gml", env!("CARGO_MANIFEST_DIR"));
        let g = parse_gml_graph(filename).unwrap();

        // check all indices and node names
        assert_eq!(g.node_weight(00.into()).unwrap().name, "Fribourg");
        assert_eq!(g.node_weight(01.into()).unwrap().name, "Basel");
        assert_eq!(g.node_weight(02.into()).unwrap().name, "Delemont");
        assert_eq!(g.node_weight(03.into()).unwrap().name, "Bern");
        assert_eq!(g.node_weight(04.into()).unwrap().name, "Kreuzlingen");
        assert_eq!(g.node_weight(05.into()).unwrap().name, "St._Gallen");
        assert_eq!(g.node_weight(06.into()).unwrap().name, "Villigen_PSI");
        assert_eq!(g.node_weight(07.into()).unwrap().name, "Zurich_(ETH)");
        assert_eq!(g.node_weight(08.into()).unwrap().name, "Buchs_SG");
        assert_eq!(g.node_weight(09.into()).unwrap().name, "Chur");
        assert_eq!(g.node_weight(10.into()).unwrap().name, "GBLX");
        assert_eq!(g.node_weight(11.into()).unwrap().name, "Swisscom");
        assert_eq!(g.node_weight(12.into()).unwrap().name, "Swisscom_1");
        assert_eq!(g.node_weight(13.into()).unwrap().name, "TELIA");
        assert_eq!(g.node_weight(14.into()).unwrap().name, "SwissIX");
        assert_eq!(g.node_weight(15.into()).unwrap().name, "TIX");
        assert_eq!(g.node_weight(16.into()).unwrap().name, "BelWue");
        assert_eq!(g.node_weight(17.into()).unwrap().name, "CERN");
        assert_eq!(g.node_weight(18.into()).unwrap().name, "CIXP");
        assert_eq!(g.node_weight(19.into()).unwrap().name, "AMS-IX");
        assert_eq!(g.node_weight(20.into()).unwrap().name, "SwissIX_1");
        assert_eq!(g.node_weight(21.into()).unwrap().name, "GEANT2");
        assert_eq!(g.node_weight(22.into()).unwrap().name, "Neuchatel");
        assert_eq!(g.node_weight(23.into()).unwrap().name, "IXEurope_Zurich");
        assert_eq!(g.node_weight(24.into()).unwrap().name, "Brig");
        assert_eq!(g.node_weight(25.into()).unwrap().name, "Martigny");
        assert_eq!(g.node_weight(26.into()).unwrap().name, "Grenchen");
        assert_eq!(g.node_weight(27.into()).unwrap().name, "Davos");
        assert_eq!(g.node_weight(28.into()).unwrap().name, "Liechtenstein");
        assert_eq!(g.node_weight(29.into()).unwrap().name, "Rapperswil");
        assert_eq!(g.node_weight(30.into()).unwrap().name, "Manno");
        assert_eq!(g.node_weight(31.into()).unwrap().name, "Zurich_(University)");
        assert_eq!(g.node_weight(32.into()).unwrap().name, "Horw");
        assert_eq!(g.node_weight(33.into()).unwrap().name, "Brugg");
        assert_eq!(g.node_weight(34.into()).unwrap().name, "CERN_1");
        assert_eq!(g.node_weight(35.into()).unwrap().name, "Lausanne_(University)");
        assert_eq!(g.node_weight(36.into()).unwrap().name, "Geneva");
        assert_eq!(g.node_weight(37.into()).unwrap().name, "Lausanne_(EPFL)");
        assert_eq!(g.node_weight(38.into()).unwrap().name, "Yverdon");
        assert_eq!(g.node_weight(39.into()).unwrap().name, "Birmensdorf");
        assert_eq!(g.node_weight(40.into()).unwrap().name, "Rueschlikon");
        assert_eq!(g.node_weight(41.into()).unwrap().name, "Winterthur");

        // check that the nodes exist and are correct
        assert_eq!(g.edge_endpoints(00.into()), Some((00.into(), 35.into())));
        assert_eq!(g.edge_endpoints(01.into()), Some((00.into(), 03.into())));
        assert_eq!(g.edge_endpoints(02.into()), Some((01.into(), 33.into())));
        assert_eq!(g.edge_endpoints(03.into()), Some((01.into(), 02.into())));
        assert_eq!(g.edge_endpoints(04.into()), Some((01.into(), 03.into())));
        assert_eq!(g.edge_endpoints(05.into()), Some((01.into(), 06.into())));
        assert_eq!(g.edge_endpoints(06.into()), Some((01.into(), 07.into())));
        assert_eq!(g.edge_endpoints(07.into()), Some((01.into(), 20.into())));
        assert_eq!(g.edge_endpoints(08.into()), Some((02.into(), 22.into())));
        assert_eq!(g.edge_endpoints(09.into()), Some((03.into(), 35.into())));
        assert_eq!(g.edge_endpoints(10.into()), Some((03.into(), 30.into())));
        assert_eq!(g.edge_endpoints(11.into()), Some((04.into(), 16.into())));
        assert_eq!(g.edge_endpoints(12.into()), Some((04.into(), 05.into())));
        assert_eq!(g.edge_endpoints(13.into()), Some((04.into(), 31.into())));
        assert_eq!(g.edge_endpoints(14.into()), Some((05.into(), 08.into())));
        assert_eq!(g.edge_endpoints(15.into()), Some((05.into(), 28.into())));
        assert_eq!(g.edge_endpoints(16.into()), Some((05.into(), 41.into())));
        assert_eq!(g.edge_endpoints(17.into()), Some((06.into(), 07.into())));
        assert_eq!(g.edge_endpoints(18.into()), Some((07.into(), 32.into())));
        assert_eq!(g.edge_endpoints(19.into()), Some((07.into(), 35.into())));
        assert_eq!(g.edge_endpoints(20.into()), Some((07.into(), 39.into())));
        assert_eq!(g.edge_endpoints(21.into()), Some((07.into(), 41.into())));
        assert_eq!(g.edge_endpoints(22.into()), Some((07.into(), 23.into())));
        assert_eq!(g.edge_endpoints(23.into()), Some((07.into(), 29.into())));
        assert_eq!(g.edge_endpoints(24.into()), Some((07.into(), 30.into())));
        assert_eq!(g.edge_endpoints(25.into()), Some((08.into(), 09.into())));
        assert_eq!(g.edge_endpoints(26.into()), Some((08.into(), 27.into())));
        assert_eq!(g.edge_endpoints(27.into()), Some((08.into(), 28.into())));
        assert_eq!(g.edge_endpoints(28.into()), Some((09.into(), 27.into())));
        assert_eq!(g.edge_endpoints(29.into()), Some((09.into(), 29.into())));
        assert_eq!(g.edge_endpoints(30.into()), Some((10.into(), 34.into())));
        assert_eq!(g.edge_endpoints(31.into()), Some((11.into(), 34.into())));
        assert_eq!(g.edge_endpoints(32.into()), Some((12.into(), 23.into())));
        assert_eq!(g.edge_endpoints(33.into()), Some((13.into(), 23.into())));
        assert_eq!(g.edge_endpoints(34.into()), Some((14.into(), 23.into())));
        assert_eq!(g.edge_endpoints(35.into()), Some((15.into(), 23.into())));
        assert_eq!(g.edge_endpoints(36.into()), Some((17.into(), 34.into())));
        assert_eq!(g.edge_endpoints(37.into()), Some((18.into(), 34.into())));
        assert_eq!(g.edge_endpoints(38.into()), Some((19.into(), 34.into())));
        assert_eq!(g.edge_endpoints(39.into()), Some((21.into(), 34.into())));
        assert_eq!(g.edge_endpoints(40.into()), Some((22.into(), 37.into())));
        assert_eq!(g.edge_endpoints(41.into()), Some((22.into(), 26.into())));
        assert_eq!(g.edge_endpoints(42.into()), Some((22.into(), 38.into())));
        assert_eq!(g.edge_endpoints(43.into()), Some((22.into(), 31.into())));
        assert_eq!(g.edge_endpoints(44.into()), Some((23.into(), 40.into())));
        assert_eq!(g.edge_endpoints(45.into()), Some((24.into(), 25.into())));
        assert_eq!(g.edge_endpoints(46.into()), Some((24.into(), 30.into())));
        assert_eq!(g.edge_endpoints(47.into()), Some((25.into(), 37.into())));
        assert_eq!(g.edge_endpoints(48.into()), Some((26.into(), 31.into())));
        assert_eq!(g.edge_endpoints(49.into()), Some((28.into(), 29.into())));
        assert_eq!(g.edge_endpoints(50.into()), Some((29.into(), 30.into())));
        assert_eq!(g.edge_endpoints(51.into()), Some((30.into(), 32.into())));
        assert_eq!(g.edge_endpoints(52.into()), Some((30.into(), 37.into())));
        assert_eq!(g.edge_endpoints(53.into()), Some((31.into(), 33.into())));
        assert_eq!(g.edge_endpoints(54.into()), Some((31.into(), 34.into())));
        assert_eq!(g.edge_endpoints(55.into()), Some((31.into(), 37.into())));
        assert_eq!(g.edge_endpoints(56.into()), Some((34.into(), 35.into())));
        assert_eq!(g.edge_endpoints(57.into()), Some((34.into(), 36.into())));
        assert_eq!(g.edge_endpoints(58.into()), Some((34.into(), 37.into())));
        assert_eq!(g.edge_endpoints(59.into()), Some((35.into(), 37.into())));
        assert_eq!(g.edge_endpoints(60.into()), Some((36.into(), 37.into())));
        assert_eq!(g.edge_endpoints(61.into()), Some((37.into(), 38.into())));
        assert_eq!(g.edge_endpoints(62.into()), Some((39.into(), 40.into())));
    }
}
