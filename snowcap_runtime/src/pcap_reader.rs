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

//! Reads pcap files and extracts all udp packets with exactly 8 bytes, to interpret them.

use super::physical_network::CLIENT_ID_BASE;
use snowcap::netsim::{Prefix, RouterId};

use etherparse::{SlicedPacket, TransportSlice};
use pcap::Capture;
use std::collections::HashMap;
use std::error::Error;

/// Read a pcap file, extract all packets, and return which flows and which sequence numbers of
/// these flows have been seen
pub fn extract_pcap_flows(
    filename: impl AsRef<str>,
) -> Result<HashMap<u32, Vec<u32>>, Box<dyn Error>> {
    let mut cap = Capture::from_file(filename.as_ref())?;
    let mut result = HashMap::new();

    // iterate over all received packets
    while let Ok(packet) = cap.next() {
        let packet = SlicedPacket::from_ethernet(packet.data)?;
        let payload = packet.payload;
        let header_correct = match packet.transport {
            Some(TransportSlice::Udp(h)) => {
                let h = h.to_header();
                h.destination_port == 5001 && h.length == 16
            }
            _ => false,
        };

        if header_correct {
            // packet is one of the packets that we wish to look at!
            // Packet has the form [ flow_id;4, seq_num;4 ]
            let flow_id: u32 = ((payload[0] as u32) << 24)
                + ((payload[1] as u32) << 16)
                + ((payload[2] as u32) << 8)
                + (payload[3] as u32);
            let seq_num: u32 = ((payload[4] as u32) << 24)
                + ((payload[5] as u32) << 16)
                + ((payload[6] as u32) << 8)
                + (payload[7] as u32);

            if !result.contains_key(&flow_id) {
                result.insert(flow_id, Vec::new());
            }
            result.get_mut(&flow_id).unwrap().push(seq_num);
        }
    }

    // sort every vector (just to be safe. It should already be sorted!!)
    result.iter_mut().for_each(|(_, v)| v.sort());

    Ok(result)
}

