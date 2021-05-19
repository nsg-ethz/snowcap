# SIGCOMM 2021 Evaluation

This folder contains material necessary to reproduce the results of _Snowcap_'s SIGCOMM paper.
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
[Download](https://polybox.ethz.ch/index.php/s/pE917fiv5Aiqbd0) and extract the virtual machine image: `tar -xvf path/to/SnowcapVM.tar.gz`.
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


### Setting up GNS3

For both the Docker and the Native Compilation method, GNS3 needs to be configured appropriately, such that the case-study (evaluation 11) can be run.
For this, install `gns3-server` and `gns3-gui` on the system.
Then, start up `gns3-gui` and add the following appliances:

- [FRRouting](https://gns3.com/marketplace/appliances/frr), and name it _exactly_ `FRR 7.3.1` (capitalization and spacing is important!).
- [Python, Go, Perl, PHP](https://gns3.com/marketplace/appliances/python-go-perl-php), and name it _exactly_ `Python, Go, Perl, PHP` (capitalization and spacing is important!).

Also, make sure that no authentication is required to connect to the GNS3 server (by editing the file `~/.config/GNS3/<VERSION>/gns3_server.conf` and setting `auth = False`).
Finally, close `gns3-gui` again, and make sure the server is no longer running.


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
After execution has finished, you can find the raw data at `eval_sigcomm2021/result_XX/`
```sh
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t -e "SPEEDUP=100" snowcap
```

For running the case study, you must first make sure that GNS3 is [setup properly](#setting-up-gns3).
Then, you can perform the measurement by running the following commands in the project root directory:

```sh
mkdir eval_sigcomm2021/result_11
gns3server > /dev/null 2>&1 &
sleep 5
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t snowcap /snowcap/target/release/snowcap_main run -r -s 3 -a --json /snowcap/eval_sigcomm2021/result_11/random.json topology-zoo /snowcap/eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t snowcap /snowcap/target/release/snowcap_main run --json /snowcap/eval_sigcomm2021/result_11/snowcap.json topology-zoo /snowcap/eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
docker run -v "$(pwd)/eval_sigcomm2021:/snowcap/eval_sigcomm2021" -t snowcap sh -c "cd /snowcap && python3.8 eval_sigcomm2021/scripts/table_11.py"
```

You can also generate the plots for the precomputed data.
The resulting plots will be stored at `eval_sigcomm2021/precomputed_results/plot_XX.pdf`
For this, build the docker image (as explained above), and then execute the following command:

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
  The python 3.8 binary must be available in the `$PATH`, under either the name `python`, `python3`, or `python3.8`.
- Latex build environment (and the program `pdflatex` available).
- GNS3 (`gns3-server` and `gns3-gui`), and follow the instructions on [setting up GNS3](#setting-up-gns3)

The provided scripts will check that all dependencies are installed correctly, and build _Snowcap_ if necessary.

#### Running the experiments

To run the experiments, change directory into the root of the project.
Then, to generate the data of the experiments (excluding the case-study on GNS3), execute the following command.
You can change the `SPEEDUP=100` factor, and you can also specify the number of threads used for the execution by setting `THREADS=XX`.
After completion, the raw data from all results will be stored at `eval_sigcomm2021/result_XX/`.

```sh
SPEEDUP=100 ./eval_sigcomm2021/scripts/generate_data.sh
```

To run the case-study, execute the following (still in the project root):

```sh
mkdir eval_sigcomm2021/result_11
gns3server > /dev/null 2>&1 &
sleep 5
RUST_LOG=info ./target/release/snowcap_main run -r -s 3 -a --json eval_sigcomm2021/result_11/random.json topology-zoo eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
RUST_LOG=info ./target/release/snowcap_main run --json eval_sigcomm2021/result_11/snowcap.json topology-zoo eval_sigcomm2021/topology_zoo/HiberniaIreland.gml FM2RR -s 10
```

Finally, you can generate the plots by running the following command.
This will first check that all dependencies are installed, and then generate the plots, which will be stored at `eval_sigcomm2021/plot_XX.pdf`.

```sh
./eval_sigcomm2021/scripts/generate_plots.sh
```

You can also generate the plots for the precomputed data using (which will generate all results in `eval_sigcomm2021/precomputed_results/plot_XX.pdf`):

```sh
PRECOMPUTED_DATA=yes ./eval_sigcomm2021/scripts/generate_plots.sh
```

</details>
</details>
