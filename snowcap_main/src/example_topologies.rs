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

use snowcap::example_networks::{self, repetitions::*, ExampleNetwork};
use snowcap::hard_policies::*;
use snowcap::netsim::{config::Config, Network};
use std::error::Error;
use std::fmt;

use clap::Clap;

#[derive(Clap, Debug, Clone)]
pub enum Topology {
    AbileneNetwork,
    BipartiteCarouselFusion,
    BipartiteGadget,
    CarouselGadget,
    ChainGadget,
    DifficultGadgetComplete,
    DifficultGadgetMinimal,
    DifficultGadgetRepeated,
    EvilTwinGadget,
    FirewallNet,
    MediumNet,
    VariableAbileneNetwork,
    SimpleNet,
    SmallNet,
    StateSpecificChainGadget,
}

impl fmt::Display for Topology {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Topology::AbileneNetwork => {
                write!(f, "AbileneNetwork")
            }
            Topology::BipartiteCarouselFusion => {
                write!(f, "BipartiteCarouselFusion")
            }
            Topology::BipartiteGadget => {
                write!(f, "BipartiteGadget")
            }
            Topology::CarouselGadget => {
                write!(f, "CarouselGadget")
            }
            Topology::ChainGadget => {
                write!(f, "ChainGadget")
            }
            Topology::DifficultGadgetComplete => {
                write!(f, "DifficultGadgetComplete")
            }
            Topology::DifficultGadgetMinimal => {
                write!(f, "DifficultGadgetMinimal")
            }
            Topology::DifficultGadgetRepeated => {
                write!(f, "DifficultGadgetRepeated")
            }
            Topology::EvilTwinGadget => {
                write!(f, "EvilTwinGadget")
            }
            Topology::FirewallNet => {
                write!(f, "FirewallNet")
            }
            Topology::MediumNet => {
                write!(f, "MediumNet")
            }
            Topology::VariableAbileneNetwork => {
                write!(f, "VariableAbileneNetwork")
            }
            Topology::SimpleNet => {
                write!(f, "SimpleNet")
            }
            Topology::SmallNet => {
                write!(f, "SmallNet")
            }
            Topology::StateSpecificChainGadget => {
                write!(f, "StateSpecificChainGadget")
            }
        }
    }
}

#[derive(Clap, Debug, Clone, PartialEq, Eq, Copy)]
pub enum Reps {
    #[clap(name = "1")]
    Rep1,
    #[clap(name = "2")]
    Rep2,
    #[clap(name = "3")]
    Rep3,
    #[clap(name = "4")]
    Rep4,
    #[clap(name = "5")]
    Rep5,
    #[clap(name = "6")]
    Rep6,
    #[clap(name = "7")]
    Rep7,
    #[clap(name = "8")]
    Rep8,
    #[clap(name = "9")]
    Rep9,
    #[clap(name = "10")]
    Rep10,
    #[clap(name = "11")]
    Rep11,
    #[clap(name = "12")]
    Rep12,
    #[clap(name = "13")]
    Rep13,
    #[clap(name = "14")]
    Rep14,
    #[clap(name = "15")]
    Rep15,
    #[clap(name = "16")]
    Rep16,
    #[clap(name = "17")]
    Rep17,
    #[clap(name = "18")]
    Rep18,
    #[clap(name = "19")]
    Rep19,
    #[clap(name = "20")]
    Rep20,
    #[clap(name = "30")]
    Rep30,
    #[clap(name = "40")]
    Rep40,
    #[clap(name = "50")]
    Rep50,
    #[clap(name = "60")]
    Rep60,
    #[clap(name = "70")]
    Rep70,
    #[clap(name = "80")]
    Rep80,
    #[clap(name = "90")]
    Rep90,
    #[clap(name = "100")]
    Rep100,
}

impl fmt::Display for Reps {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Reps::Rep1 => write!(f, "1"),
            Reps::Rep2 => write!(f, "2"),
            Reps::Rep3 => write!(f, "3"),
            Reps::Rep4 => write!(f, "4"),
            Reps::Rep5 => write!(f, "5"),
            Reps::Rep6 => write!(f, "6"),
            Reps::Rep7 => write!(f, "7"),
            Reps::Rep8 => write!(f, "8"),
            Reps::Rep9 => write!(f, "9"),
            Reps::Rep10 => write!(f, "10"),
            Reps::Rep11 => write!(f, "11"),
            Reps::Rep12 => write!(f, "12"),
            Reps::Rep13 => write!(f, "13"),
            Reps::Rep14 => write!(f, "14"),
            Reps::Rep15 => write!(f, "15"),
            Reps::Rep16 => write!(f, "16"),
            Reps::Rep17 => write!(f, "17"),
            Reps::Rep18 => write!(f, "18"),
            Reps::Rep19 => write!(f, "19"),
            Reps::Rep20 => write!(f, "20"),
            Reps::Rep30 => write!(f, "30"),
            Reps::Rep40 => write!(f, "40"),
            Reps::Rep50 => write!(f, "50"),
            Reps::Rep60 => write!(f, "60"),
            Reps::Rep70 => write!(f, "70"),
            Reps::Rep80 => write!(f, "80"),
            Reps::Rep90 => write!(f, "90"),
            Reps::Rep100 => write!(f, "100"),
        }
    }
}