/// Infer the path of all packets using the results from the pcap files
///
/// Input: Every element of the vector contains the following fields:
/// - Link node A,
/// - Link node B,
/// - Table, mapping the flow id to the received sequence numbers (sorted) at this node.
///
/// The output is shaped as follows: For each flow id, we have a table, which maps a path (wrapped
/// in an option to also incoperate when packets were dropped) to the number of packets that took
/// this path.
///
/// # TODO
/// This algorithm can be improved, if we would actively go through every capture in sequential
/// order, removing sequence numbers that we have already seen.
///
pub fn path_inference(
    captures: Vec<(RouterId, RouterId, HashMap<u32, Vec<u32>>)>,
    flows: &HashMap<(RouterId, Prefix), u32>,
) -> HashMap<(RouterId, Prefix), HashMap<Option<Vec<RouterId>>, usize>> {
    // extract all flows to consider, including their maximum sequence number
    let mut flow_counts: HashMap<u32, u32> = HashMap::new();
    captures.iter().map(|(_, _, caps)| caps.iter()).flatten().for_each(|(flow, v)| {
        let entry = flow_counts.entry(*flow).or_insert(0);
        *entry = *v.last().unwrap() + 1;
    });

    let flows_inverse: HashMap<u32, (RouterId, Prefix)> =
        flows.iter().map(|((r, p), flow)| (*flow, (*r, *p))).collect();

    let mut result: HashMap<(RouterId, Prefix), HashMap<Option<Vec<RouterId>>, usize>> =
        flows.iter().map(|((r, p), _)| ((*r, *p), HashMap::new())).collect();

    // go through all flows and try to reconstruct the for each packet
    for (flow, num_packets) in flow_counts {
        let (flow_start, flow_prefix) = flows_inverse.get(&flow).unwrap();
        // repeat for every single packet
        for seq_num in 0..num_packets {
            // get all links, that contain this sequence number of the given flow
            let mut links = captures
                .iter()
                .filter(|(_, _, cap)| {
                    cap.get(&flow).map(|v| v.binary_search(&seq_num).is_ok()).unwrap_or(false)
                })
                .map(|(a, b, _)| (*a, *b))
                .collect::<Vec<(RouterId, RouterId)>>();

            // reconstruct the path.
            let mut path = vec![*flow_start];
            let valid_path = loop {
                let last_node = path.last().unwrap();
                if last_node != flow_start && last_node.index() >= CLIENT_ID_BASE as usize {
                    // we reached the end
                    break true;
                }
                // search the next link
                let next_link = links
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (a, b))| {
                        if a == last_node {
                            Some((i, *b))
                        } else if b == last_node {
                            Some((i, *a))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if next_link.len() != 1 {
                    // number of next possible links is not exactly 1! abort
                    break false;
                }
                let (idx, next_link) = next_link[0];
                links.remove(idx);
                path.push(next_link);
            };

            // wrap the path in an option
            let path = if valid_path { Some(path) } else { None };

            // store the statistics
            *result.get_mut(&(*flow_start, *flow_prefix)).unwrap().entry(path).or_insert(0) += 1;
        }
    }

    result
}

#[cfg(test)]
mod test {
    use super::*;
    use maplit::hashmap;

    #[test]
    fn read_pcap() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let crate_name = env!("CARGO_CRATE_NAME");
        let filename = if manifest_dir.ends_with(crate_name) {
            format!("{}/test_files/test.pcap", manifest_dir)
        } else {
            format!("{}/{}/test_files/test.pcap", manifest_dir, crate_name)
        };
        let result = extract_pcap_flows(filename).unwrap();
        assert!(result.contains_key(&44));
        let v = result.get(&44).unwrap();
        assert_eq!(v, &vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn path_inference_all_correct() {
        let n1: RouterId = 1000001.into();
        let n2: RouterId = 2.into();
        let n3: RouterId = 3.into();
        let n4: RouterId = 4.into();
        let n5: RouterId = 5.into();
        let n6: RouterId = 1000006.into();

        let flows = hashmap![(n1, Prefix(0)) => 0, (n1, Prefix(1)) => 1];

        let captures = vec![
            (
                n2,
                n1,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                    1 => vec![0, 1, 2, 3, 4, 5, 6, 7]
                },
            ),
            (
                n2,
                n3,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                },
            ),
            (
                n4,
                n2,
                hashmap! {
                    1 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                },
            ),
            (
                n3,
                n5,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                },
            ),
            (
                n4,
                n5,
                hashmap! {
                    1 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                },
            ),
            (
                n6,
                n5,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                    1 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                },
            ),
        ];

        let result = path_inference(captures, &flows);

        assert_eq!(
            result,
            hashmap![
                (n1, Prefix(0)) => hashmap![Some(vec![n1, n2, n3, n5, n6]) => 8],
                (n1, Prefix(1)) => hashmap![Some(vec![n1, n2, n4, n5, n6]) => 8]
            ]
        );
    }

    #[test]
    fn path_inference_some_correct() {
        let n1: RouterId = 1000001.into();
        let n2: RouterId = 2.into();
        let n3: RouterId = 3.into();
        let n4: RouterId = 4.into();
        let n5: RouterId = 5.into();
        let n6: RouterId = 1000006.into();

        let flows = hashmap![(n1, Prefix(0)) => 0, (n1, Prefix(1)) => 1];

        let captures = vec![
            (
                n2,
                n1,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                    1 => vec![0, 1, 2, 3, 4, 5, 6, 7]
                },
            ),
            (
                n2,
                n3,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5],
                },
            ),
            (
                n4,
                n2,
                hashmap! {
                    0 => vec![6, 7],
                    1 => vec![0, 1, 2, 5, 6, 7],
                },
            ),
            (
                n3,
                n5,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5],
                },
            ),
            (
                n4,
                n5,
                hashmap! {
                    0 => vec![6, 7],
                    1 => vec![0, 1, 2, 5, 6, 7],
                },
            ),
            (
                n6,
                n5,
                hashmap! {
                    0 => vec![0, 1, 2, 3, 4, 5, 6, 7],
                    1 => vec![0, 1, 2, 5, 6, 7],
                },
            ),
        ];

        let result = path_inference(captures, &flows);

        assert_eq!(
            result,
            hashmap![
                (n1, Prefix(0)) => hashmap![
                    Some(vec![n1, n2, n3, n5, n6]) => 6,
                    Some(vec![n1, n2, n4, n5, n6]) => 2
                ],
                (n1, Prefix(1)) => hashmap![
                    Some(vec![n1, n2, n4, n5, n6]) => 6,
                    None => 2,
                ]
            ]
        );
    }
}
