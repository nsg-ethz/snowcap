# SIGCOMM 2021 Evaluation

This folder contains material necessary to reproduce the results of Snowcap's SIGCOMM paper.
To simplify the process, we have prepared a virtual machine (for KVM), which is preconfigured.
The virtual machine can be downloaded [here](https://polybox.ethz.ch/index.php/s/pE917fiv5Aiqbd0) (the compressed archive is about 7.5 GB large, and the uncompressed image is 41 GB large).
It has all dependencies installed, and contains a clone of this repository.

This document will first highlight some [important notes](#important-notes) about running the experiments.
Then, we guide you through the [installation of KVM](#installing-kvm), enabling nested virtualization (tested on _Ubuntu 20.04 LTS_), and how to add and start the virtual machine.
Finally, we explain how to [execute the experiments](#running-the-experiments) and what output is expected.

## Important Notes

Generating the entire dataset is a time-consuming task.
We used a Server with 64 Cores (128 Threads), and it still took about one week (This means that it takes about 20'000 CPU hours to completely perform the evaluation).
To make it easier to perform the artifact validation, we have prepared the following:
- The script can be configured to perform less iterations (e.g., `SPEEDUP=100`). 
  This means that the collected data will not be as precise, and the collected statistics will be inaccurate.
  But it should still show the same trend.
  The experiments using `SPEEDUP=100` on a system with 24 cores will take somewhere between 10 and 20 hours to complete.
  If the processing time is available, you can use `SPEEDUP=1` to get the dataset as we have used in our paper.
- We have pre-computed all data, and we provide them to be analyzed. 
  To generate the plots based on the pre-computed data, see [Precomputed Data](#precomputed-data).
- Make sure to enable **nested virtualization** in the KVM module of the Linux kernel.
  A tutorial of this is described [here](#installing-kvm).
  If you don't enable this option, the last experiment will take considerably longer.

## Installing KVM
The following commands will setup `kvm`, `qemu` and `virt-manager` on an _Ubuntu 20.04 LTS_ system.
First, check if you are running an AMD or an Intel CPU. For AMD, replace `<CPU>` with `amd`, and for Intel, replace `<CPU>` with `intel`.
Then, check that nested virtualization is supported on your local machine:
```sh
cat /sys/module/kvm_<CPU>/parameters/nested
```
If this command returns `Y` or `1`, then nested virtualization is supported.
If it says `N` or ^`0`, then you might use a different machine.
Then, enable nested virtualization by running: (make sure that no other virtual machines are running)
```sh
sudo modprobe -r kvm_<CPU>
sudo modprobe kvm_<CPU> nested=1
```
Now, we can install and configure `kvm`, `qemu` and `virt-manager`:
```sh
sudo apt install qemu-kvm libvirt-daemon-system libvirt-clients bridge-utils virt-manager
sudo usermod -aG libvirt $USER
```
Then, log out and back in such that the changes take effect. Finally, start KVM with:
```sh
sudo systemctl start libvirtd
sudo virsh net-start default
```

### Adding the Virtual Machine
In the next step, we setup the virtual machine.
Extract the virtual machine image: `tar -xvf path/to/SnowcapVM.tar.gz`.
Then, start up `virt-manager`.
Create a new virtual machine, and choose _Import existing disk image_ and provide the extracted image (by clicking _Browse_, followed by _Browse Locally_).
As the operating system, choose _Ubuntu 20.04 LTS_.
Give the VM at least 16GB or RAM and _as many_ processing cores _as possible_ (as it will speed up the time to generate the data significantly).
Finally, check the box for customizing the VM before starting it.

In the configuration window, that pops up after finishing the dialog, go to _CPUs_, and make sure the option _Copy host CPU configuration_ is unchecked.
Then, in the _Model_ field, type `host-passthrough`.
Finally, press _Apply_ and then _Begin Installation_.
Now, the virtual machine should start, and you should see the desktop.

The password to the user `snowcap` is `snowcap`.

## Running the Experiments

Double-click on the `RUN.sh` script, located on the desktop.
Then, a window should pop-up asking for the `SPEEDUP` factor. 
Enter a factor (like `100`) and press enter.
A speedup factor of 100 takes about 12-24 hours on a virtual machine with 24 cores.
Pressing enter will start all experiments.

The experiments are executed one-by-one.
The results are stored, and the plots are generated after each individual experiment is completed.
The following 
1. **§2.1, Figure 2:**
   Case-study 1 showing the probability that reconfiguring the network from an iBGP full-mesh to a Route-Reflector topology causes black holes or forwarding loops.
2. **§2.2, Figure 3:**
   Case-study 2 showing the number of unnecessary traffic shifts caused by merging two networks.
   _Important_: We prepare the extended figure, that can be found in Appendix C.3, Figure 10.
3. **§6.1, Figure 7a:**
   Evaluation showing how _Snowcap_ scales compared to the _Random_ approach for solving the _Chain Gadget_ (simple dependencies).
4. **§6.1, Figure 7b:**
   Evaluation showing how _Snowcap_ scales compared to the _Random_ approach for solving the _Bipartite Gadget_ (complex dependencies).
5. **§6.1, Figure 7c:**
   Evaluation showing how the hard specification can impact the runtime of _Snowcap_.
6. **§6.2, Figure 8 left:**
   Evaluation comparing _Snowcap_ with three baseline approaches with respect to their induced cost while solving the problem of doubling all IGP link weights.
7. **§6.2, Figure 8 middle:**
   Evaluation comparing _Snowcap_ with three baseline approaches with respect to their induced cost while solving the problem of doubling all local preferences.
8. **§6.2, Figure 8 right:**
   Evaluation comparing _Snowcap_ with three baseline approaches with respect to their induced cost while solving the problem of network acquisition.
9. **§6.2, Figure 9:**
   Evaluation highlighting the computation overhead of _Snowcap_ optimizing for soft specification.
10. **§6.3**
    Evaluation measuring the accuracy of Snowcap with respect to transient behavior.
    _Important_: As this evaluation is not represented as a plot in the paper, we generate a table showing the results.
11. **§7**
    Case study where we apply _Snowcap_ to a reconfiguration scenario, and simulate it with GNS3.
    _Important_: As this evaluation is not represented as a plot in the paper, we generate a table showing the results.

Each of these measurements are performed one-by-one.
After each measurement is completed, the following files and folders are created (per measurement):
- `~/snowcap/eval_sigcomm2021/plot_X.pdf` (and `table_X.pdf`): The resulting plot, rendered in Tikz (very similar to the plots found in the paper)
- `~/snowcap/eval_sigcomm2021/result_X`: Raw data generated while generating the data
- `~/snowcap/eval_sigcomm2021/result_X/plot/plot.tex`: Generated `tex` file which is used to render the plot.

## Precomputed Data

We have prepared all measurements without changing `SPEEDUP=1`, and we provide them along with the project.
In order to generate the plots from the precomputed data, open a terminal and navigate to the `snowcap` folder:
```sh
cd ~/snowcap
```
Then, generate all plots at once using
```sh
PRECOMPUTED_DATA=yes ./eval_sigcomm2021/scripts/generate_plots.sh
```
Afterwards, all plots can be found in `~/snowcap/eval_sigcomm2021/precomputed_results/*.pdf`.



---



<details>
<summary>
<h2>Fallback without the provided VM</h2>
These commands are required <em><strong>only if</strong></em> the method above does not work, and if you cannot run the VM.
However, the Case study cannot be generated without the VM, as it requires GNS3 to be setup correctly.
</summary>

There are two different ways to perform the measurements without using the provided VM.
The first one uses the docker image, and the second one uses native compilation, where all dependencies need to be installed manually.

<details>
<summary>
<h3>Docker Method</h3>
</summary>

This method runs _Snowcap_ and all all scripts generating the plots in a prepared docker image, which has all dependencies installed and setup correctly.

#### Docker Setup

This method requires Docker to be installed and configured correctly on your system.
The following commands can be used to install and setup docker on an _Ubuntu 20.04 LTS_ system (taken from the [original Docker documentation](https://docs.docker.com/engine/install/ubuntu/)):
```sh
sudo apt-get install apt-transport-https ca-certificates curl gnupg lsb-release
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
sudo apt-get update
sudo apt-get install docker-ce docker-ce-cli containerd.io
sudo groupadd docker
sudo usermod -aG docker $USER
```
Then, log out and back in such that the changes take effect. Finally, start docker with:
```sh
sudo systemctl start docker
sudo systemctl start containerd
```

#### Running the experiments

Make sure that the current working directory is at the root of the project (where the file `Dockerfile` is located).
First, you have to build the docker file (make sure you run it as non-root):
```sh
docker build -t snowcap .
```

Then, you can start the evaluation process. 
You can change the `SPEEDUP` factor to reduce the number of iterations.
You can also specify the number of threads which should be spawned by adding `-e "THREADS=X"` to the command (before the last argument `snowcap`).
To use all threads available to the system, remove this argument.
```sh
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t -e "SPEEDUP=100" snowcap
```

After execution has finished, you can find all generated files at `eval_sigcomm2021/`.
Notice, that 

#### Running the case study

For running the case study, you must first make sure that GNS3 is setup properly.
For this, install `gns3-server` and `gns3-gui` on the system.
Then, start up `gns3-gui` and add the following appliances:
- [FRRouting](https://gns3.com/marketplace/appliances/frr), and name it _exactly_ `FRR 7.3.1` (capitalization and spacing is important!).
- [Python, Go, Perl, PHP](https://gns3.com/marketplace/appliances/python-go-perl-php), and name it _exactly_ `Python, Go, Perl, PHP` (capitalization and spacing is important!).
Also, make sure that no authentication is required to connect to the GNS3 server (by editing the file `~/.config/GNS3/<VERSION>/gns3_server.conf` and setting `auth = False`).
Then, you can perform the measurement by running the following commands in the project root directory:

```sh
mkdir eval_sigcomm2021/result_11
gns3server > /dev/null 2>&1 &
sleep 5
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t snowcap /snowcap/target/release/snowcap_main run -r -s 3 -a --json /snowcap/eval_sigcomm2021/result_11/random.json topology-zoo /snowcap/eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t snowcap /snowcap/target/release/snowcap_main run --json /snowcap/eval_sigcomm2021/result_11/snowcap.json topology-zoo /snowcap/eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t snowcap sh -c "cd /snowcap && python3.8 eval_sigcomm2021/scripts/table_11.py"
```

#### Using Precomputed Data

You can also generate the plots for the precomputed data.
For this, build the docker image (as explained above), and then type:

```sh
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t -e "PRECOMPUTED_DATA=yes" snowcap sh /snowcap/eval_sigcomm2021/scripts/generate_plots.sh
```

You can then find all generated plots at `eval_sigcomm2021/precomputed_results/`.

</details>

<details>
<summary>
<h3>Native Compilation</h3>
</summary>

#### Dependencies

- Stable [Rust toolchain](https://www.rust-lang.org/tools/install) (1.49 or higher, you might need to update the toolchain: `rustup update`, and make sure to have the cargo directory in the `$PATH` variable.)
- Python 3.8 or higher, with the packages `numpy`, `pandas` and `matplotlib` installed.
- Latex build environment (and the program `pdflatex` available).
- GNS3 (`gns3-server` and `gns3-gui`)

#### Setup

In the project root directory, build the project. (Don't forget to build for the release version)

``` sh
cargo build --release
```

#### General Notes

- Many experiments are based on topologies from Topology Zoo.
  These topologies are located at: `eval_sigcomm2021/topology_zoo/`.
  Our procedure for some topologies to generate configuration does not work in all cases, and these can safely be ignored (they will be ignored when using the commands below).
- All images in the paper are generated with Tikz.
  Hence, all scripts require the user to have a working installation of LaTeX on the machine.
  If LaTeX is not available on the server, you can copy the results `eval_sigcomm2021/result_*` to a local machine and run the python scripts from there.
- Once the script is executed, the plot is stored at `eval_sigcomm2021/plot_*.pdf`.
- It takes a very long time (several days) to run all experiments.
  All experiments are run in parallel, and hence, the more cores you use the better.
  We have used a server with 64 cores (128 threads) to speed up the process.
- All commands must be executed from the project root.

#### Case Study: IGP Reconfiguraiton (§2.1, Figure 2)

The first case study measures the probability of a reconfiguration ordering to violate reachability.
We take the topology zoo networks, and use the scenario `FM2RR`.
The following tests three different approaches:

- Random ordering of the commands
- Random ordering of the routers to reconfigure
- Best-practice: _Insert_ before _Update_ before _Remove_.

Reduce the number of iterations `-i 10000` to speed up the process. 

``` sh
mkdir eval_sigcomm2021/result_1
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    RUST_LOG=none ./target/release/problem_probability -i 10000 -n 10 -s FM2RR --many-prefixes eval_sigcomm2021/topology_zoo/${topo} probability -s -o eval_sigcomm2021/result_1/${topo}.json
done
python eval_sigcomm2021/scripts/plot_1.py
```

#### Case Study: Network Acquisition (§2.2, Figure 3)

The second case study measures the number of traffic shifts induced by performing a network merging scenario in a random fashion.
Here, we will produce the extended version from Figure 10 (in Appendix A).
For some topologies in TopologyZoo, the Network Acquisition scenario does not result in a valid configuration (due to graph properties).
These topologies are skipped (which is why the error `checks failed!` appears).

Reduce the number of iterations `-i 10000` to speed up the process. 

```sh
mkdir eval_sigcomm2021/result_2
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    RUST_LOG=none ./target/release/problem_probability --many-prefixes -i 10000 -n 1 -s NetAcq --seed 10 eval_sigcomm2021/topology_zoo/${topo} cost -a -f 100 -o eval_sigcomm2021/result_2/${topo}.json
done
python eval_sigcomm2021/scripts/plot_2.py
```

#### Evaluation: Scalability (Number of Commands) (§6.1, Figure 7a)

The first evaluation compares _Snowcap_ to the _Random_ approach for solving a simple problem, which can be scaled along the number of commands.
The _Random_ approach scales really bad, and it takes a very long time to find the correct order per chance.
Therefore, we just run the _Random_ approach up to a size of 9 (which already takes quite a while).
After that, we only do the computation for the _Exploration Only_ approach and _Snowcap_ itself. 

Reduce the number of iterations `-i 1000` to speed up the process.

```sh
mkdir eval_sigcomm2021/result_3
for N in 1 2 3 4 5 6 7 8 9; do
    RUST_LOG=none ./target/release/snowcap_main bench strategy --random --tree --main --json eval_sigcomm2021/result_3/n${N}.json -i 1000 example chain-gadget -r ${N}
done
for N in 10 11 12 13 14 15 16 17 18 19 20 30 40 50 60 70 80 90 100; do
    RUST_LOG=none ./target/release/snowcap_main bench strategy --tree --main --json eval_sigcomm2021/result_3/n${N}.json -i 1000 example chain-gadget -r ${N}
done
python eval_sigcomm2021/scripts/plot_3.py
```

#### Evaluation: Scalability (Difficult Dependencies) (§6.1, Figure 7b)

The second evaluation compares _Snowcap_ to the _Random_ and the _Exploration Only_ approach for solving a more complex problem, which can be scaled along the number of dependency groups without immediate effect.
Here, the _Exploration Only_, and the _Random_ approach scale really bad.
Therefore, we run the _Exploration Only_ approach up to 5, and the _Random_ approach up to 16 dependency groups, but _Snowcap_ for up to 20.

Reduce the number of iterations `-i 1000` to speed up the process.

```sh
mkdir eval_sigcomm2021/result_4
for N in 1 2 3 4 5; do
    RUST_LOG=none ./target/release/snowcap_main bench strategy --random --tree --main --json eval_sigcomm2021/result_4/n${N}.json -i 1000 example difficult-gadget-repeated -r ${N}
done
for N in 6 7 8 9 10 11 12 13 14 15 16; do
    RUST_LOG=none ./target/release/snowcap_main bench strategy --random --main --json eval_sigcomm2021/result_4/n${N}.json -i 1000 example difficult-gadget-repeated -r ${N}
done
for N in 17 18 19 20; do
    RUST_LOG=none ./target/release/snowcap_main bench strategy --main --json eval_sigcomm2021/result_4/n${N}.json -i 1000 example difficult-gadget-repeated -r ${N}
done
python eval_sigcomm2021/scripts/plot_4.py
```

#### Evaluation: Scalability (Specification Complexity) (§6.1, Figure 7c)

For this evaluation, we let _Snowcap_ run on the _Abilene Network_ (form Topology Zoo), while varying the number of commands and the complexity of the specification.
We vary the complexity of the specification from 0 to 66 (number of flows that are restricted), and we vary the number of commands from 5 to 29.

```sh
mkdir eval_sigcomm2021/result_5
for r in 1 3 5 7 9 11 13; do
    for v in $(seq 0 66); do
        RUST_LOG=none ./target/release/snowcap_main bench strategy --main -i 1000 --json eval_sigcomm2021/result_5/r${r}_v${v}.json example variable-abilene-network -i ${v} -r ${r}
    done
done
python eval_sigcomm2021/scripts/plot_5.py
```

#### Evaluation: Effectiveness (IGPx2) (§6.2, Figure 8 left)

We run the scenario _IGPx2_ on _Snowcap_ (while minimizing traffic shifts), as well as _Most-Important-First_ and _Most-Important-Last_, and the _Random_ approach on all topologies from topology-zoo.
This will take quite some time, so make sure you use as many cores as possible.
You can reduce the number of iterations by changing `-i 10000`.

```sh
mkdir eval_sigcomm2021/result_6
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" == "GtsCe.gml" ]; then
        echo "Skipping GtsCe.gml!"
    else
       RUST_LOG=none ./target/release/snowcap_main bench optimizer --main --mif --mil --random -i 10000 --json eval_sigcomm2021/result_6/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} IGPx2
    fi
done
python eval_sigcomm2021/scripts/plot_6-8.py 6
```

#### Evaluation: Effectiveness (LPx2) (§6.2, Figure 8 middle)

We run the scenario _LPx2_ on _Snowcap_ (while minimizing traffic shifts), as well as _Most-Important-First_ and _Most-Important-Last_, and the _Random_ approach on all topologies from topology-zoo.
This will take quite some time, so make sure you use as many cores as possible.
You can reduce the number of iterations by changing `-i 10000`.

```sh
mkdir eval_sigcomm2021/result_7
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" == "GtsCe.gml" ]; then
        echo "Skipping GtsCe.gml!"
    else
       RUST_LOG=none ./target/release/snowcap_main bench optimizer --main --mif --mil --random -i 10000 --json eval_sigcomm2021/result_7/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} LPx2
    fi
done
python eval_sigcomm2021/scripts/plot_6-8.py 7
```

#### Evaluation: Effectiveness (NetAcq) (§6.2, Figure 8 right)

We run the scenario _NetAcq_ on _Snowcap_ (while minimizing traffic shifts), as well as _Most-Important-First_ and _Most-Important-Last_, and the _Random_ approach on all topologies from topology-zoo.
This will take quite some time, so make sure you use as many cores as possible.
You can reduce the number of iterations by changing `-i 10000`.

```sh
mkdir eval_sigcomm2021/result_8
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" == "GtsCe.gml" ]; then
        echo "Skipping GtsCe.gml!"
    else
       RUST_LOG=none ./target/release/snowcap_main bench optimizer --main --mif --mil --random -i 10000 --json eval_sigcomm2021/result_8/${topo}.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} NetAcq
    fi
done
python eval_sigcomm2021/scripts/plot_6-8.py 8
```

#### Evaluation: Optimization Overhead (§6.2, Figure 9)

We run the scenario _FM2RR_ on _Snowcap_, once while minimizing for traffic shifts, and once without minimization.
In addition, we run the same scenario using the _Random_ approach for comparison.
We then compare the number of states, that have been explored.

```sh
mkdir eval_sigcomm2021/result_9
for topo in $(ls eval_sigcomm2021/topology_zoo); do 
    if [ "$topo" == "GtsCe.gml" ]; then
        echo "Skipping GtsCe.gml!"
    else
        RUST_LOG=none ./target/release/snowcap_main bench strategy --main -i 64 -t 100000 --json eval_sigcomm2021/result_9/${topo}.strat.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR &&\
        RUST_LOG=none ./target/release/snowcap_main bench optimizer --main -i 64 -t 100000 --json eval_sigcomm2021/result_9/${topo}.optim.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR &&\
        RUST_LOG=none ./target/release/snowcap_main bench strategy --random -i 10000 --json eval_sigcomm2021/result_9/${topo}.rand.json topology-zoo -m eval_sigcomm2021/topology_zoo/${topo} FM2RR
    fi
done
python eval_sigcomm2021/scripts/plot_9.py
```

#### Evaluation: Accuracy of Snowcap (§6.3)

We load the _Switch_ topology from Topology Zoo, on top of which we reconfigure the random configuration to remove one external session.
During this, we assert that some path conditions are still ensured.
This will not generate a plot, but a Table summarizing the results.

```sh
mkdir eval_sigcomm2021/result_10
RUST_LOG=none ./target/release/snowcap_main transient eval_sigcomm2021/topology_zoo/SwitchL3.gml -i 1000 -r | tee eval_sigcomm2021/result_10/raw_output
python eval_sigcomm2021/scripts/table_10.py
```

#### Case Study with GNS3 (§7)

For running the case study, you must first make sure that GNS3 is setup properly.
For this, install `gns3-server` and `gns3-gui` on the system.
Then, start up `gns3-gui` and add the following appliances:
- [FRRouting](https://gns3.com/marketplace/appliances/frr), and name it _exactly_ `FRR 7.3.1` (capitalization and spacing is important!).
- [Python, Go, Perl, PHP](https://gns3.com/marketplace/appliances/python-go-perl-php), and name it _exactly_ `Python, Go, Perl, PHP` (capitalization and spacing is important!).
Also, make sure that no authentication is required to connect to the GNS3 server (by editing the file `~/.config/GNS3/<VERSION>/gns3_server.conf` and setting `auth = False`).
Then, you can perform the measurement by running the following commands in the project root directory:

```sh
mkdir eval_sigcomm2021/result_11
gns3server > /dev/null 2>&1 &
sleep 5
./target/release/snowcap_main run -r -s 3 -a --json eval_sigcomm2021/result_11/random.json topology-zoo eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
./target/release/snowcap_main run --json eval_sigcomm2021/result_11/snowcap.json topology-zoo eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
python3.8 ./eval_sigcomm2021/scripts/table_11.py
```

#### Using Precomputed Data

You can also generate the plots for the precomputed data.
For this, build the docker image (as explained above), and then type:

```sh
PRECOMPUTED_DATA=yes ./eval_sigcomm2021/scripts/generate_plots.sh
```

You can then find all generated plots at `eval_sigcomm2021/precomputed_results/`.

</details>
</details>