pub fn example_networks_scenario(
    topology: Topology,
    initial_variant: usize,
    final_variant: Option<usize>,
    repetitions: Option<Reps>,
) -> Result<(Network, Config, HardPolicy), Box<dyn Error>> {
    let final_variant = final_variant.unwrap_or(initial_variant);

    match (topology, repetitions) {
        (Topology::AbileneNetwork, _) => {
            type CurrentNet = example_networks::AbileneNetwork;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep1)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition1>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep2)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition2>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep3)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition3>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep4)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition4>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep5)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition5>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep6)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition6>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep7)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition7>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep8)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition8>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep9)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition9>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep10)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition10>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep11)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition11>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep12)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition12>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep13)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition13>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep14)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition14>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep15)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition15>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep16)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition16>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep17)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition17>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep18)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition18>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep19)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition19>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep20)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition20>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep30)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition30>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep40)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition40>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep50)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition50>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep60)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition60>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep70)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition70>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep80)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition80>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep90)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition90>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, Some(Reps::Rep100)) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion<Repetition100>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteCarouselFusion, _) => {
            type CurrentNet = example_networks::BipartiteCarouselFusion;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep1)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition1>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep2)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition2>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep3)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition3>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep4)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition4>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep5)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition5>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep6)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition6>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep7)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition7>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep8)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition8>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep9)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition9>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep10)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition10>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep11)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition11>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep12)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition12>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep13)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition13>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep14)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition14>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep15)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition15>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep16)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition16>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep17)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition17>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep18)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition18>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep19)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition19>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep20)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition20>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep30)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition30>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep40)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition40>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep50)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition50>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep60)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition60>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep70)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition70>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep80)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition80>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep90)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition90>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, Some(Reps::Rep100)) => {
            type CurrentNet = example_networks::BipartiteGadget<Repetition100>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::BipartiteGadget, _) => {
            type CurrentNet = example_networks::BipartiteGadget;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::CarouselGadget, _) => {
            type CurrentNet = example_networks::CarouselGadget;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep1)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition1>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep2)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition2>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep3)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition3>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep4)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition4>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep5)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition5>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep6)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition6>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep7)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition7>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep8)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition8>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep9)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition9>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep10)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition10>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep11)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition11>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep12)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition12>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep13)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition13>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep14)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition14>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep15)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition15>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep16)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition16>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep17)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition17>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep18)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition18>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep19)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition19>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep20)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition20>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep30)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition30>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep40)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition40>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep50)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition50>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep60)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition60>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep70)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition70>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep80)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition80>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep90)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition90>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, Some(Reps::Rep100)) => {
            type CurrentNet = example_networks::ChainGadget<Repetition100>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::ChainGadget, _) => {
            type CurrentNet = example_networks::ChainGadget;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetComplete, _) => {
            type CurrentNet = example_networks::DifficultGadgetComplete;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetMinimal, _) => {
            type CurrentNet = example_networks::DifficultGadgetMinimal;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep1)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition1>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep2)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition2>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep3)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition3>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep4)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition4>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep5)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition5>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep6)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition6>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep7)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition7>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep8)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition8>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep9)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition9>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep10)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition10>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep11)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition11>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep12)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition12>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep13)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition13>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep14)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition14>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep15)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition15>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep16)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition16>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep17)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition17>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep18)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition18>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep19)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition19>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep20)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition20>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep30)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition30>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep40)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition40>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep50)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition50>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep60)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition60>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep70)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition70>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep80)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition80>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep90)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition90>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, Some(Reps::Rep100)) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated<Repetition100>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::DifficultGadgetRepeated, _) => {
            type CurrentNet = example_networks::DifficultGadgetRepeated;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::EvilTwinGadget, _) => {
            type CurrentNet = example_networks::EvilTwinGadget;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::FirewallNet, _) => {
            type CurrentNet = example_networks::FirewallNet;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::MediumNet, _) => {
            type CurrentNet = example_networks::MediumNet;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep1)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition1>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep2)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition2>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep3)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition3>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep4)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition4>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep5)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition5>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep6)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition6>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep7)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition7>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep8)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition8>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep9)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition9>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep10)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition10>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep11)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition11>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep12)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition12>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep13)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition13>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, Some(Reps::Rep14)) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition14>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::VariableAbileneNetwork, _) => {
            type CurrentNet = example_networks::VariableAbileneNetwork<Repetition1>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::SimpleNet, _) => {
            type CurrentNet = example_networks::SimpleNet;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::SmallNet, _) => {
            type CurrentNet = example_networks::SmallNet;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep1)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition1>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep2)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition2>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep3)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition3>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep4)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition4>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep5)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition5>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep6)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition6>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep7)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition7>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep8)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition8>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep9)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition9>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep10)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition10>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep11)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition11>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep12)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition12>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep13)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition13>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep14)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition14>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep15)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition15>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep16)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition16>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep17)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition17>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep18)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition18>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep19)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition19>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep20)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition20>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep30)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition30>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep40)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition40>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep50)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition50>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep60)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition60>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep70)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition70>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep80)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition80>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep90)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition90>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, Some(Reps::Rep100)) => {
            type CurrentNet = example_networks::StateSpecificChainGadget<Repetition100>;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
        (Topology::StateSpecificChainGadget, _) => {
            type CurrentNet = example_networks::StateSpecificChainGadget;
            let net = CurrentNet::net(initial_variant);
            Ok((
                net.clone(),
                CurrentNet::final_config(&net, final_variant),
                CurrentNet::get_policy(&net, final_variant),
            ))
        }
    }
}
